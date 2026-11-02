//! The host side and the client apps.
//!
//! Host: one file per driver in ~/.config/pkcs11/modules,
//!
//!     remote: unix:path=/run/user/<uid>/tessera/<driver>
//!
//! makes the host's p11-kit (its proxy, and its server behind
//! `p11-kit-server.socket`, or on Debian a plain user service) serve Tessera's tokens next to the host's own
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

/// How an app is set up once it may reach the host's p11-kit server.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Setup {
    /// Register p11-kit-client.so in every Firefox profile.
    Firefox,
    /// Signs through poppler: an empty NSS database of its own, in the
    /// first place poppler looks (NSS's roots module, p11-kit in the
    /// runtime, already reaches the server).
    Poppler,
    /// Register p11-kit-client.so in the browser's ~/.pki/nssdb.
    Chromium,
    /// Nothing: the app's package already does it (PJeOffice Pro).
    None,
}

pub struct ClientApp {
    pub id: &'static str,
    pub name: &'static str,
    /// A generic icon, for when the app's own is not available.
    pub icon: &'static str,
    /// Tessera's copy of the app's icon (data/app-icons), when its license
    /// allows one.
    pub bundled_icon: Option<&'static str>,
    pub setup: Setup,
}

pub const FIREFOX: ClientApp = ClientApp {
    id: "org.mozilla.firefox",
    name: "Firefox",
    icon: "web-browser-symbolic",
    bundled_icon: None,
    setup: Setup::Firefox,
};
pub const CHROMIUM: ClientApp = ClientApp {
    id: "org.chromium.Chromium",
    name: "Chromium",
    icon: "web-browser-symbolic",
    bundled_icon: Some("tessera-app-chromium"),
    setup: Setup::Chromium,
};
pub const CHROME: ClientApp = ClientApp {
    id: "com.google.Chrome",
    name: "Google Chrome",
    icon: "web-browser-symbolic",
    bundled_icon: None,
    setup: Setup::Chromium,
};
pub const EDGE: ClientApp = ClientApp {
    id: "com.microsoft.Edge",
    name: "Microsoft Edge",
    icon: "web-browser-symbolic",
    bundled_icon: None,
    setup: Setup::Chromium,
};
pub const BRAVE: ClientApp = ClientApp {
    id: "com.brave.Browser",
    name: "Brave",
    icon: "web-browser-symbolic",
    bundled_icon: None,
    setup: Setup::Chromium,
};
pub const PAPERS: ClientApp = ClientApp {
    id: "org.gnome.Papers",
    name: "Papers",
    icon: "x-office-document-symbolic",
    bundled_icon: Some("tessera-app-papers"),
    setup: Setup::Poppler,
};
/// Okular signs through poppler too (KDE runtime, same freedesktop base).
pub const OKULAR: ClientApp = ClientApp {
    id: "org.kde.okular",
    name: "Okular",
    icon: "x-office-document-symbolic",
    bundled_icon: Some("tessera-app-okular"),
    setup: Setup::Poppler,
};
pub const PJEOFFICE: ClientApp = ClientApp {
    id: "br.jus.cnj.PJeOfficePro",
    name: "PJeOffice Pro",
    icon: "document-edit-symbolic",
    bundled_icon: None,
    setup: Setup::None,
};

/// The apps shown in the window, in order.
pub const APPS: [&ClientApp; 7] = [
    &FIREFOX, &CHROMIUM, &CHROME, &EDGE, &BRAVE, &PAPERS, &OKULAR,
];

/// The one-time host command that starts the host's p11-kit server.
const SOCKET_COMMAND: &str = "systemctl --user enable --now p11-kit-server.socket";

