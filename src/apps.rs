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
    /// Register p11-kit-client.so in every profile listed by the
    /// profiles.ini in these folders of the app's own (~/.var/app/<id>/…):
    /// Firefox and Thunderbird.
    Profiles(&'static [&'static str]),
    /// Signs through poppler: an empty NSS database of its own, in the
    /// first place poppler looks (NSS's roots module, p11-kit in the
    /// runtime, already reaches the server).
    Poppler,
    /// Register p11-kit-client.so in the browser's ~/.pki/nssdb.
    Chromium,
    /// An empty NSS database of its own, named in its settings (CertDir):
    /// see libreoffice.rs.
    LibreOffice,
    /// p11-kit-client.so and the slot of Tessera's tokens in its pkcs11.cfg:
    /// see jsignpdf.rs.
    JSignPdf,
    /// p11-kit-client.so as the PKCS#11 provider in its settings file: see
    /// masterpdf.rs.
    MasterPdf,
    /// By hand in the app's settings: the row shows where, and the module's
    /// path.
    ByHand,
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
    /// Where, in the app, the module can be loaded by hand (shown with
    /// p11-kit-client.so's path).
    pub by_hand: Option<&'static str>,
}

pub const FIREFOX: ClientApp = ClientApp {
    id: "org.mozilla.firefox",
    name: "Firefox",
    icon: "web-browser-symbolic",
    bundled_icon: None,
    // Firefox 147+ keeps its profiles under .config, older releases under
    // .mozilla.
    setup: Setup::Profiles(&["config/mozilla/firefox", ".mozilla/firefox"]),
    by_hand: Some("Or by hand in Firefox: Settings → Privacy & Security → Security Devices → Load, module file:"),
};
/// Thunderbird's Flatpak, built by Mozilla. Its ESR has an ID of its own,
/// and the old ID (org.mozilla.Thunderbird) stopped being updated in 2026;
/// those two show only when installed. Mozilla's builds carry their own NSS,
/// which does not load the runtime's p11-kit by itself: like Firefox, the
/// module goes into every profile.
pub const THUNDERBIRD: ClientApp = ClientApp {
    id: "org.mozilla.thunderbird",
    name: "Thunderbird",
    icon: "mail-send-symbolic",
    bundled_icon: None,
    setup: Setup::Profiles(THUNDERBIRD_ROOTS),
    by_hand: Some(THUNDERBIRD_BY_HAND),
};
pub const THUNDERBIRD_ESR: ClientApp = ClientApp {
    id: "org.mozilla.thunderbird_esr",
    name: "Thunderbird ESR",
    ..THUNDERBIRD
};
pub const THUNDERBIRD_OLD: ClientApp = ClientApp {
    id: "org.mozilla.Thunderbird",
    name: "Thunderbird",
    ..THUNDERBIRD
};
const THUNDERBIRD_BY_HAND: &str =
    "Or by hand in Thunderbird: Settings → Privacy & Security → Security Devices → Load, module file:";
