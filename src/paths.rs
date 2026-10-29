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

/// Name under which Tessera registers the client module in NSS databases.
pub const NSS_MODULE_NAME: &str = "Tessera (p11-kit)";

fn xdg(var: &str, fallback: &str) -> PathBuf {
    match env::var_os(var) {
        Some(v) if !v.is_empty() => PathBuf::from(v),
        _ => home().join(fallback),
    }
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

/// The `p11-kit` command. The runtime ships it in /usr/bin.
pub fn p11kit_command() -> PathBuf {
    env::var_os("TESSERA_P11KIT")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("p11-kit"))
}

/// Papers reads its signing certificates through poppler, which looks first
/// for a Firefox-style profile under $XDG_CONFIG_HOME/mozilla/firefox. In the
/// Papers Flatpak that is inside its own data directory, so a small NSS
/// database there is the one Papers will open.
pub fn papers_nss_dir() -> PathBuf {
    home()
        .join(".var/app/org.gnome.Papers/config/mozilla/firefox")
        .join("tessera.default")
}

/// Where the Firefox Flatpak keeps its profiles (Firefox 147+ uses
/// .config/mozilla/firefox; older releases .mozilla/firefox).
pub fn firefox_profile_roots() -> Vec<PathBuf> {
    let base = home().join(".var/app/org.mozilla.firefox");
    vec![
        base.join(".config/mozilla/firefox"),
        base.join(".mozilla/firefox"),
    ]
}
