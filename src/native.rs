//! Native apps: the host's own Firefox, Chromium-based browsers and PDF
//! viewers (installed from the distribution, not as Flatpaks).
//!
//! A native app needs no system connection. It loads p11-kit-proxy.so,
//! which reads ~/.config/pkcs11/modules and so reaches Tessera's servers
//! directly. All it needs is p11-kit-proxy.so in the NSS database it opens:
//!
//! - Firefox: every profile (profiles.ini under ~/.config/mozilla/firefox,
//!   Firefox 147+, and ~/.mozilla/firefox).
//! - Chromium, Chrome, Edge and Brave share one database: ~/.pki/nssdb if
//!   it exists, otherwise (Chromium 146+) $XDG_DATA_HOME/pki/nssdb.
//! - Okular, Papers and Evince sign through poppler, which opens the most
//!   recent *default* Firefox profile that has a cert9.db, first under
//!   $XDG_CONFIG_HOME/mozilla/firefox, then ~/.mozilla/firefox, and
//!   otherwise ~/.pki/nssdb.
//!
//! On Fedora, nss-sysinit may already load p11-kit's modules; registering
//! the proxy there too would show each token twice, so a database that uses
//! sysinit is reported as "probably ready".

use std::path::PathBuf;

use crate::apps;
use crate::i18n::t;
use crate::nss;
use crate::paths;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Firefox,
    Chromium,
    Poppler,
}

pub struct NativeApp {
    pub name: &'static str,
    /// The icon name the distributions use, and a generic one.
    pub icon: &'static str,
    pub fallback_icon: &'static str,
    pub kind: Kind,
}

pub const FIREFOX: NativeApp = NativeApp {
    name: "Firefox",
    icon: "firefox",
    fallback_icon: "web-browser-symbolic",
    kind: Kind::Firefox,
};
pub const CHROMIUM_BROWSERS: NativeApp = NativeApp {
    name: "Chromium-based browsers",
    icon: "chromium",
    fallback_icon: "web-browser-symbolic",
    kind: Kind::Chromium,
};
pub const PDF_VIEWERS: NativeApp = NativeApp {
    name: "PDF viewers (Okular, Papers, Evince)",
    icon: "okular",
    fallback_icon: "x-office-document-symbolic",
    kind: Kind::Poppler,
};

pub const APPS: [&NativeApp; 3] = [&FIREFOX, &CHROMIUM_BROWSERS, &PDF_VIEWERS];

/// The Chromium-based browsers, by their folder under ~/.config.
const CHROMIUM_FAMILY: [(&str, &str); 4] = [
    ("Chromium", "chromium"),
    ("Google Chrome", "google-chrome"),
    ("Microsoft Edge", "microsoft-edge"),
    ("Brave", "BraveSoftware/Brave-Browser"),
];

fn firefox_roots() -> Vec<PathBuf> {
    vec![
        paths::host_config_home().join("mozilla/firefox"),
        paths::home().join(".mozilla/firefox"),
    ]
}

/// The native Firefox's profile folders.
pub fn firefox_profiles() -> Vec<PathBuf> {
    paths::profiles_in(&firefox_roots())
}

/// The Chromium-based browsers that have been used on this host.
pub fn chromium_browsers() -> Vec<&'static str> {
    let config = paths::host_config_home();
    CHROMIUM_FAMILY
        .iter()
        .filter(|(_, dir)| config.join(dir).is_dir())
        .map(|(name, _)| *name)
        .collect()
}

/// The database the native Chromium-based browsers share, once one of them
/// has made it.
pub fn chromium_nssdb() -> Option<PathBuf> {
    [
        paths::home().join(".pki/nssdb"),
        paths::host_data_home().join("pki/nssdb"),
    ]
    .into_iter()
    .find(|d| d.is_dir())
}

/// The Chromium Flatpak may read the real ~/.pki/nssdb too: p11-kit-proxy.so
/// there would make it list each token twice (it already sees them through
/// its root-certificates module), and ask for the PIN twice.
fn shared_with_chromium_flatpak(db: &std::path::Path) -> bool {
    db == paths::home().join(".pki/nssdb")
        && paths::home()
            .join(".var/app")
            .join(apps::CHROMIUM.id)
            .is_dir()
}