/// The same on Debian and its derivatives (Ubuntu, Mint), which ship the
/// server (`p11-kit server`) but no systemd units for it, and build it
/// without socket activation: a plain user service, started at login, that
/// creates the socket itself where the other distributions' socket unit
/// puts it. Written by printf, one argument per line, so that systemd's `%t`
/// (the runtime folder) reaches the file as it is.
const DEBIAN_COMMAND: &str = concat!(
    "mkdir -p ~/.config/systemd/user && printf '%s\\n' ",
    "'[Unit]' 'Description=p11-kit server' 'Documentation=man:p11-kit(8)' ",
    "'[Service]' 'ExecStartPre=/bin/mkdir -p -m 700 %t/p11-kit' ",
    "'ExecStart=/usr/bin/p11-kit server -f -n %t/p11-kit/pkcs11 pkcs11:' ",
    "'Restart=on-failure' '[Install]' 'WantedBy=default.target' ",
    "> ~/.config/systemd/user/p11-kit-server.service && ",
    "systemctl --user daemon-reload && ",
    "systemctl --user enable --now p11-kit-server.service"
);

/// The command for this host (see SOCKET_COMMAND and DEBIAN_COMMAND).
pub fn host_server_command() -> &'static str {
    server_command_for(crate::native::host_is_debian_family())
}

fn server_command_for(debian: bool) -> &'static str {
    if debian {
        DEBIAN_COMMAND
    } else {
        SOCKET_COMMAND
    }
}

/// Is the command too long to show in the window (it is copied instead)?
pub fn host_server_command_is_long() -> bool {
    crate::native::host_is_debian_family()
}

/// The command that lets a sandboxed app reach the host's p11-kit server.
pub fn override_command(app: &ClientApp) -> String {
    format!(
        "flatpak override --user --filesystem=xdg-run/p11-kit/pkcs11 {}",
        app.id
    )
}

/// The NSS database a Chromium-based browser reads: since Chromium 146,
/// $XDG_DATA_HOME/pki/nssdb (in Flatpak, ~/.var/app/<id>/data/pki/nssdb),
/// unless the older ~/.pki/nssdb exists, which then wins. ~/.pki is the real
/// one for Chromium, which may use the home folder, and the one kept by
/// --persist=.pki for the others. None until the browser has made one
/// (Tessera sees the folder only if it existed when Tessera started).
pub fn chromium_nssdb(app: &ClientApp) -> Option<std::path::PathBuf> {
    let data = paths::home().join(".var/app").join(app.id);
    let legacy = if app.id == CHROMIUM.id {
        paths::home().join(".pki/nssdb")
    } else {
        data.join(".pki/nssdb")
    };
    [legacy, data.join("data/pki/nssdb")]
        .into_iter()
        .find(|d| d.is_dir())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_command_by_distribution() {
        assert_eq!(server_command_for(false), SOCKET_COMMAND);
        assert_eq!(server_command_for(true), DEBIAN_COMMAND);
    }

    /// The Debian command writes the unit systemd expects, with `%t` intact
    /// (the systemctl part is left out: there is no user systemd here).
    #[test]
    fn debian_command_writes_the_service() {
        let home = std::env::temp_dir().join(format!("tessera-unit-{}", std::process::id()));
        let _ = fs::remove_dir_all(&home);
        fs::create_dir_all(&home).unwrap();
        let write = DEBIAN_COMMAND.split(" && systemctl").next().unwrap();
        let ok = std::process::Command::new("sh")
            .arg("-c")
            .arg(write)
            .env("HOME", &home)
            .status()
            .unwrap()
            .success();
        assert!(ok);
        let unit =
            fs::read_to_string(home.join(".config/systemd/user/p11-kit-server.service")).unwrap();
        assert_eq!(
            unit,
            "[Unit]\nDescription=p11-kit server\nDocumentation=man:p11-kit(8)\n\
             [Service]\nExecStartPre=/bin/mkdir -p -m 700 %t/p11-kit\n\
             ExecStart=/usr/bin/p11-kit server -f -n %t/p11-kit/pkcs11 pkcs11:\n\
             Restart=on-failure\n[Install]\nWantedBy=default.target\n"
        );
        let _ = fs::remove_dir_all(&home);
    }
}
