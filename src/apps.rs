//! The host side and the client apps.
//!
//! Host: one file per driver in ~/.config/pkcs11/modules,
//!
//!     remote: unix:path=/run/user/<uid>/tessera/<driver>
//!
//! makes the host's p11-kit (its proxy, and its server behind
//! `p11-kit-server.socket`) serve Tessera's tokens next to the host's own
//! modules. Native apps that use p11-kit-proxy.so see them directly.
//!
//! Sandboxed apps: `flatpak override --user
//! --filesystem=xdg-run/p11-kit/pkcs11 <app>` puts the host's server socket
//! where p11-kit-client.so looks for it, in place of Flatpak's trust-store
//! server (the host server serves the host's trust store too).

use anyhow::{Context, Result};
use std::fs;

use crate::drivers::Driver;
use crate::paths;

pub struct ClientApp {
    pub id: &'static str,
    pub name: &'static str,
}

pub const FIREFOX: ClientApp = ClientApp {
    id: "org.mozilla.firefox",
    name: "Firefox",
};
pub const PAPERS: ClientApp = ClientApp {
    id: "org.gnome.Papers",
    name: "Papers",
};
pub const PJEOFFICE: ClientApp = ClientApp {
    id: "br.jus.cnj.PJeOfficePro",
    name: "PJeOffice Pro",
};

/// The one-time host command that starts the host's p11-kit server.
pub const HOST_SERVER_COMMAND: &str = "systemctl --user enable --now p11-kit-server.socket";

/// The command that lets a sandboxed app reach the host's p11-kit server.
pub fn override_command(app_id: &str) -> String {
    format!("flatpak override --user --filesystem=xdg-run/p11-kit/pkcs11 {app_id}")
}

/// Has `flatpak override --user` given this app the p11-kit socket?
pub fn has_socket_override(app_id: &str) -> bool {
    let Ok(text) = fs::read_to_string(paths::user_override(app_id)) else {
        return false;
    };
    text.lines()
        .filter_map(|l| l.trim().strip_prefix("filesystems="))
        .flat_map(|v| v.split(';'))
        .any(|f| f.trim() == "xdg-run/p11-kit/pkcs11")
}

/// Does the host's p11-kit server answer?
pub fn host_server_answers() -> bool {
    std::os::unix::net::UnixStream::connect(paths::host_p11kit_socket()).is_ok()
}

fn host_module_name(id: &str) -> String {
    format!("tessera-{id}.module")
}

fn host_module_contents(d: &Driver) -> String {
    format!(
        "# Written by Tessera ({}): {}, served from its sandbox.\nremote: unix:path={}\n",
        paths::APP_ID,
        d.name,
        paths::driver_socket(&d.id).display()
    )
}

/// Write one host module file per running driver and remove Tessera's stale
/// ones. Returns how many are in place.
pub fn write_host_modules(drivers: &[Driver], running: &[String]) -> Result<usize> {
    let dir = paths::host_modules_dir();
    fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
    let wanted: Vec<&Driver> = drivers.iter().filter(|d| running.contains(&d.id)).collect();
    for entry in fs::read_dir(&dir)?.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with("tessera-")
            && name.ends_with(".module")
            && !wanted.iter().any(|d| host_module_name(&d.id) == name)
        {
            let _ = fs::remove_file(entry.path());
        }
    }
    for d in &wanted {
        let path = dir.join(host_module_name(&d.id));
        let body = host_module_contents(d);
        if fs::read_to_string(&path).ok().as_deref() != Some(body.as_str()) {
            fs::write(&path, body).with_context(|| format!("writing {}", path.display()))?;
        }
    }
    Ok(wanted.len())
}