/// Evolution's Flatpak reads the real ~/.pki/nssdb when there is one, else
/// its own data/pki/nssdb (evolution-data-server 3.62). When it starts its
/// S/MIME certificates it adds NSS's root-certificates module, which in the
/// runtime is p11-kit's trust module: as for Chromium, step 1 is enough.
pub const EVOLUTION: ClientApp = ClientApp {
    id: "org.gnome.Evolution",
    name: "Evolution",
    icon: "mail-send-symbolic",
    bundled_icon: None,
    setup: Setup::Chromium,
    by_hand: None,
};
pub const CHROMIUM: ClientApp = ClientApp {
    id: "org.chromium.Chromium",
    name: "Chromium",
    icon: "web-browser-symbolic",
    bundled_icon: Some("tessera-app-chromium"),
    setup: Setup::Chromium,
    by_hand: None,
};
pub const CHROME: ClientApp = ClientApp {
    id: "com.google.Chrome",
    name: "Google Chrome",
    icon: "web-browser-symbolic",
    bundled_icon: None,
    setup: Setup::Chromium,
    by_hand: None,
};
pub const EDGE: ClientApp = ClientApp {
    id: "com.microsoft.Edge",
    name: "Microsoft Edge",
    icon: "web-browser-symbolic",
    bundled_icon: None,
    setup: Setup::Chromium,
    by_hand: None,
};
pub const BRAVE: ClientApp = ClientApp {
    id: "com.brave.Browser",
    name: "Brave",
    icon: "web-browser-symbolic",
    bundled_icon: None,
    setup: Setup::Chromium,
    by_hand: None,
};
pub const PAPERS: ClientApp = ClientApp {
    id: "org.gnome.Papers",
    name: "Papers",
    icon: "x-office-document-symbolic",
    bundled_icon: Some("tessera-app-papers"),
    setup: Setup::Poppler,
    by_hand: None,
};
/// Okular signs through poppler too (KDE runtime, same freedesktop base).
pub const OKULAR: ClientApp = ClientApp {
    id: "org.kde.okular",
    name: "Okular",
    icon: "x-office-document-symbolic",
    bundled_icon: Some("tessera-app-okular"),
    setup: Setup::Poppler,
    by_hand: None,
};
pub const LIBREOFFICE: ClientApp = ClientApp {
    id: crate::libreoffice::FLATPAK_ID,
    name: "LibreOffice",
    icon: "x-office-document-symbolic",
    bundled_icon: None,
    setup: Setup::LibreOffice,
    by_hand: None,
};
pub const JSIGNPDF: ClientApp = ClientApp {
    id: crate::jsignpdf::FLATPAK_ID,
    name: "JSignPdf",
    icon: "document-edit-symbolic",
    bundled_icon: None,
    setup: Setup::JSignPdf,
    by_hand: None,
};
/// Master PDF Editor (Flathub, Qt 6 build) loads one PKCS#11 library,
/// chosen in Tools → Settings → Certificates → "PKCS#11 provider": see
/// masterpdf.rs. The row also shows where, for doing it by hand.
pub const MASTERPDF: ClientApp = ClientApp {
    id: crate::masterpdf::FLATPAK_ID,
    name: "Master PDF Editor",
    icon: "x-office-document-symbolic",
    bundled_icon: None,
    setup: Setup::MasterPdf,
    by_hand: Some(
        "Or by hand in Master PDF Editor: Tools → Settings → Certificates → PKCS#11 provider, file:",
    ),
};
pub const PJEOFFICE: ClientApp = ClientApp {
    id: "br.jus.cnj.PJeOfficePro",
    name: "PJeOffice Pro",
    icon: "document-edit-symbolic",
    bundled_icon: None,
    setup: Setup::None,
    by_hand: None,
};
/// Assinador SERPRO (FlatPark): its package already has the p11-kit socket
/// and its own bridge to p11-kit-client.so (which hides the trust slots).
pub const ASSINADOR: ClientApp = ClientApp {
    id: "br.gov.serpro.desktop.assinador",
    name: "Assinador SERPRO",
    icon: "document-edit-symbolic",
    bundled_icon: None,
    setup: Setup::None,
    by_hand: None,
};

/// Where Thunderbird's Flatpak keeps its profiles, in its own folder
/// (--persist=.thunderbird; newer releases may use the XDG one).
const THUNDERBIRD_ROOTS: &[&str] = &[".thunderbird", "config/thunderbird"];

/// The apps with steps, shown in the window, in order.
pub const APPS: [&ClientApp; 14] = [
    &FIREFOX,
    &CHROMIUM,
    &CHROME,
    &EDGE,
    &BRAVE,
    &THUNDERBIRD,
    &THUNDERBIRD_ESR,
    &THUNDERBIRD_OLD,
    &EVOLUTION,
    &PAPERS,
    &OKULAR,
    &LIBREOFFICE,
    &JSIGNPDF,
    &MASTERPDF,
];

/// Is the app's row shown? Thunderbird's ESR and old ID only when they are
/// installed.
pub fn shown(app: &ClientApp) -> bool {
    ![THUNDERBIRD_ESR.id, THUNDERBIRD_OLD.id].contains(&app.id) || paths::flatpak_installed(app.id)
}

