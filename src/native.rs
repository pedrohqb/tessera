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
//! - PJeOffice Pro is Java and keeps its own list of PKCS#11 libraries in
//!   ~/.pjeoffice-pro/pjeoffice-pro.config (`list.a3=`, separated by `|`):
//!   there p11-kit-proxy.so goes by full path, which depends on the
//!   distribution. PJeOffice skips paths that do not exist and takes only
//!   names ending in a known extension (.so, not .so.0).
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
    PjeOffice,
}

pub struct NativeApp {
    pub name: &'static str,
    /// Icons to try, in order: names in the icon theme (the desktop's, a
    /// Flatpak's, Tessera's own copy) or Flatpak app IDs, whose icon may also
    /// come from the AppStream data; then a generic one.
    pub icons: &'static [&'static str],
    pub fallback_icon: &'static str,
    pub kind: Kind,
}

pub const FIREFOX: NativeApp = NativeApp {
    name: "Firefox",
    icons: &["firefox", "org.mozilla.firefox"],
    fallback_icon: "web-browser-symbolic",
    kind: Kind::Firefox,
};
/// Always Chromium's icon, whichever of these browsers are installed.
pub const CHROMIUM_BROWSERS: NativeApp = NativeApp {
    name: "Chromium-based browsers",
    icons: &["org.chromium.Chromium", "chromium", "tessera-app-chromium"],
    fallback_icon: "web-browser-symbolic",
    kind: Kind::Chromium,
};
pub const PDF_VIEWERS: NativeApp = NativeApp {
    name: "PDF viewers (Okular, Papers, Evince)",
    icons: &["org.kde.okular", "okular", "tessera-app-okular"],
    fallback_icon: "x-office-document-symbolic",
    kind: Kind::Poppler,
};
pub const PJEOFFICE: NativeApp = NativeApp {
    name: "PJeOffice Pro",
    icons: &["br.jus.cnj.PJeOfficePro", "pjeoffice-pro", "pjeoffice"],
    fallback_icon: "document-edit-symbolic",
    kind: Kind::PjeOffice,
};

pub const APPS: [&NativeApp; 4] = [&FIREFOX, &CHROMIUM_BROWSERS, &PDF_VIEWERS, &PJEOFFICE];

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

/// PJeOffice Pro's settings, which it writes when it closes.
pub fn pjeoffice_config() -> PathBuf {
    paths::home().join(".pjeoffice-pro/pjeoffice-pro.config")
}

/// p11-kit's proxy module on the host, where each family of distributions
/// keeps it: lib64 on Fedora, RHEL and openSUSE, the multiarch folder on
/// Debian and Ubuntu, /usr/lib elsewhere (Arch and its derivatives). From
/// the host's os-release (in a Flatpak, /run/host/os-release): a Flatpak
/// cannot look into the host's /usr. Only one path: on Arch /usr/lib64 is a
/// link to /usr/lib, and two paths to the same module would show each token
/// twice.
pub fn host_p11kit_library() -> &'static str {
    let release = ["/run/host/os-release", "/etc/os-release"]
        .iter()
        .find_map(|f| std::fs::read_to_string(f).ok())
        .unwrap_or_default();
    p11kit_library_for(&release)
}

fn p11kit_library_for(os_release: &str) -> &'static str {
    let ids: Vec<String> = os_release
        .lines()
        .filter_map(|l| l.strip_prefix("ID=").or_else(|| l.strip_prefix("ID_LIKE=")))
        .flat_map(|v| {
            v.trim_matches('"')
                .split_whitespace()
                .map(str::to_lowercase)
                .collect::<Vec<_>>()
        })
        .collect();
    let any = |names: &[&str]| ids.iter().any(|i| names.contains(&i.as_str()));
    if any(&["debian", "ubuntu"]) {
        "/usr/lib/x86_64-linux-gnu/p11-kit-proxy.so"
    } else if any(&["fedora", "rhel", "centos", "suse", "opensuse", "mageia"]) {
        "/usr/lib64/p11-kit-proxy.so"
    } else {
        "/usr/lib/p11-kit-proxy.so"
    }
}

/// Is this list.a3 entry p11-kit (the proxy, or libp11-kit.so, which is the
/// same module), in a form PJeOffice accepts?
fn is_p11kit_entry(entry: &str) -> bool {
    let name = entry.rsplit('/').next().unwrap_or(entry);
    name.ends_with(".so") && name.contains("p11-kit")
}

/// The value of `list.a3` in PJeOffice's settings, if the file exists.
fn pjeoffice_a3() -> Option<String> {
    let text = std::fs::read_to_string(pjeoffice_config()).ok()?;
    Some(
        text.lines()
            .find_map(|l| l.trim().strip_prefix("list.a3="))
            .unwrap_or("")
            .trim()
            .to_string(),
    )
}

