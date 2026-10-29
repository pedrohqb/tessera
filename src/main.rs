//! Tessera: a PKCS#11 token hub for Flatpak.
//!
//! Usage:
//!   tessera                     open the window (and start the token service)
//!   tessera --background        start the token service without a window
//!   tessera serve               run only the token service, in the foreground
//!   tessera status              print readers, tokens and drivers
//!   tessera nss-setup DIR       register p11-kit-client.so in the NSS
//!                               database DIR (a Firefox profile, say)
//!   tessera probe-module MOD    load a driver directly and report each step
//!   tessera remote ...          (internal) serve one connection for p11-kit server
//!   tessera list-tokens         (internal) list tokens at P11_KIT_SERVER_ADDRESS
//!   tessera nss-add-module DIR  (internal) the NSS work of nss-setup
//!   tessera nss-poppler DIR      (internal) make the NSS database for Papers and Okular

mod apps;
mod drivers;
mod i18n;
mod nss;
mod paths;
mod remote;
mod server;
mod settings;
mod stable;
mod status;
mod trace;
mod tray;
mod ui;

use std::path::PathBuf;
use std::process::ExitCode;

/// (id, name) of the drivers whose server answers.
fn answering_drivers(ds: &[drivers::Driver]) -> Vec<(String, String)> {
    ds.iter()
        .filter(|d| server::socket_answers(&paths::driver_socket(&d.id)))
        .map(|d| (d.id.clone(), d.name.clone()))
        .collect()
}

fn print_status() -> ExitCode {
    let ds = drivers::discover();
    let answering = answering_drivers(&ds);
    println!("Sockets: {}", paths::socket_dir().display());
    println!("Host modules: {}", paths::host_modules_dir().display());
    println!("\nDrivers:");
    if ds.is_empty() {
        println!("  (none)");
    }
    for d in &ds {
        let state = if !d.available() {
            "files missing"
        } else if answering.iter().any(|(id, _)| id == &d.id) {
            "serving"
        } else {
            "not running"
        };
        println!("  {} [{}] {} ({state})", d.name, d.id, d.module.display());
    }
    let snap = status::snapshot(&answering);
    println!("\nReaders:");
    match &snap.readers {
        status::Readers::Ok(rs) if rs.is_empty() => println!("  (none)"),
        status::Readers::Ok(rs) => {
            for r in rs {
                let present = if r.card_present {
                    "  [card present]"
                } else {
                    ""
                };
                println!("  {}{present}", r.name);
            }
        }
        status::Readers::NoService(e) => println!("  pcscd not reachable: {e}"),
    }
    println!("\nTokens:");
    if snap.tokens.is_empty() {
        println!("  (none)");
    }
    for t in &snap.tokens {
        println!(
            "  {} ({} {}, serial {}) via {}",
            t.label, t.manufacturer, t.model, t.serial, t.driver
        );
        for c in &t.certificates {
            let expired = if c.expired { " (expired)" } else { "" };
            println!(
                "    - {} | issuer: {} | valid until {}{expired}",
                c.subject, c.issuer, c.not_after
            );
        }
    }
    for e in &snap.errors {
        println!("  error: {e}");
    }
    ExitCode::SUCCESS
}

/// Consecutive listings without an answer before a driver is stopped.
pub const MAX_STRIKES: u32 = 3;

/// What the supervision remembers between two checks.
#[derive(Default)]
pub struct Health {
    strikes: std::collections::HashMap<String, u32>,
}

impl Health {
    pub fn reset(&mut self) {
        self.strikes.clear();
    }

    /// Listings in a row without an answer from this driver.
    pub fn strikes(&self, id: &str) -> u32 {
        self.strikes.get(id).copied().unwrap_or(0)
    }
}

/// After a listing: publish to the host the drivers that answered; restart
/// one that did not, and stop it after MAX_STRIKES listings in a row (a
/// driver that hangs would hang every app that loads it). Shared by `serve`
/// and the window.
pub fn check_drivers(
    srv: &mut server::Server,
    ds: &[drivers::Driver],
    snap: &status::Snapshot,
    health: &mut Health,
) -> Result<usize, String> {
    for id in &snap.ok {
        health.strikes.remove(id);
    }
    for id in &snap.timed_out {
        let n = health.strikes.entry(id.clone()).or_insert(0);
        *n += 1;
        if *n >= MAX_STRIKES {
            srv.stop_driver(
                id,
                i18n::t(
                    "No answer 3 times in a row. Press Restart to try again, or switch it off.",
                ),
            );
        } else if let Some(d) = ds.iter().find(|d| &d.id == id) {
            srv.restart_driver(d);
        }
    }
    apps::write_host_modules(ds, &snap.ok).map_err(|e| format!("{e:#}"))
}