/// The apps whose package already reaches the system's p11-kit server.
pub const NO_STEPS: [&ClientApp; 2] = [&PJEOFFICE, &ASSINADOR];

/// The profiles of an app that keeps Mozilla-style profiles in its own
/// folder.
pub fn app_profiles(app: &ClientApp) -> Vec<std::path::PathBuf> {
    match app.setup {
        Setup::Profiles(roots) => {
            let base = paths::home().join(".var/app").join(app.id);
            let roots: Vec<_> = roots.iter().map(|r| base.join(r)).collect();
            paths::profiles_in(&roots)
        }
        _ => vec![],
    }
}

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

/// The NSS database a Chromium-based browser (or Evolution) reads: since Chromium 146,
/// $XDG_DATA_HOME/pki/nssdb (in Flatpak, ~/.var/app/<id>/data/pki/nssdb),
/// unless the older ~/.pki/nssdb exists, which then wins. ~/.pki is the real
/// one for Chromium, which may use the home folder, and the one kept by
/// --persist=.pki for the others. None until the browser has made one
/// (Tessera sees the folder only if it existed when Tessera started).
pub fn chromium_nssdb(app: &ClientApp) -> Option<std::path::PathBuf> {
    let data = paths::home().join(".var/app").join(app.id);
    let legacy = if app.id == CHROMIUM.id || app.id == EVOLUTION.id {
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

/// A driver without a token is listed after every other module. Through
/// p11-kit's RPC, a module whose slot list is empty answers
/// C_GetSlotList(CK_TRUE, buffer, &0) with CKR_BUFFER_TOO_SMALL (the
/// server sends a NULL array for a 0-length one, which the client then
/// takes for a buffer too small). The system's p11-kit server walks the
/// modules in order with p11_kit_iter, which stops at the first error: every
/// module after an empty one of Tessera's was missing for the apps (OpenSC
/// with no card hid the GD Burti's token, say). p11-kit orders the modules by
/// `priority:`, highest first (0 when unset; its trust module has 1), then by
/// name; with -1 the empty ones come last, where they cut nothing off.
pub const EMPTY_PRIORITY: &str = "-1";

fn host_module_contents(d: &Driver, has_token: bool) -> String {
    let priority = if has_token {
        String::new()
    } else {
        format!("priority: {EMPTY_PRIORITY}\n")
    };
    format!(
        "# Written by Tessera ({}): {}, served from its sandbox.\nremote: unix:path={}\n{priority}",
        paths::APP_ID,
        d.name,
        paths::driver_socket(&d.id).display()
    )
}

/// Write one host module file per running driver and remove Tessera's stale
/// ones; those of `with_tokens` (ids) come first for the apps, the others
/// last (see EMPTY_PRIORITY). Returns how many are in place.
pub fn write_host_modules(
    drivers: &[Driver],
    running: &[String],
    with_tokens: &[String],
) -> Result<usize> {
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
        let body = host_module_contents(d, with_tokens.contains(&d.id));
        if fs::read_to_string(&path).ok().as_deref() != Some(body.as_str()) {
            fs::write(&path, body).with_context(|| format!("writing {}", path.display()))?;
        }
    }
    Ok(wanted.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A driver with a token keeps p11-kit's default place; one without
    /// goes after every other module (EMPTY_PRIORITY).
    #[test]
    fn empty_drivers_come_last() {
        let d = Driver {
            id: "opensc".into(),
            name: "OpenSC".into(),
            vendor: String::new(),
            description: String::new(),
            module: "/x.so".into(),
            tools: vec![],
            create_dirs: vec![],
            library_path: vec![],
            env: Default::default(),
            when_on: Default::default(),
        };
        let with = host_module_contents(&d, true);
        assert!(with.contains("remote: unix:path="));
        assert!(!with.contains("priority"));
        let without = host_module_contents(&d, false);
        assert!(without.ends_with("\npriority: -1\n"), "{without}");
    }

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