/// PJeOffice's settings with `library` in `list.a3`, in place of any other
/// p11-kit entry (the other libraries stay); the line is added if missing,
/// and everything else is kept.
fn with_a3(text: &str, library: &str) -> String {
    let joined = |old: &str| {
        let mut entries: Vec<&str> = old
            .split('|')
            .map(str::trim)
            .filter(|e| !e.is_empty() && !is_p11kit_entry(e))
            .collect();
        entries.push(library);
        format!("list.a3={}", entries.join("|"))
    };
    let mut found = false;
    let mut out: Vec<String> = text
        .lines()
        .map(|l| match l.trim_start().strip_prefix("list.a3=") {
            Some(old) => {
                found = true;
                joined(old)
            }
            None => l.to_string(),
        })
        .collect();
    if !found {
        out.push(joined(""));
    }
    out.join("\n") + "\n"
}

/// The databases the app opens; empty until there is one.
fn databases(app: &NativeApp) -> Vec<PathBuf> {
    match app.kind {
        Kind::Firefox => firefox_profiles(),
        Kind::Chromium => chromium_nssdb().into_iter().collect(),
        Kind::Poppler => poppler_dbs(),
        Kind::PjeOffice => vec![],
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
    if app.kind == Kind::PjeOffice {
        return match pjeoffice_a3() {
            None => State::NoDatabase,
            Some(v) if v.split('|').any(|e| is_p11kit_entry(e.trim())) => State::Ready,
            Some(_) => State::ToDo,
        };
    }
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
            Kind::PjeOffice => t("Open PJeOffice Pro once (it writes ~/.pjeoffice-pro/pjeoffice-pro.config when it closes), then come back"),
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
    if app.kind == Kind::PjeOffice {
        let file = pjeoffice_config();
        let text = std::fs::read_to_string(&file).map_err(|_| subtitle(app))?;
        let library = host_p11kit_library();
        std::fs::write(&file, with_a3(&text, library))
            .map_err(|e| format!("{}: {e}", file.display()))?;
        return Ok(format!(
            "{}: {} ({library})",
            t(app.name),
            t("set up; restart the app")
        ));
    }
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

        // Nothing installed: no database anywhere.
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

        // PJeOffice Pro: its list.a3, by distribution.
        assert!(matches!(state(&PJEOFFICE), State::NoDatabase));
        fs::create_dir_all(home.join(".pjeoffice-pro")).unwrap();
        fs::write(
            pjeoffice_config(),
            "#Salvo em 2026-11-01\nlist.a3=/usr/lib/libaetpkss.so\nlist.a1=\n",
        )
        .unwrap();
        assert!(matches!(state(&PJEOFFICE), State::ToDo));
        set_up(&PJEOFFICE).unwrap();
        let text = fs::read_to_string(pjeoffice_config()).unwrap();
        assert!(text.contains(&format!(
            "list.a3=/usr/lib/libaetpkss.so|{}\n",
            host_p11kit_library()
        )));
        assert!(text.starts_with("#Salvo em 2026-11-01\n") && text.contains("list.a1=\n"));
        assert!(matches!(state(&PJEOFFICE), State::Ready));
        // Set up again: still one p11-kit entry.
        set_up(&PJEOFFICE).unwrap();
        let text = fs::read_to_string(pjeoffice_config()).unwrap();
        assert_eq!(text.matches("p11-kit").count(), 1);
        // The path Pedro set by hand counts as ready; .so.0 does not
        // (PJeOffice ignores it).
        fs::write(pjeoffice_config(), "list.a3=/lib/libp11-kit.so\n").unwrap();
        assert!(matches!(state(&PJEOFFICE), State::Ready));
        fs::write(pjeoffice_config(), "list.a3=/usr/lib64/libp11-kit.so.0\n").unwrap();
        assert!(matches!(state(&PJEOFFICE), State::ToDo));
        // A file without list.a3 gets the line.
        fs::write(pjeoffice_config(), "list.a1=\n").unwrap();
        set_up(&PJEOFFICE).unwrap();
        assert!(fs::read_to_string(pjeoffice_config())
            .unwrap()
            .ends_with(&format!("list.a3={}\n", host_p11kit_library())));

        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn p11kit_library_by_distribution() {
        assert_eq!(
            p11kit_library_for("ID=fedora\nVERSION_ID=43\n"),
            "/usr/lib64/p11-kit-proxy.so"
        );
        assert_eq!(
            p11kit_library_for("ID=bluefin\nID_LIKE=\"fedora\"\n"),
            "/usr/lib64/p11-kit-proxy.so"
        );
        assert_eq!(
            p11kit_library_for("ID=opensuse-tumbleweed\nID_LIKE=\"opensuse suse\"\n"),
            "/usr/lib64/p11-kit-proxy.so"
        );
        assert_eq!(
            p11kit_library_for("ID=linuxmint\nID_LIKE=\"ubuntu debian\"\n"),
            "/usr/lib/x86_64-linux-gnu/p11-kit-proxy.so"
        );
        assert_eq!(
            p11kit_library_for("ID=cachyos\nID_LIKE=arch\n"),
            "/usr/lib/p11-kit-proxy.so"
        );
        assert_eq!(p11kit_library_for(""), "/usr/lib/p11-kit-proxy.so");
    }
}