/// The databases poppler may open: every *default* Firefox profile with a
/// cert9.db (poppler takes the most recent one), or else ~/.pki/nssdb.
pub fn poppler_dbs() -> Vec<PathBuf> {
    let mut out = Vec::new();
    for root in firefox_roots() {
        let Ok(entries) = std::fs::read_dir(&root) else {
            continue;
        };
        let mut dirs: Vec<PathBuf> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                p.join("cert9.db").is_file()
                    && p.file_name()
                        .is_some_and(|n| n.to_string_lossy().contains("default"))
            })
            .collect();
        dirs.sort();
        out.extend(dirs);
    }
    if out.is_empty() {
        let pki = paths::home().join(".pki/nssdb");
        if pki.is_dir() {
            out.push(pki);
        }
    }
    out
}

/// Is the app on this host (and so worth a row)?
pub fn present(app: &NativeApp) -> bool {
    match app.kind {
        Kind::Firefox => !firefox_profiles().is_empty(),
        Kind::Chromium => !chromium_browsers().is_empty() || chromium_nssdb().is_some(),
        // Not detectable from here (no folder of their own): always shown.
        Kind::Poppler => true,
    }
}

/// The databases the app opens; empty until there is one.
fn databases(app: &NativeApp) -> Vec<PathBuf> {
    match app.kind {
        Kind::Firefox => firefox_profiles(),
        Kind::Chromium => chromium_nssdb().into_iter().collect(),
        Kind::Poppler => poppler_dbs(),
    }
}

pub enum State {
    Ready,
    /// nss-sysinit may already bring in p11-kit's modules.
    ProbablyReady,
    ToDo,
    /// No database yet: the app has to be opened once.
    NoDatabase,
    /// The database is the Chromium Flatpak's too.
    SharedWithFlatpak,
}

pub fn state(app: &NativeApp) -> State {
    let dbs = databases(app);
    if dbs.is_empty() {
        return State::NoDatabase;
    }
    if dbs.iter().all(|d| nss::has_library(d, paths::PROXY_MODULE)) {
        return State::Ready;
    }
    if dbs.iter().any(|d| shared_with_chromium_flatpak(d)) {
        return State::SharedWithFlatpak;
    }
    if dbs.iter().all(|d| nss::uses_sysinit(d)) {
        return State::ProbablyReady;
    }
    State::ToDo
}

/// One line about the app: what it is and what it still needs.
pub fn subtitle(app: &NativeApp) -> String {
    let what = match app.kind {
        Kind::Chromium => {
            let names = chromium_browsers();
            if names.is_empty() {
                t("Chromium, Chrome, Edge, Brave").to_string()
            } else {
                names.join(", ")
            }
        }
        _ => t("Native app").to_string(),
    };
    let state = match state(app) {
        State::Ready => t("Ready"),
        State::ProbablyReady => {
            t("Probably ready: the system's NSS loads p11-kit (nss-sysinit). Set up only if the tokens do not appear")
        }
        State::ToDo => t("Set-up missing"),
        State::NoDatabase => match app.kind {
            Kind::Poppler => t("No certificate database yet: these viewers read the default Firefox profile's, or else ~/.pki/nssdb. Open Firefox or a Chromium-based browser once"),
            _ => t("Open the app once, then come back"),
        },
        State::SharedWithFlatpak => t("Not available: the Chromium Flatpak uses the same certificate database (~/.pki/nssdb) and would see each token twice"),
    };
    format!("{what} · {state}")
}

/// Does the row offer "Set up"?
pub fn can_set_up(app: &NativeApp) -> bool {
    matches!(state(app), State::ToDo | State::ProbablyReady)
}

