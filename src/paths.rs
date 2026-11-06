//! Where Tessera finds and keeps things.
//!
//! Inside the Flatpak sandbox the XDG variables point into
//! ~/.var/app/io.github.pedrohqb.Tessera, and $XDG_RUNTIME_DIR is the same
//! directory inside and outside every sandbox, which is what lets other apps
//! reach our socket.
//!
//! How apps reach the tokens: Tessera runs one `p11-kit server` per driver,
//! on $XDG_RUNTIME_DIR/tessera/<driver>. On the host, a module file per
//! driver in ~/.config/pkcs11/modules (`remote: unix:path=...`) makes the
//! host's own p11-kit server (`p11-kit-server.socket`, on
//! $XDG_RUNTIME_DIR/p11-kit/pkcs11) serve them together with the host's
//! modules. Sandboxed apps reach that socket with
//! `--filesystem=xdg-run/p11-kit/pkcs11` and the runtime's p11-kit-client.so,
//! as FlatPark's PJeOffice Pro already does.
//!
//! Why not inside the sandbox? Flatpak writes `user-config: none` into every
//! sandbox's /etc/pkcs11/pkcs11.conf and replaces $XDG_RUNTIME_DIR/p11-kit
//! with its own trust-store server, so neither a configured multi-module
//! server nor a custom client configuration is possible there.

use std::env;
use std::path::PathBuf;

pub const APP_ID: &str = "io.github.pedrohqb.Tessera";

/// The PKCS#11 module apps load to reach the host's p11-kit server (and that
/// Tessera loads to look at its own servers). It ships in the freedesktop
/// runtime, and so in the GNOME and KDE ones built on it.
pub const CLIENT_MODULE: &str = "/usr/lib/x86_64-linux-gnu/pkcs11/p11-kit-client.so";

/// The module native apps load: p11-kit's proxy, which reads
/// ~/.config/pkcs11/modules and so reaches Tessera's servers directly. By
/// name, not path: Fedora keeps it in /usr/lib64, Debian in
/// /usr/lib/x86_64-linux-gnu, and dlopen finds it in either.
pub const PROXY_MODULE: &str = "p11-kit-proxy.so";

/// Name under which Tessera registers the client module in NSS databases.
pub const NSS_MODULE_NAME: &str = "Tessera (p11-kit)";

fn xdg(var: &str, fallback: &str) -> PathBuf {
    match env::var_os(var) {
        Some(v) if !v.is_empty() => PathBuf::from(v),
        _ => home().join(fallback),
    }
}

/// $XDG_DATA_HOME (in the Flatpak, Tessera's own data folder).
pub fn data_home() -> PathBuf {
    xdg("XDG_DATA_HOME", ".local/share")
}

pub fn home() -> PathBuf {
    env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/"))
}

pub fn runtime_dir() -> PathBuf {
    match env::var_os("XDG_RUNTIME_DIR") {
        Some(v) if !v.is_empty() => PathBuf::from(v),
        // SAFETY: getuid has no preconditions and cannot fail.
        _ => PathBuf::from(format!("/run/user/{}", unsafe { libc_getuid() })),
    }
}

extern "C" {
    #[link_name = "getuid"]
    fn libc_getuid() -> u32;
}

/// Where Tessera's servers listen: $XDG_RUNTIME_DIR/tessera, the same path
/// on the host and in Tessera's sandbox.
pub fn socket_dir() -> PathBuf {
    if let Some(p) = env::var_os("TESSERA_SOCKET_DIR") {
        return PathBuf::from(p);
    }
    runtime_dir().join("tessera")
}

/// The socket of one driver's server.
pub fn driver_socket(id: &str) -> PathBuf {
    socket_dir().join(id)
}

/// The host's p11-kit module directory (~/.config/pkcs11/modules on the
/// host; Tessera's own $XDG_CONFIG_HOME is inside its sandbox data).
pub fn host_modules_dir() -> PathBuf {
    if let Some(p) = env::var_os("TESSERA_HOST_MODULES") {
        return PathBuf::from(p);
    }
    home().join(".config/pkcs11/modules")
}

/// Logs of the driver servers.
pub fn log_dir() -> PathBuf {
    xdg("XDG_DATA_HOME", ".local/share")
        .join("tessera")
        .join("logs")
}

/// Where Tessera keeps its own settings.
pub fn settings_file() -> PathBuf {
    xdg("XDG_CONFIG_HOME", ".config")
        .join("tessera")
        .join("settings.toml")
}

/// Directories searched for driver descriptions (`<dir>/<id>/driver.toml`).
pub fn driver_dirs() -> Vec<PathBuf> {
    if let Some(p) = env::var_os("TESSERA_DRIVERS_PATH") {
        return env::split_paths(&p).collect();
    }
    vec![
        // Bundled with the app.
        PathBuf::from("/app/lib/tessera/drivers"),
        // Driver extensions (io.github.pedrohqb.Tessera.Driver.<Name>), each
        // mounted in its own subdirectory.
        PathBuf::from("/app/lib/tessera/extensions"),
    ]
}

