//! Supervision of the `p11-kit server` process.
//!
//! The server loads the drivers (inside Tessera's sandbox, where they and
//! their vendor tools live) and serves every token on
//! $XDG_RUNTIME_DIR/p11-kit/pkcs11. Client apps load the runtime's
//! p11-kit-client.so, which talks to that socket.

use anyhow::{bail, Context, Result};
use std::fs::{self, File};
use std::os::unix::net::UnixStream;
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use crate::drivers::{self, Driver};
use crate::paths;

const PR_SET_PDEATHSIG: i32 = 1;
const SIGTERM: u64 = 15;

extern "C" {
    fn prctl(option: i32, arg2: u64, arg3: u64, arg4: u64, arg5: u64) -> i32;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum State {
    Stopped,
    Running,
    /// Something else (usually the host's p11-kit-server.socket) answers on
    /// the socket.
    Conflict,
    Failed(String),
}

#[derive(Default)]
pub struct Server {
    child: Option<Child>,
    failure: Option<String>,
}

fn log_path() -> PathBuf {
    paths::p11kit_config_home().join("server.log")
}

fn log_tail() -> String {
    let text = fs::read_to_string(log_path()).unwrap_or_default();
    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    lines[lines.len().saturating_sub(5)..].join("\n")
}

/// Is anyone accepting connections on the socket?
pub fn socket_answers() -> bool {
    UnixStream::connect(paths::socket_path()).is_ok()
}

impl Server {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn start(&mut self, drivers: &[Driver]) -> Result<()> {
        self.stop();
        self.failure = None;
        let socket = paths::socket_path();
        if socket.exists() {
            if socket_answers() {
                bail!("conflict");
            }
            // A stale socket left by a server that died.
            let _ = fs::remove_file(&socket);
        }
        if let Some(parent) = socket.parent() {
            fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
        }
        drivers::prepare_dirs(drivers);
        let config_home = drivers::write_p11kit_config(drivers)?;
        let log = File::create(log_path())?;
        let mut cmd = Command::new(paths::p11kit_command());
        // If Tessera dies without stopping the server (killed, crashed), the
        // kernel stops the server too, so no orphan keeps the socket busy.
        // SAFETY: prctl is async-signal-safe and touches no Rust state.
        unsafe {
            cmd.pre_exec(|| {
                if prctl(PR_SET_PDEATHSIG, SIGTERM, 0, 0, 0) != 0 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
        let child = cmd
            .arg("server")
            .arg("--foreground")
            .arg("--name")
            .arg(&socket)
            .arg("pkcs11:")
            .env("XDG_CONFIG_HOME", &config_home)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(log)
            .spawn()
            .context("starting p11-kit server")?;
        self.child = Some(child);

        let deadline = Instant::now() + Duration::from_secs(3);
        while Instant::now() < deadline {
            if socket.exists() {
                return Ok(());
            }
            if let State::Failed(msg) = self.poll() {
                bail!(msg);
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        Ok(())
    }

    pub fn stop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
            let _ = fs::remove_file(paths::socket_path());
        }
    }

    pub fn poll(&mut self) -> State {
        if let Some(child) = self.child.as_mut() {
            match child.try_wait() {
                Ok(None) => return State::Running,
                Ok(Some(status)) => {
                    self.child = None;
                    let tail = log_tail();
                    let msg = if tail.is_empty() {
                        format!("{status}")
                    } else {
                        tail
                    };
                    self.failure = Some(msg);
                }
                Err(e) => {
                    self.child = None;
                    self.failure = Some(e.to_string());
                }
            }
        }
        if let Some(f) = &self.failure {
            if f == "conflict" {
                return State::Conflict;
            }
            return State::Failed(f.clone());
        }
        if socket_answers() {
            return State::Conflict;
        }
        State::Stopped
    }

    /// Record a start error so `poll` reports it.
    pub fn set_failure(&mut self, msg: String) {
        self.failure = Some(msg);
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stop();
    }
}
