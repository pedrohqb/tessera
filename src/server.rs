//! Supervision of the driver servers: one `p11-kit server --provider
//! <module>` per available driver, on $XDG_RUNTIME_DIR/tessera/<driver>.
//!
//! One server per driver needs no p11-kit configuration (which Flatpak
//! disables inside sandboxes), and a driver that fails to load or crashes
//! affects only its own server. For each connection, p11-kit server starts
//! `p11-kit remote`, which loads the driver with LD_LIBRARY_PATH set from the
//! driver's `library_path`.

use anyhow::{bail, Context, Result};
use std::fs::{self, File};
use std::os::unix::net::UnixStream;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use crate::drivers::{self, Driver};
use crate::paths;

const PR_SET_PDEATHSIG: i32 = 1;
const SIGTERM: u64 = 15;

extern "C" {
    fn prctl(option: i32, arg2: u64, arg3: u64, arg4: u64, arg5: u64) -> i32;
    fn setpgid(pid: i32, pgid: i32) -> i32;
    fn kill(pid: i32, sig: i32) -> i32;
}

/// Stop a server and everything it started. p11-kit server runs a
/// `p11-kit remote` per connection; one stuck in a driver that hangs would
/// outlive its server, so the server runs in its own process group and the
/// whole group is stopped.
fn stop_child(mut child: Child) {
    let pid = child.id() as i32;
    // SAFETY: plain syscalls on a process group we created.
    unsafe {
        kill(-pid, 15 /* SIGTERM */);
    }
    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < deadline {
        if let Ok(Some(_)) = child.try_wait() {
            break;
        }
        std::thread::sleep(Duration::from_millis(30));
    }
    // SAFETY: as above.
    unsafe {
        kill(-pid, 9 /* SIGKILL */);
    }
    let _ = child.kill();
    let _ = child.wait();
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum State {
    Stopped,
    Running,
    /// Something else answers on one of the sockets (another Tessera).
    Conflict,
    Failed(String),
}

struct Instance {
    id: String,
    name: String,
    child: Option<Child>,
    failure: Option<String>,
}

#[derive(Default)]
pub struct Server {
    instances: Vec<Instance>,
    failure: Option<String>,
}

fn log_path(id: &str) -> PathBuf {
    paths::log_dir().join(format!("{id}.log"))
}

fn log_tail(id: &str) -> String {
    let text = fs::read_to_string(log_path(id)).unwrap_or_default();
    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    lines[lines.len().saturating_sub(3)..].join("\n")
}

pub fn socket_answers(socket: &Path) -> bool {
    UnixStream::connect(socket).is_ok()
}

fn spawn(driver: &Driver) -> Result<Child> {
    let socket = paths::driver_socket(&driver.id);
    if socket.exists() {
        if socket_answers(&socket) {
            bail!("conflict");
        }
        // A stale socket left by a server that died.
        let _ = fs::remove_file(&socket);
    }
    let log = File::create(log_path(&driver.id))?;
    let mut cmd = Command::new(paths::p11kit_command());
    // If Tessera dies without stopping the server (killed, crashed), the
    // kernel stops the server too, so no orphan keeps the socket busy.
    // SAFETY: prctl is async-signal-safe and touches no Rust state.
    unsafe {
        cmd.pre_exec(|| {
            if prctl(PR_SET_PDEATHSIG, SIGTERM, 0, 0, 0) != 0 || setpgid(0, 0) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    cmd.arg("server")
        .arg("--foreground")
        .arg("--name")
        .arg(&socket)
        .arg("--provider")
        .arg(&driver.module)
        .arg("pkcs11:")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(log);
    cmd.envs(&driver.env);
    if !driver.library_path.is_empty() {
        let mut parts: Vec<PathBuf> = driver.library_path.clone();
        if let Some(old) = std::env::var_os("LD_LIBRARY_PATH") {
            parts.extend(std::env::split_paths(&old));
        }
        cmd.env("LD_LIBRARY_PATH", std::env::join_paths(parts)?);
    }
    let child = cmd.spawn().context("starting p11-kit server")?;

    let deadline = Instant::now() + Duration::from_secs(3);
    while Instant::now() < deadline && !socket.exists() {
        std::thread::sleep(Duration::from_millis(30));
    }
    Ok(child)
}

impl Server {
    pub fn new() -> Self {
        Self::default()
    }

    /// Start a server for every available driver not in `disabled`.
    pub fn start(&mut self, drivers: &[Driver], disabled: &[String]) -> Result<()> {
        self.stop();
        self.failure = None;
        let dir = paths::socket_dir();
        fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
        fs::create_dir_all(paths::log_dir())?;
        drivers::prepare_dirs(drivers);
        let mut conflict = false;
        for d in drivers
            .iter()
            .filter(|d| d.available() && !disabled.contains(&d.id))
        {
            let (child, failure) = match spawn(d) {
                Ok(c) => (Some(c), None),
                Err(e) => {
                    let msg = format!("{e:#}");
                    conflict |= msg == "conflict";
                    (None, Some(msg))
                }
            };
            self.instances.push(Instance {
                id: d.id.clone(),
                name: d.name.clone(),
                child,
                failure,
            });
        }
        if conflict {
            bail!("conflict");
        }
        Ok(())
    }

    pub fn stop(&mut self) {
        for inst in self.instances.drain(..) {
            if let Some(child) = inst.child {
                stop_child(child);
                let _ = fs::remove_file(paths::driver_socket(&inst.id));
            }
        }
    }

    /// Stop one driver's server and record why (a driver that does not
    /// answer would hang every app that loads it).
    pub fn stop_driver(&mut self, id: &str, reason: &str) {
        for inst in self.instances.iter_mut().filter(|i| i.id == id) {
            if let Some(child) = inst.child.take() {
                stop_child(child);
                let _ = fs::remove_file(paths::driver_socket(&inst.id));
            }
            inst.failure = Some(reason.to_string());
        }
    }

    /// The overall state: Running when every driver's server runs.
    pub fn poll(&mut self) -> State {
        let mut failures = Vec::new();
        for inst in self.instances.iter_mut() {
            if let Some(child) = inst.child.as_mut() {
                match child.try_wait() {
                    Ok(None) => continue,
                    Ok(Some(status)) => {
                        inst.child = None;
                        let tail = log_tail(&inst.id);
                        inst.failure = Some(if tail.is_empty() {
                            status.to_string()
                        } else {
                            tail
                        });
                    }
                    Err(e) => {
                        inst.child = None;
                        inst.failure = Some(e.to_string());
                    }
                }
            }
            if let Some(f) = &inst.failure {
                if f == "conflict" {
                    return State::Conflict;
                }
                failures.push(format!("{}: {f}", inst.name));
            }
        }
        if let Some(f) = &self.failure {
            return State::Failed(f.clone());
        }
        if !failures.is_empty() {
            return State::Failed(failures.join("\n"));
        }
        if self.instances.iter().any(|i| i.child.is_some()) {
            State::Running
        } else {
            State::Stopped
        }
    }

    /// Ids of the drivers whose server is running.
    pub fn running(&self) -> Vec<String> {
        self.instances
            .iter()
            .filter(|i| i.child.is_some())
            .map(|i| i.id.clone())
            .collect()
    }

    /// Record a start error so `poll` reports it.
    pub fn set_failure(&mut self, msg: String) {
        if msg != "conflict" {
            self.failure = Some(msg);
        }
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stop();
    }
}