/// p11-kit's server helper (what `p11-kit server` runs). It is started
/// directly so that P11_KIT_PRIVATEDIR only redirects the per-connection
/// `p11-kit-remote` to Tessera (see remote.rs).
pub fn p11kit_server_command() -> PathBuf {
    if let Some(p) = env::var_os("TESSERA_P11KIT_SERVER") {
        return PathBuf::from(p);
    }
    for c in [
        "/usr/libexec/p11-kit/p11-kit-server",
        "/usr/lib/x86_64-linux-gnu/p11-kit/p11-kit-server",
        "/usr/lib/p11-kit/p11-kit-server",
    ] {
        if std::path::Path::new(c).exists() {
            return PathBuf::from(c);
        }
    }
    PathBuf::from("/usr/libexec/p11-kit/p11-kit-server")
}

/// Papers and Okular sign through poppler, which looks first for a
/// Firefox-style profile under $XDG_CONFIG_HOME/mozilla/firefox. In a Flatpak
/// that is inside the app's own data folder, so a small NSS database there is
/// the one the app opens.
pub fn poppler_nss_dir(app_id: &str) -> PathBuf {
    home()
        .join(".var/app")
        .join(app_id)
        .join("config/mozilla/firefox/tessera.default")
}

/// The profile folders listed in each root's profiles.ini.
pub fn profiles_in(roots: &[PathBuf]) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for root in roots {
        let Ok(ini) = std::fs::read_to_string(root.join("profiles.ini")) else {
            continue;
        };
        let (mut path, mut relative) = (None::<String>, true);
        let mut flush = |path: &mut Option<String>, relative: bool| {
            if let Some(p) = path.take() {
                let dir = if relative {
                    root.join(&p)
                } else {
                    PathBuf::from(&p)
                };
                if dir.is_dir() && !out.contains(&dir) {
                    out.push(dir);
                }
            }
        };
        for line in ini.lines().map(str::trim) {
            if line.starts_with('[') {
                flush(&mut path, relative);
                relative = true;
            } else if let Some(v) = line.strip_prefix("Path=") {
                path = Some(v.to_string());
            } else if let Some(v) = line.strip_prefix("IsRelative=") {
                relative = v != "0";
            }
        }
        flush(&mut path, relative);
    }
    out
}

/// Is Tessera running as a Flatpak (rather than as a plain binary on the
/// host, as in development)?
pub fn in_flatpak() -> bool {
    std::path::Path::new("/.flatpak-info").exists()
}

/// One of the host's XDG folders. In a Flatpak, $XDG_CONFIG_HOME and
/// $XDG_DATA_HOME point into the app's own folder; Flatpak passes the host's
/// values as $HOST_XDG_CONFIG_HOME and $HOST_XDG_DATA_HOME when they are set.
fn host_xdg(var: &str, fallback: &str) -> PathBuf {
    let name = if in_flatpak() {
        format!("HOST_{var}")
    } else {
        var.to_string()
    };
    match env::var_os(name) {
        Some(v) if !v.is_empty() => PathBuf::from(v),
        _ => home().join(fallback),
    }
}

/// The host's ~/.config.
pub fn host_config_home() -> PathBuf {
    host_xdg("XDG_CONFIG_HOME", ".config")
}

/// The host's ~/.local/share.
pub fn host_data_home() -> PathBuf {
    host_xdg("XDG_DATA_HOME", ".local/share")
}

/// Is this Flatpak app installed, for the user or for the system? Its folder
/// in ~/.var/app counts too (it is there once the app has run). In Tessera's
/// sandbox the install folders are visible read-only (see the manifest).
pub fn flatpak_installed(id: &str) -> bool {
    [
        host_data_home().join("flatpak/app"),
        PathBuf::from("/var/lib/flatpak/app"),
        home().join(".var/app"),
    ]
    .iter()
    .any(|d| d.join(id).is_dir())
}

/// The host's p11-kit server socket, as `--filesystem=xdg-run/p11-kit/pkcs11`
/// shows it inside a sandbox.
pub fn host_p11kit_socket() -> PathBuf {
    runtime_dir().join("p11-kit/pkcs11")
}

/// `flatpak override --user` file for an app.
pub fn user_override(app_id: &str) -> PathBuf {
    home().join(".local/share/flatpak/overrides").join(app_id)
}

/// An app's icon from the AppStream data Flatpak keeps for each remote
/// (Flathub, FlatPark…), which has the icons of every app in the remote,
/// installed or not: what software centers show.
pub fn appstream_icon(app_id: &str) -> Option<PathBuf> {
    if !app_id.contains('.') {
        return None;
    }
    for base in [
        home().join(".local/share/flatpak/appstream"),
        PathBuf::from("/var/lib/flatpak/appstream"),
    ] {
        let Ok(remotes) = std::fs::read_dir(&base) else {
            continue;
        };
        let mut remotes: Vec<PathBuf> = remotes.flatten().map(|e| e.path()).collect();
        remotes.sort();
        for remote in remotes {
            for size in ["128x128", "64x64"] {
                let icon = remote
                    .join("x86_64/active/icons")
                    .join(size)
                    .join(format!("{app_id}.png"));
                if icon.is_file() {
                    return Some(icon);
                }
            }
        }
    }
    None
}