/// Register p11-kit-proxy.so where the app looks (in a child process, see
/// nss.rs). Returns a line for the user.
pub fn set_up(app: &NativeApp) -> Result<String, String> {
    let dbs = databases(app);
    if dbs.is_empty() {
        return Err(subtitle(app));
    }
    if dbs.iter().any(|d| shared_with_chromium_flatpak(d)) {
        return Err(subtitle(app));
    }
    if dbs.iter().any(|d| nss::profile_in_use(d)) {
        return Err(t("Close Firefox first, then try again").into());
    }
    for d in &dbs {
        nss::add_library(d, paths::PROXY_MODULE).map_err(|e| format!("{}: {e:#}", d.display()))?;
    }
    Ok(format!("{}: {}", t(app.name), t("set up; restart the app")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// The native apps' states, on a made-up home folder. One test, because
    /// it changes HOME and the XDG variables for the whole process.
    #[test]
    fn native_apps_are_found_and_their_state_read() {
        let home = std::env::temp_dir().join(format!("tessera-native-{}", std::process::id()));
        let _ = fs::remove_dir_all(&home);
        std::env::set_var("HOME", &home);
        std::env::set_var("XDG_CONFIG_HOME", home.join(".config"));
        std::env::set_var("XDG_DATA_HOME", home.join(".local/share"));

        // Nothing installed: only the PDF viewers' row, with no database.
        assert!(!present(&FIREFOX));
        assert!(!present(&CHROMIUM_BROWSERS));
        assert!(present(&PDF_VIEWERS));
        assert!(matches!(state(&PDF_VIEWERS), State::NoDatabase));

        // Firefox with two profiles; poppler may open either *default* one.
        let ff = home.join(".mozilla/firefox");
        for p in ["abc.default", "xyz.default-release", "other"] {
            fs::create_dir_all(ff.join(p)).unwrap();
        }
        fs::write(
            ff.join("profiles.ini"),
            "[Profile0]\nPath=abc.default\nIsRelative=1\n\n[Profile1]\nPath=xyz.default-release\nIsRelative=1\n",
        )
        .unwrap();
        assert!(present(&FIREFOX));
        assert_eq!(firefox_profiles().len(), 2);
        // poppler only takes a profile that has a database.
        assert!(poppler_dbs().is_empty());
        for p in ["abc.default", "xyz.default-release", "other"] {
            fs::write(ff.join(p).join("cert9.db"), b"").unwrap();
        }
        assert_eq!(poppler_dbs().len(), 2);
        assert!(matches!(state(&FIREFOX), State::ToDo));
        assert!(can_set_up(&FIREFOX));
        let proxy = "\nlibrary=/usr/lib64/p11-kit-proxy.so\nname=Tessera (p11-kit)\n";
        fs::write(ff.join("abc.default/pkcs11.txt"), proxy).unwrap();
        assert!(matches!(state(&FIREFOX), State::ToDo));
        fs::write(ff.join("xyz.default-release/pkcs11.txt"), proxy).unwrap();
        assert!(matches!(state(&FIREFOX), State::Ready));
        assert!(matches!(state(&PDF_VIEWERS), State::Ready));
        assert!(!can_set_up(&FIREFOX));

        // Chromium-based browsers: found by their folder, sharing ~/.pki/nssdb.
        fs::create_dir_all(home.join(".config/google-chrome")).unwrap();
        assert!(present(&CHROMIUM_BROWSERS));
        assert_eq!(chromium_browsers(), vec!["Google Chrome"]);
        assert!(matches!(state(&CHROMIUM_BROWSERS), State::NoDatabase));
        let db = home.join(".local/share/pki/nssdb");
        fs::create_dir_all(&db).unwrap();
        assert_eq!(chromium_nssdb(), Some(db.clone()));
        fs::write(db.join("pkcs11.txt"), "library=libnsssysinit.so\n").unwrap();
        assert!(matches!(state(&CHROMIUM_BROWSERS), State::ProbablyReady));
        // The older ~/.pki/nssdb wins, and the Chromium Flatpak reads it too.
        fs::create_dir_all(home.join(".pki/nssdb")).unwrap();
        fs::create_dir_all(home.join(".var/app/org.chromium.Chromium")).unwrap();
        assert_eq!(chromium_nssdb(), Some(home.join(".pki/nssdb")));
        assert!(matches!(
            state(&CHROMIUM_BROWSERS),
            State::SharedWithFlatpak
        ));
        assert!(!can_set_up(&CHROMIUM_BROWSERS));
        assert!(set_up(&CHROMIUM_BROWSERS).is_err());

        // Outside Flatpak, starting at login is a file in ~/.config/autostart.
        let autostart = home.join(".config/autostart/io.github.pedrohqb.Tessera.desktop");
        paths::set_host_autostart(true).unwrap();
        assert!(fs::read_to_string(&autostart)
            .unwrap()
            .contains("--background"));
        paths::set_host_autostart(false).unwrap();
        assert!(!autostart.exists());
        paths::set_host_autostart(false).unwrap();

        let _ = fs::remove_dir_all(&home);
    }
}
