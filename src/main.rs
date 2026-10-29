//! Tessera: a PKCS#11 token hub for Flatpak.
//!
//! Usage:
//!   tessera                     open the window (and start the token service)
//!   tessera --background        start the token service without a window
//!   tessera serve               run only the token service, in the foreground
//!   tessera status              print readers, tokens and drivers
//!   tessera setup-app ID [DIR]  point a Flatpak app at Tessera (and register
//!                               the module in the NSS database DIR)
//!   tessera nss-add-module DIR  (internal) register p11-kit-client.so in DIR

mod apps;
mod drivers;
mod i18n;
mod nss;
mod paths;
mod server;
mod settings;
mod status;
mod ui;

use std::path::PathBuf;
use std::process::ExitCode;

fn print_status() -> ExitCode {
    println!("Socket: {}", paths::socket_path().display());
    println!(
        "Service: {}",
        if server::socket_answers() {
            "answering"
        } else {
            "not running"
        }
    );
    println!("\nDrivers:");
    let ds = drivers::discover();
    if ds.is_empty() {
        println!("  (none)");
    }
    for d in &ds {
        println!(
            "  {} [{}] {}{}",
            d.name,
            d.id,
            d.module.display(),
            if d.available() { "" } else { "  (missing)" }
        );
    }
    let snap = status::snapshot();
    println!("\nReaders:");
    match &snap.readers {
        status::Readers::Ok(rs) if rs.is_empty() => println!("  (none)"),
        status::Readers::Ok(rs) => {
            for r in rs {
                println!(
                    "  {}{}",
                    r.name,
                    if r.card_present {
                        "  [card present]"
                    } else {
                        ""
                    }
                );
            }
        }
        status::Readers::NoService(e) => println!("  pcscd not reachable: {e}"),
    }
    println!("\nTokens:");
    match &snap.tokens {
        Ok(ts) if ts.is_empty() => println!("  (none)"),
        Ok(ts) => {
            for t in ts {
                println!(
                    "  {} ({} {}, serial {})",
                    t.label, t.manufacturer, t.model, t.serial
                );
                for c in &t.certificates {
                    println!(
                        "    - {} | issuer: {} | valid until {}{}",
                        c.subject,
                        c.issuer,
                        c.not_after,
                        if c.expired { " (expired)" } else { "" }
                    );
                }
            }
        }
        Err(e) => println!("  error: {e}"),
    }
    ExitCode::SUCCESS
}

fn serve() -> ExitCode {
    let ds = drivers::discover();
    let mut srv = server::Server::new();
    if let Err(e) = srv.start(&ds) {
        if e.to_string() == "conflict" {
            eprintln!(
                "tessera: another p11-kit server already answers on {}",
                paths::socket_path().display()
            );
            return ExitCode::FAILURE;
        }
        eprintln!("tessera: cannot start the token service: {e:#}");
        return ExitCode::FAILURE;
    }
    eprintln!(
        "tessera: serving {} driver(s) on {}",
        ds.iter().filter(|d| d.available()).count(),
        paths::socket_path().display()
    );
    loop {
        std::thread::sleep(std::time::Duration::from_secs(1));
        match srv.poll() {
            server::State::Running => {}
            other => {
                eprintln!("tessera: service stopped: {other:?}");
                return ExitCode::FAILURE;
            }
        }
    }
}

extern "C" {
    fn signal(signum: i32, handler: usize) -> usize;
}

fn main() -> ExitCode {
    // Tessera looks at its own server through p11-kit-client.so. Inside a
    // Flatpak sandbox the client's default address is Flatpak's trust-store
    // server, so point it at our socket (before any thread starts).
    std::env::set_var("P11_KIT_SERVER_ADDRESS", paths::server_address());
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
        Some("setup-app") => {
            // tessera setup-app APP_ID [NSS_DIR]: what the window's "Set up"
            // does, for scripts and testing.
            let Some(app_id) = args.get(2) else {
                eprintln!("usage: tessera setup-app APP_ID [NSS_DIR]");
                return ExitCode::FAILURE;
            };
            if let Err(e) = apps::write_module_file(app_id) {
                eprintln!("tessera: {e:#}");
                return ExitCode::FAILURE;
            }
            println!("{}", paths::app_module_file(app_id).display());
            if let Some(dir) = args.get(3) {
                if let Err(e) = nss::add_module(&PathBuf::from(dir)) {
                    eprintln!("tessera: {e:#}");
                    return ExitCode::FAILURE;
                }
                println!("{dir}: {} registered", paths::PROXY_MODULE);
            }
            println!("Now run: {}", apps::override_command(app_id));
            return ExitCode::SUCCESS;
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
