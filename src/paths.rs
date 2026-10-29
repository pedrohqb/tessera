//! Where Tessera finds and keeps things.
//!
//! Inside the Flatpak sandbox the XDG variables point into
//! ~/.var/app/io.github.pedrohqb.Tessera, and $XDG_RUNTIME_DIR is the same
//! directory inside and outside every sandbox, which is what lets other apps
//! reach our socket.
//!
//! The socket is not the p11-kit default ($XDG_RUNTIME_DIR/p11-kit/pkcs11):
//! inside every sandbox Flatpak replaces that directory with its own p11-kit
//! server for the host's trust store, so it cannot be mounted there. Client
//! apps reach Tessera instead through a `remote:` line in their own p11-kit
//! configuration (see apps.rs).

use std::env;
use std::path::PathBuf;

pub const APP_ID: &str = "io.github.pedrohqb.Tessera";

/// The PKCS#11 module Tessera itself uses to look at its own server (with
/// P11_KIT_SERVER_ADDRESS pointing at our socket). It ships in the
/// freedesktop runtime, and so in the GNOME and KDE ones built on it.
pub const CLIENT_MODULE: &str = "/usr/lib/x86_64-linux-gnu/pkcs11/p11-kit-client.so";

/// The PKCS#11 module client apps load: p11-kit's proxy, which serves every
/// module in the app's p11-kit configuration, including the `remote:` entry
/// Tessera writes there. Same path in every freedesktop-based runtime.
pub const PROXY_MODULE: &str = "/usr/lib/x86_64-linux-gnu/p11-kit-proxy.so";

/// Name under which Tessera registers the proxy module in NSS databases.
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

/// Tessera's socket: $XDG_RUNTIME_DIR/tessera/pkcs11, the same path on the
/// host and in every sandbox granted `xdg-run/tessera`.
pub fn socket_path() -> PathBuf {
    if let Some(p) = env::var_os("TESSERA_SOCKET") {
        return PathBuf::from(p);
    }
    runtime_dir().join("tessera").join("pkcs11")
}

/// The p11-kit address of our socket, as clients write it.
pub fn server_address() -> String {
    format!("unix:path={}", socket_path().display())
}

/// The p11-kit module file Tessera writes into a client app's configuration
/// ($XDG_CONFIG_HOME of a Flatpak app is ~/.var/app/<id>/config).
pub fn app_module_file(app_id: &str) -> PathBuf {
    home()
        .join(".var/app")
        .join(app_id)
        .join("config/pkcs11/modules/tessera.module")
}

/// A private p11-kit configuration, given to the server process only as its
/// XDG_CONFIG_HOME. It never touches the user's own ~/.config/pkcs11, even when
/// Tessera runs outside Flatpak during development.
pub fn p11kit_config_home() -> PathBuf {
    xdg("XDG_DATA_HOME", ".local/share")
        .join("tessera")
        .join("p11-kit")
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
