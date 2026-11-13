//! Supervision of the driver servers: one `p11-kit server --provider
//! <module>` per available driver, on $XDG_RUNTIME_DIR/tessera/<driver>.
//!
//! One server per driver needs no p11-kit configuration (which Flatpak
//! disables inside sandboxes), and a driver that fails to load or crashes
//! affects only its own server. For each connection, p11-kit server starts
//! `p11-kit remote`, which loads the driver with LD_LIBRARY_PATH set from the
//! driver's `library_path`.

use anyhow::{bail, Context, Result};
use std::collections::BTreeMap;
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
    /// The environment its server was started with (driver.env_with).
    env: BTreeMap<String, String>,
}

#[derive(Default)]
pub struct Server {
    instances: Vec<Instance>,
    failure: Option<String>,
    /// The drivers given to start(), for `settle`.
    drivers: Vec<Driver>,
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

fn spawn(driver: &Driver, env: &BTreeMap<String, String>) -> Result<Child> {
    let socket = paths::driver_socket(&driver.id);
    if socket.exists() {
        if socket_answers(&socket) {
            bail!("conflict");
        }
        // A stale socket left by a server that died.
        let _ = fs::remove_file(&socket);
    }
    let log = File::create(log_path(&driver.id))?;
    let mut cmd = Command::new(paths::p11kit_server_command());
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
    cmd.arg("--foreground")
        .arg("--name")
        .arg(&socket)
        .arg("--provider")
        .arg(&driver.module)
        .arg("pkcs11:")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(log);
    cmd.envs(env);
    // Each connection is served by `tessera remote` instead of
    // `p11-kit remote` (see remote.rs).
    match crate::remote::private_dir() {
        Ok(dir) => {
            // Our notify-send first (see remote.rs).
            let mut path = vec![dir.join("bin")];
            if let Some(old) = std::env::var_os("PATH") {
                path.extend(std::env::split_paths(&old));
            }
            if let Ok(joined) = std::env::join_paths(path) {
                cmd.env("PATH", joined);
            }
            cmd.env("P11_KIT_PRIVATEDIR", dir);
        }
        Err(e) => eprintln!("tessera: using p11-kit remote: {e}"),
    }
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
        self.drivers = drivers.to_vec();
        let wanted: Vec<&Driver> = drivers
            .iter()
            .filter(|d| d.available() && !disabled.contains(&d.id))
            .collect();
        // The drivers whose environment depends on others (OpenSC on
        // SafeNet's) start last, with the set that actually started.
        let (plain, dependent): (Vec<&Driver>, Vec<&Driver>) =
            wanted.into_iter().partition(|d| d.when_on.is_empty());
        for d in plain.into_iter().chain(dependent) {
            let env = d.env_with(&self.running());
            let (child, failure) = match spawn(d, &env) {
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
                env,
            });
        }
        if conflict {
            bail!("conflict");
        }
        Ok(())
    }

    /// Start again, with the right environment, every running driver whose
    /// environment depends on which others run (`when_on`), after one of
    /// those stopped or came back: OpenSC leaves IDPrime cards to SafeNet
    /// only while SafeNet's server runs.
    fn settle(&mut self) {
        let running = self.running();
        for i in 0..self.instances.len() {
            if self.instances[i].child.is_none() {
                continue;
            }
            let Some(d) = self
                .drivers
                .iter()
                .find(|d| d.id == self.instances[i].id && !d.when_on.is_empty())
                .cloned()
            else {
                continue;
            };
            let env = d.env_with(&running);
            if env == self.instances[i].env {
                continue;
            }
            let inst = &mut self.instances[i];
            if let Some(child) = inst.child.take() {
                stop_child(child);
                let _ = fs::remove_file(paths::driver_socket(&inst.id));
            }
            match spawn(&d, &env) {
                Ok(c) => {
                    inst.child = Some(c);
                    inst.failure = None;
                }
                Err(e) => inst.failure = Some(format!("{e:#}")),
            }
            inst.env = env;
        }
    }

    /// Number of drivers whose server runs, and of those switched on.
    pub fn counts(&self) -> (usize, usize) {
        (
            self.instances.iter().filter(|i| i.child.is_some()).count(),
            self.instances.len(),
        )
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
        self.settle();
    }

    /// Start one driver's server again (after it stopped answering once).
    pub fn restart_driver(&mut self, driver: &Driver) {
        // Without its own server: the others that run.
        let others: Vec<String> = self
            .running()
            .into_iter()
            .filter(|id| id != &driver.id)
            .collect();
        let env = driver.env_with(&others);
        for inst in self.instances.iter_mut().filter(|i| i.id == driver.id) {
            if let Some(child) = inst.child.take() {
                stop_child(child);
                let _ = fs::remove_file(paths::driver_socket(&inst.id));
            }
            match spawn(driver, &env) {
                Ok(c) => {
                    inst.child = Some(c);
                    inst.failure = None;
                }
                Err(e) => inst.failure = Some(format!("{e:#}")),
            }
            inst.env = env.clone();
        }
        self.settle();
    }

    /// Switch one driver's server on or off, leaving the others' servers
    /// alone: an app keeps its connection to each server for as long as it
    /// runs, and p11-kit does not connect again, so stopping every server
    /// (start) takes every token away from the apps that are open. The
    /// drivers whose environment depends on this one (`when_on`) are started
    /// again by settle, and only those.
    pub fn set_enabled(&mut self, driver: &Driver, on: bool) {
        if on {
            if self
                .instances
                .iter()
                .any(|i| i.id == driver.id && i.child.is_some())
            {
                return;
            }
            self.instances.retain(|i| i.id != driver.id);
            if !self.drivers.iter().any(|d| d.id == driver.id) {
                self.drivers.push(driver.clone());
            }
            let env = driver.env_with(&self.running());
            let (child, failure) = match spawn(driver, &env) {
                Ok(c) => (Some(c), None),
                Err(e) => (None, Some(format!("{e:#}"))),
            };
            self.instances.push(Instance {
                id: driver.id.clone(),
                name: driver.name.clone(),
                child,
                failure,
                env,
            });
        } else if let Some(pos) = self.instances.iter().position(|i| i.id == driver.id) {
            let inst = self.instances.remove(pos);
            if let Some(child) = inst.child {
                stop_child(child);
                let _ = fs::remove_file(paths::driver_socket(&inst.id));
            }
        }
        self.settle();
    }

    /// The process id of a driver's server, if it runs (for tests).
    #[cfg(test)]
    fn pid_of(&self, id: &str) -> Option<u32> {
        self.instances
            .iter()
            .find(|i| i.id == id)
            .and_then(|i| i.child.as_ref())
            .map(|c| c.id())
    }

    /// The overall state: Running when every driver's server runs.
    pub fn poll(&mut self) -> State {
        let mut exited = false;
        for inst in self.instances.iter_mut() {
            if let Some(child) = inst.child.as_mut() {
                match child.try_wait() {
                    Ok(None) => {}
                    Ok(Some(status)) => {
                        exited = true;
                        inst.child = None;
                        let tail = log_tail(&inst.id);
                        inst.failure = Some(if tail.is_empty() {
                            status.to_string()
                        } else {
                            tail
                        });
                    }
                    Err(e) => {
                        exited = true;
                        inst.child = None;
                        inst.failure = Some(e.to_string());
                    }
                }
            }
        }
        if exited {
            self.settle();
        }
        let mut failures = Vec::new();
        for inst in &self.instances {
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
        // One driver down is shown on its own row; the service runs as long
        // as one driver does.
        if self.instances.iter().any(|i| i.child.is_some()) {
            return State::Running;
        }
        if !failures.is_empty() {
            return State::Failed(failures.join("\n"));
        }
        State::Stopped
    }

    /// Why a driver's server is not running, if it was started and stopped.
    pub fn failure_of(&self, id: &str) -> Option<String> {
        self.instances
            .iter()
            .find(|i| i.id == id && i.child.is_none())
            .and_then(|i| i.failure.clone())
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A driver served by a real p11-kit server, with a module that needs
    /// no token (p11-kit's trust module).
    fn driver(id: &str, when_on: &[&str]) -> Driver {
        Driver {
            id: id.into(),
            name: id.into(),
            vendor: String::new(),
            description: String::new(),
            module: PathBuf::from("/usr/lib/x86_64-linux-gnu/pkcs11/p11-kit-trust.so"),
            tools: vec![],
            create_dirs: vec![],
            library_path: vec![],
            env: BTreeMap::new(),
            when_on: when_on
                .iter()
                .map(|o| {
                    (
                        o.to_string(),
                        BTreeMap::from([("TESSERA_TEST_WITH".to_string(), o.to_string())]),
                    )
                })
                .collect(),
        }
    }

    #[test]
    fn switching_one_driver_leaves_the_others_running() {
        let module = Path::new("/usr/lib/x86_64-linux-gnu/pkcs11/p11-kit-trust.so");
        if !module.is_file() || !paths::p11kit_server_command().is_file() {
            // In CI the tools are there, and a skip would hide a failure.
            if std::env::var("TESSERA_TEST_REQUIRE").as_deref() == Ok("1") {
                panic!("no p11-kit server or trust module (TESSERA_TEST_REQUIRE=1)");
            }
            eprintln!("skipped: no p11-kit server or trust module here");
            return;
        }
        let dir = std::env::temp_dir().join(format!("tessera-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::env::set_var("TESSERA_SOCKET_DIR", dir.join("sockets"));
        std::env::set_var("XDG_DATA_HOME", dir.join("data"));

        // a and b plain; c depends on b (as OpenSC does on SafeNet).
        let ds = vec![driver("a", &[]), driver("b", &[]), driver("c", &["b"])];
        let mut srv = Server::new();
        srv.start(&ds, &[]).unwrap();
        let (a, c) = (srv.pid_of("a").unwrap(), srv.pid_of("c").unwrap());
        assert!(srv.pid_of("b").is_some());
        // An app's connection to a's server, open across the switch.
        let app = UnixStream::connect(paths::driver_socket("a")).unwrap();

        srv.set_enabled(&ds[1], false);
        assert_eq!(srv.pid_of("b"), None);
        assert!(!paths::driver_socket("b").exists());
        assert_eq!(srv.pid_of("a"), Some(a), "a's server was restarted");
        assert!(socket_answers(&paths::driver_socket("a")));
        let c2 = srv.pid_of("c").unwrap();
        assert_ne!(c2, c, "c depends on b and must start again");
        assert_eq!(srv.counts(), (2, 2));

        srv.set_enabled(&ds[1], true);
        assert!(srv.pid_of("b").is_some());
        assert!(socket_answers(&paths::driver_socket("b")));
        assert_eq!(srv.pid_of("a"), Some(a), "a's server was restarted");
        assert_ne!(srv.pid_of("c").unwrap(), c2, "c must start again with b");
        assert_eq!(srv.counts(), (3, 3));
        // Switching on a driver that runs changes nothing.
        let b = srv.pid_of("b");
        srv.set_enabled(&ds[1], true);
        assert_eq!(srv.pid_of("b"), b);

        drop(app);
        srv.stop();
        let _ = std::fs::remove_dir_all(&dir);
    }
}