fn serve() -> ExitCode {
    let ds = drivers::discover();
    let disabled = settings::Settings::load().disabled_drivers;
    let mut srv = server::Server::new();
    if let Err(e) = srv.start(&ds, &disabled) {
        if e.to_string() == "conflict" {
            eprintln!(
                "tessera: another Tessera already serves in {}",
                paths::socket_dir().display()
            );
            return ExitCode::FAILURE;
        }
        eprintln!("tessera: cannot start the token service: {e:#}");
        return ExitCode::FAILURE;
    }
    // Nothing is published until each driver has answered once.
    let _ = apps::write_host_modules(&ds, &[]);
    eprintln!("tessera: serving in {}", paths::socket_dir().display());
    let mut last = usize::MAX;
    let mut health = Health::default();
    loop {
        let running: Vec<(String, String)> = ds
            .iter()
            .filter(|d| srv.running().contains(&d.id))
            .map(|d| (d.id.clone(), d.name.clone()))
            .collect();
        let snap = status::snapshot(&running);
        for e in &snap.errors {
            eprintln!("tessera: {e}");
        }
        for id in &snap.timed_out {
            eprintln!(
                "tessera: {id}: no answer ({} of {MAX_STRIKES})",
                health.strikes(id) + 1
            );
        }
        match check_drivers(&mut srv, &ds, &snap, &mut health) {
            Ok(n) if n != last => {
                eprintln!(
                    "tessera: {n} driver(s) published in {}",
                    paths::host_modules_dir().display()
                );
                last = n;
            }
            Ok(_) => {}
            Err(e) => eprintln!("tessera: cannot write the host modules: {e}"),
        }
        std::thread::sleep(std::time::Duration::from_secs(10));
    }
}

extern "C" {
    fn signal(signum: i32, handler: usize) -> usize;
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("status") => {
            // Let `tessera status | head` end quietly instead of panicking
            // on a closed pipe (only here: the GUI keeps SIGPIPE ignored).
            // SAFETY: restoring SIGPIPE's default action before any thread
            // exists.
            unsafe {
                signal(13 /* SIGPIPE */, 0 /* SIG_DFL */);
            }
            return print_status();
        }
        Some("serve") => return serve(),
        Some("remote") => return ExitCode::from(remote::run(&args[2..]) as u8),
        Some("probe-sign") => {
            let Some(module) = args.get(2) else {
                eprintln!("usage: tessera probe-sign MODULE");
                return ExitCode::FAILURE;
            };
            return ExitCode::from(status::probe_sign(module) as u8);
        }
        Some("probe-module") => {
            let Some(module) = args.get(2) else {
                eprintln!("usage: tessera probe-module MODULE");
                return ExitCode::FAILURE;
            };
            return ExitCode::from(status::probe_module(module) as u8);
        }
        Some("list-tokens") => {
            return match status::list_tokens_in_process() {
                Ok(text) => {
                    print!("{text}");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("{e}");
                    ExitCode::FAILURE
                }
            };
        }
        Some("nss-setup") => {
            let Some(dir) = args.get(2) else {
                eprintln!("usage: tessera nss-setup DIR");
                return ExitCode::FAILURE;
            };
            return match nss::add_module(&PathBuf::from(dir)) {
                Ok(_) => {
                    println!("{dir}: {} registered", paths::CLIENT_MODULE);
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("tessera: {e:#}");
                    ExitCode::FAILURE
                }
            };
        }
        Some("nss-poppler") => {
            let Some(dir) = args.get(2) else {
                eprintln!("usage: tessera nss-poppler DIR");
                return ExitCode::FAILURE;
            };
            return match nss::set_up_poppler_db_in_process(&PathBuf::from(dir)) {
                Ok(()) => ExitCode::SUCCESS,
                Err(e) => {
                    eprintln!("{e:#}");
                    ExitCode::FAILURE
                }
            };
        }
        Some("nss-add-module") => {
            let Some(dir) = args.get(2) else {
                eprintln!("usage: tessera nss-add-module DIR");
                return ExitCode::FAILURE;
            };
            return match nss::add_module_in_process(&PathBuf::from(dir)) {
                Ok(nss::Outcome::Added) => {
                    println!("added");
                    ExitCode::SUCCESS
                }
                Ok(nss::Outcome::AlreadyPresent) => {
                    println!("present");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("{e:#}");
                    ExitCode::FAILURE
                }
            };
        }
        _ => {}
    }
    ui::run()
}
