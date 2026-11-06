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
//! - Okular and Papers sign through poppler, which opens the most recent
//!   *default* Firefox profile that has a cert9.db, first under
//!   $XDG_CONFIG_HOME/mozilla/firefox, then ~/.mozilla/firefox. They depend
//!   on Firefox: their row has no button of its own, and is ready only when
//!   those profiles are.
//!
//! - PJeOffice Pro is Java and keeps its own list of PKCS#11 libraries in
//!   ~/.pjeoffice-pro/pjeoffice-pro.config (`list.a3=`, separated by `|`):
//!   there p11-kit-proxy.so goes by full path, which depends on the
//!   distribution. PJeOffice skips paths that do not exist and takes only
//!   names ending in a known extension (.so, not .so.0).
//!
//! Some systems already load p11-kit in every NSS database: Fedora and its
//! family through crypto-policies (see system_nss_loads_p11kit), others
//! through nss-sysinit or by registering it in the database. Registering it
//! again would show each token twice, so such an app counts as ready and
//! gets no button; if Tessera's module is there as well, the button removes
//! it.

use std::path::PathBuf;

use crate::apps;
use crate::i18n::t;
use crate::libreoffice;
use crate::nss;
use crate::paths;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Firefox,
    Thunderbird,
    Chromium,
    Evolution,
    Poppler,
    LibreOffice,
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
    name: "Okular and Papers",
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

pub const THUNDERBIRD: NativeApp = NativeApp {
    name: "Thunderbird",
    icons: &["thunderbird", "org.mozilla.Thunderbird"],
    fallback_icon: "mail-send-symbolic",
    kind: Kind::Thunderbird,
};
/// Evolution opens the same database as the Chromium-based browsers
/// (evolution-data-server's camel_init: ~/.pki/nssdb if it exists, else
/// $XDG_DATA_HOME/pki/nssdb from 3.62).
pub const EVOLUTION: NativeApp = NativeApp {
    name: "Evolution",
    icons: &["org.gnome.Evolution", "evolution"],
    fallback_icon: "mail-send-symbolic",
    kind: Kind::Evolution,
};
pub const LIBREOFFICE: NativeApp = NativeApp {
    name: "LibreOffice",
    icons: &["libreoffice-startcenter", "org.libreoffice.LibreOffice"],
    fallback_icon: "x-office-document-symbolic",
    kind: Kind::LibreOffice,
};

pub const APPS: [&NativeApp; 7] = [
    &FIREFOX,
    &CHROMIUM_BROWSERS,
    &THUNDERBIRD,
    &EVOLUTION,
    &PDF_VIEWERS,
    &LIBREOFFICE,
    &PJEOFFICE,
];

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

/// Thunderbird keeps using ~/.thunderbird while it exists; a new install
/// may use $XDG_CONFIG_HOME/thunderbird (no "mozilla" level, unlike
/// Firefox).
fn thunderbird_roots() -> Vec<PathBuf> {
    vec![
        paths::home().join(".thunderbird"),
        paths::host_config_home().join("thunderbird"),
    ]
}

/// The native Thunderbird's profile folders.
pub fn thunderbird_profiles() -> Vec<PathBuf> {
    paths::profiles_in(&thunderbird_roots())
}

/// The database the native LibreOffice signs with, when its settings name
/// one that exists (CertDir; see libreoffice.rs).
fn libreoffice_cert_dir() -> Option<PathBuf> {
    libreoffice::cert_dir(&libreoffice::native_profile())
        .flatten()
        .map(PathBuf::from)
        .filter(|d| d.is_dir())
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

/// The Chromium and Evolution Flatpaks may read the real ~/.pki/nssdb too:
/// p11-kit-proxy.so there would make them list each token twice (they
/// already see them through their root-certificates module), and ask for
/// the PIN twice.
fn shared_with_chromium_flatpak(db: &std::path::Path) -> bool {
    db == paths::home().join(".pki/nssdb")
        && [apps::CHROMIUM.id, apps::EVOLUTION.id]
            .iter()
            .any(|id| paths::home().join(".var/app").join(id).is_dir())
}

/// The databases poppler may open: every *default* Firefox profile with a
/// cert9.db (poppler takes the most recent one).
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
    p11kit_library_for(&host_os_release())
}

/// The host's os-release (in a Flatpak, /run/host/os-release;
/// TESSERA_OS_RELEASE, another file, for the tests).
fn host_os_release() -> String {
    let test = std::env::var_os("TESSERA_OS_RELEASE").map(PathBuf::from);
    test.into_iter()
        .chain(["/run/host/os-release", "/etc/os-release"].map(PathBuf::from))
        .find_map(|f| std::fs::read_to_string(f).ok())
        .unwrap_or_default()
}

/// The distribution and the ones it derives from (ID and ID_LIKE).
fn release_ids(os_release: &str) -> Vec<String> {
    os_release
        .lines()
        .filter_map(|l| l.strip_prefix("ID=").or_else(|| l.strip_prefix("ID_LIKE=")))
        .flat_map(|v| {
            v.trim_matches('"')
                .split_whitespace()
                .map(str::to_lowercase)
                .collect::<Vec<_>>()
        })
        .collect()
}

/// Is the host Debian or one of its derivatives (Ubuntu, Mint…)?
pub fn host_is_debian_family() -> bool {
    debian_family(&host_os_release())
}

/// Releases of the Debian family whose p11-kit is older than 0.25.6
/// (Debian 11 to 13, Ubuntu 20.04 to 24.04, and what is built on them: MX,
/// LMDE, Linux Mint, Pop!_OS…). From 0.25.6 on, p11-kit sends a mechanism's
/// parameters in another layout, not tied to the protocol version, so an
/// older system p11-kit and Tessera's disagree: tokens and the PIN work,
/// every signature with parameters (RSA-PSS, TLS 1.3) fails (history.md
/// item 38). By codename, which derivatives keep in UBUNTU_CODENAME or
/// DEBIAN_CODENAME: a Flatpak cannot ask the host's p11-kit its version.
const OLD_P11KIT_CODENAMES: [&str; 6] =
    ["bullseye", "bookworm", "trixie", "focal", "jammy", "noble"];

/// Is the host's p11-kit too old for signing through Tessera? `protocol` is
/// what the system's p11-kit server answered (status::host_protocol), when
/// it is known to be that server: the version it speaks decides. Without it
/// (the server is off), the release codename.
pub fn host_has_old_p11kit(protocol: Option<u8>) -> bool {
    old_p11kit(protocol, || old_p11kit_by_release(&host_os_release()))
}

fn old_p11kit(protocol: Option<u8>, by_release: impl FnOnce() -> bool) -> bool {
    match protocol {
        Some(v) => v < 2,
        None => by_release(),
    }
}

fn old_p11kit_by_release(os_release: &str) -> bool {
    debian_family(os_release)
        && os_release
            .lines()
            .filter_map(|l| {
                ["VERSION_CODENAME=", "UBUNTU_CODENAME=", "DEBIAN_CODENAME="]
                    .iter()
                    .find_map(|k| l.trim().strip_prefix(k))
            })
            .map(|v| v.trim().trim_matches('"').to_lowercase())
            .any(|c| OLD_P11KIT_CODENAMES.contains(&c.as_str()))
}

fn debian_family(os_release: &str) -> bool {
    release_ids(os_release)
        .iter()
        .any(|i| ["debian", "ubuntu"].contains(&i.as_str()))
}

fn p11kit_library_for(os_release: &str) -> &'static str {
    let ids = release_ids(os_release);
    let any = |names: &[&str]| ids.iter().any(|i| names.contains(&i.as_str()));
    if debian_family(os_release) {
        "/usr/lib/x86_64-linux-gnu/p11-kit-proxy.so"
    } else if any(&["fedora", "rhel", "centos", "suse", "opensuse", "mageia"]) {
        "/usr/lib64/p11-kit-proxy.so"
    } else {
        "/usr/lib/p11-kit-proxy.so"
    }
}

/// Fedora's crypto-policies file for NSS (reported by the CI test).
#[cfg(test)]
const NSS_POLICY: &str = "/etc/crypto-policies/back-ends/nss.config";

/// Does the system's NSS load p11-kit for every program that uses it?
/// Fedora and its family do: their crypto-policies register p11-kit-proxy.so
/// for NSS, and their NSS reads that whenever it opens a database, so the
/// native Firefox, Chromium, Okular and Papers see the tokens with nothing
/// registered in their databases (and would see each token twice with
/// Tessera's module too). openSUSE ships the same crypto-policies file, but
/// its NSS does not load it. Decided by the distribution, which is all a
/// Flatpak can see of the host; CI checks it against what NSS actually loads
/// on Fedora, Debian, Ubuntu, Arch and openSUSE
/// (detection_matches_this_system).
pub fn system_nss_loads_p11kit() -> bool {
    policy_by_release(&host_os_release())
}

fn policy_by_release(os_release: &str) -> bool {
    let ids = release_ids(os_release);
    ids.iter()
        .any(|i| ["fedora", "rhel", "centos"].contains(&i.as_str()))
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
        Kind::Thunderbird => thunderbird_profiles(),
        Kind::Chromium | Kind::Evolution => chromium_nssdb().into_iter().collect(),
        Kind::Poppler => poppler_dbs(),
        Kind::LibreOffice => libreoffice_cert_dir().into_iter().collect(),
        Kind::PjeOffice => vec![],
    }
}

pub enum State {
    Ready,
    /// The system already loads p11-kit there (crypto-policies on Fedora,
    /// or nss-sysinit).
    BySystem,
    /// The system loads p11-kit and Tessera's module is there too: each
    /// token shows twice. "Remove" takes Tessera's module out.
    Twice,
    ToDo,
    /// No database yet: the app has to be opened once.
    NoDatabase,
    /// Okular and Papers: Firefox's profiles are not set up yet.
    NeedsFirefox,
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
    if app.kind == Kind::LibreOffice {
        return libreoffice_state();
    }
    let dbs = databases(app);
    // Okular and Papers cannot be detected (no folder of their own); with
    // the system's NSS loading p11-kit they work whatever the database. The
    // browsers only count once they have one: an app that is not there is
    // not "ready" (on Bluefin the browsers are Flatpaks).
    if app.kind != Kind::Poppler && dbs.is_empty() {
        return State::NoDatabase;
    }
    if system_nss_loads_p11kit() {
        // A duplicate in Firefox's profiles is the Firefox row's to remove.
        if app.kind != Kind::Poppler && dbs.iter().any(|d| nss::has_tessera_module(d)) {
            return State::Twice;
        }
        return State::BySystem;
    }
    if dbs.is_empty() {
        return State::NoDatabase;
    }
    if dbs.iter().all(|d| nss::has_p11kit(d)) {
        return State::Ready;
    }
    if dbs
        .iter()
        .all(|d| nss::has_p11kit(d) || nss::uses_sysinit(d))
    {
        return State::BySystem;
    }
    if app.kind == Kind::Poppler {
        return State::NeedsFirefox;
    }
    if dbs.iter().any(|d| shared_with_chromium_flatpak(d)) {
        return State::SharedWithFlatpak;
    }
    State::ToDo
}

/// LibreOffice: with no certificate path of its own (CertDir), it opens a
/// Thunderbird or old-style Firefox profile, or a temporary database, so
/// Set up gives it one; a CertDir the user chose gets the module instead.
fn libreoffice_state() -> State {
    let Some(_) = libreoffice::cert_dir(&libreoffice::native_profile()) else {
        return State::NoDatabase;
    };
    let db = libreoffice_cert_dir();
    if system_nss_loads_p11kit() {
        return match db {
            Some(d) if nss::has_tessera_module(&d) => State::Twice,
            _ => State::BySystem,
        };
    }
    match db {
        Some(d) if nss::has_p11kit(&d) => State::Ready,
        Some(d) if nss::uses_sysinit(&d) => State::BySystem,
        _ => State::ToDo,
    }
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
        Kind::Poppler => t("Native apps; they use Firefox's certificate database").to_string(),
        Kind::Evolution => {
            t("Native app; same certificate database as the Chromium-based browsers").to_string()
        }
        _ => t("Native app").to_string(),
    };
    let state = match state(app) {
        State::Ready => t("Ready"),
        State::BySystem => t("Ready: the system's NSS already loads p11-kit"),
        State::Twice => t("Each token shows twice: the system's NSS already loads p11-kit, and Tessera's module is there too. Remove Tessera's"),
        State::ToDo => t("Set-up missing"),
        State::NeedsFirefox => t("Set up Firefox first"),
        State::NoDatabase => match app.kind {
            Kind::PjeOffice => t("Open PJeOffice Pro once (it writes ~/.pjeoffice-pro/pjeoffice-pro.config when it closes), then come back"),
            Kind::LibreOffice => t("Open LibreOffice once (it writes its settings when it closes), then come back"),
            Kind::Poppler => t("Needs Firefox: install it, open it once and set it up"),
            _ => t("Open the app once, then come back"),
        },
        State::SharedWithFlatpak => t("Not available: the Chromium or Evolution Flatpak uses the same certificate database (~/.pki/nssdb) and would see each token twice"),
    };
    format!("{what} · {state}")
}

/// Does the row offer "Set up"? Not when p11-kit is already there (by
/// Tessera, by the distribution or through nss-sysinit), and never for
/// Okular and Papers, which follow Firefox.
pub fn can_set_up(app: &NativeApp) -> bool {
    matches!(state(app), State::ToDo | State::Twice)
}

/// The button's label: "Remove" when Tessera's module is a duplicate.
pub fn button_label(app: &NativeApp) -> &'static str {
    match state(app) {
        State::Twice => t("Remove"),
        _ => t("Set up"),
    }
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
    if app.kind == Kind::LibreOffice && libreoffice::running(&libreoffice::native_profile()) {
        return Err(format!(
            "{}: {} ({})",
            t(app.name),
            t("close the app first, then try again"),
            t("if it is closed, a crash may have left the file .lock in its profile")
        ));
    }
    let dbs = databases(app);
    if dbs.iter().any(|d| nss::profile_in_use(d)) {
        return Err(format!(
            "{}: {}",
            t(app.name),
            t("close the app first, then try again")
        ));
    }
    if matches!(state(app), State::Twice) {
        for d in dbs.iter().filter(|d| nss::has_tessera_module(d)) {
            nss::remove_tessera_module(d).map_err(|e| format!("{}: {e:#}", d.display()))?;
        }
        return Ok(format!(
            "{}: {}",
            t(app.name),
            t("Tessera's duplicate module removed; restart the app")
        ));
    }
    if app.kind == Kind::LibreOffice && dbs.is_empty() {
        // No certificate path of its own yet: one in Tessera's data folder.
        let db = libreoffice::native_db();
        nss::add_library(&db, paths::PROXY_MODULE)
            .map_err(|e| format!("{}: {e:#}", db.display()))?;
        libreoffice::set_cert_dir(&libreoffice::native_profile(), &db)?;
        return Ok(format!("{}: {}", t(app.name), t("set up; restart the app")));
    }
    if dbs.is_empty() {
        return Err(subtitle(app));
    }
    if dbs.iter().any(|d| shared_with_chromium_flatpak(d)) {
        return Err(subtitle(app));
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
        // As on a distribution whose NSS does not load p11-kit by itself
        // (the Fedora case comes at the end).
        fs::create_dir_all(&home).unwrap();
        let release = home.join("os-release");
        // Fedora's family with no native browser (Bluefin): the browsers are
        // not "ready", since they are not there.
        fs::write(&release, "ID=bluefin\nID_LIKE=\"fedora\"\n").unwrap();
        std::env::set_var("TESSERA_OS_RELEASE", &release);
        assert!(matches!(state(&FIREFOX), State::NoDatabase));
        assert!(matches!(state(&CHROMIUM_BROWSERS), State::NoDatabase));
        assert!(!can_set_up(&FIREFOX));
        fs::write(&release, "ID=debian\n").unwrap();

        // Nothing installed: no database anywhere.
        assert!(matches!(state(&PDF_VIEWERS), State::NoDatabase));
        assert!(!can_set_up(&PDF_VIEWERS));

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
        // Okular and Papers wait for Firefox, with no button of their own.
        assert!(matches!(state(&PDF_VIEWERS), State::NeedsFirefox));
        assert!(!can_set_up(&PDF_VIEWERS));
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
        assert!(matches!(state(&CHROMIUM_BROWSERS), State::BySystem));
        assert!(!can_set_up(&CHROMIUM_BROWSERS));
        // Registered by the distribution, under another name or path.
        fs::write(
            db.join("pkcs11.txt"),
            "library=/usr/lib64/libp11-kit.so.0\n",
        )
        .unwrap();
        assert!(matches!(state(&CHROMIUM_BROWSERS), State::Ready));
        // p11-kit's trust module alone does not count.
        fs::write(
            db.join("pkcs11.txt"),
            "library=/usr/lib64/pkcs11/p11-kit-trust.so\n",
        )
        .unwrap();
        assert!(matches!(state(&CHROMIUM_BROWSERS), State::ToDo));
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

        // Thunderbird: its profiles under ~/.thunderbird and the XDG folder.
        assert!(matches!(state(&THUNDERBIRD), State::NoDatabase));
        let tb = home.join(".thunderbird");
        fs::create_dir_all(tb.join("p1.default-esr")).unwrap();
        fs::write(
            tb.join("profiles.ini"),
            "[Profile0]\nPath=p1.default-esr\nIsRelative=1\n",
        )
        .unwrap();
        let xtb = home.join(".config/thunderbird");
        fs::create_dir_all(xtb.join("p2.default")).unwrap();
        fs::write(
            xtb.join("profiles.ini"),
            "[Profile0]\nPath=p2.default\nIsRelative=1\n",
        )
        .unwrap();
        assert_eq!(thunderbird_profiles().len(), 2);
        assert!(matches!(state(&THUNDERBIRD), State::ToDo));
        fs::write(tb.join("p1.default-esr/pkcs11.txt"), proxy).unwrap();
        fs::write(xtb.join("p2.default/pkcs11.txt"), proxy).unwrap();
        assert!(matches!(state(&THUNDERBIRD), State::Ready));

        // Evolution shares the Chromium-based browsers' database, and so its
        // state (here, the Chromium Flatpak reads it too).
        assert!(matches!(state(&EVOLUTION), State::SharedWithFlatpak));

        // LibreOffice: no settings yet, then no certificate path of its own
        // (Set up would make one), then one the user chose.
        assert!(matches!(state(&LIBREOFFICE), State::NoDatabase));
        let lo = libreoffice::native_profile();
        fs::create_dir_all(&lo).unwrap();
        let xcu = "<?xml version=\"1.0\"?>\n<oor:items xmlns:oor=\"http://openoffice.org/2001/registry\">\n</oor:items>\n";
        fs::write(lo.join("registrymodifications.xcu"), xcu).unwrap();
        assert!(matches!(state(&LIBREOFFICE), State::ToDo));
        assert!(can_set_up(&LIBREOFFICE));
        let mine = home.join("my-nssdb");
        fs::create_dir_all(&mine).unwrap();
        libreoffice::set_cert_dir(&lo, &mine).unwrap();
        assert_eq!(databases(&LIBREOFFICE), vec![mine.clone()]);
        assert!(matches!(state(&LIBREOFFICE), State::ToDo));
        fs::write(mine.join("pkcs11.txt"), proxy).unwrap();
        assert!(matches!(state(&LIBREOFFICE), State::Ready));
        // While it runs (its .lock), Set up waits.
        fs::write(lo.join(".lock"), b"").unwrap();
        fs::remove_file(mine.join("pkcs11.txt")).unwrap();
        assert!(set_up(&LIBREOFFICE).is_err());
        fs::remove_file(lo.join(".lock")).unwrap();
        fs::write(mine.join("pkcs11.txt"), proxy).unwrap();

        // Fedora and its family: their NSS loads p11-kit in every database.
        // Native Firefox (its profiles have Tessera's module from above)
        // then shows each token twice, and offers to remove the module.
        fs::write(&release, "ID=bluefin\nID_LIKE=\"fedora\"\n").unwrap();
        assert!(system_nss_loads_p11kit());
        assert!(matches!(state(&FIREFOX), State::Twice));
        assert!(can_set_up(&FIREFOX));
        assert_eq!(button_label(&FIREFOX), "Remove");
        // LibreOffice's database has Tessera's module too: twice.
        assert!(matches!(state(&LIBREOFFICE), State::Twice));
        // The others need nothing.
        for app in [&CHROMIUM_BROWSERS, &PDF_VIEWERS, &EVOLUTION] {
            assert!(matches!(state(app), State::BySystem));
            assert!(!can_set_up(app));
        }
        // Without Tessera's module, Firefox needs nothing either.
        for p in ["abc.default", "xyz.default-release"] {
            fs::write(
                ff.join(p).join("pkcs11.txt"),
                "library=\nname=NSS Internal PKCS #11 Module\n",
            )
            .unwrap();
        }
        assert!(matches!(state(&FIREFOX), State::BySystem));
        assert!(!can_set_up(&FIREFOX));
        std::env::remove_var("TESSERA_OS_RELEASE");

        // The Flatpaks' side, on the same made-up home (HOME is the
        // process's): Thunderbird's profiles in its own folder, and its ESR
        // and old ID shown only when installed.
        let tbf = home.join(".var/app/org.mozilla.thunderbird/.thunderbird");
        fs::create_dir_all(tbf.join("q.default-release")).unwrap();
        fs::write(
            tbf.join("profiles.ini"),
            "[Profile0]\nPath=q.default-release\n",
        )
        .unwrap();
        assert_eq!(
            apps::app_profiles(&apps::THUNDERBIRD),
            vec![tbf.join("q.default-release")]
        );
        assert!(apps::app_profiles(&apps::THUNDERBIRD_ESR).is_empty());
        assert!(apps::shown(&apps::THUNDERBIRD));
        assert!(!apps::shown(&apps::THUNDERBIRD_ESR));
        fs::create_dir_all(home.join(".var/app/org.mozilla.thunderbird_esr")).unwrap();
        assert!(apps::shown(&apps::THUNDERBIRD_ESR));
        assert!(!apps::shown(&apps::THUNDERBIRD_OLD));
        // Evolution's Flatpak reads the real ~/.pki/nssdb, like Chromium's.
        assert_eq!(
            apps::chromium_nssdb(&apps::EVOLUTION),
            Some(home.join(".pki/nssdb"))
        );

        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn nss_policy_by_distribution() {
        assert!(policy_by_release("ID=fedora\n"));
        assert!(policy_by_release("ID=aurora\nID_LIKE=\"fedora\"\n"));
        assert!(policy_by_release(
            "ID=rocky\nID_LIKE=\"rhel centos fedora\"\n"
        ));
        assert!(!policy_by_release("ID=debian\n"));
        assert!(!policy_by_release(
            "ID=opensuse-tumbleweed\nID_LIKE=\"opensuse suse\"\n"
        ));
        assert!(!policy_by_release("ID=arch\n"));
    }

    #[test]
    fn debian_family_by_distribution() {
        assert!(debian_family("ID=debian\n"));
        assert!(debian_family("ID=ubuntu\nID_LIKE=debian\n"));
        assert!(debian_family("ID=linuxmint\nID_LIKE=\"ubuntu debian\"\n"));
        assert!(!debian_family("ID=fedora\n"));
        assert!(!debian_family("ID=cachyos\nID_LIKE=arch\n"));
        assert!(!debian_family(
            "ID=opensuse-tumbleweed\nID_LIKE=\"opensuse suse\"\n"
        ));
        assert!(!debian_family(""));
    }

    #[test]
    fn the_servers_protocol_decides_before_the_codename() {
        // MX 25 with p11-kit 0.26 from its test repository: trixie, but 2.
        assert!(!old_p11kit(Some(2), || true));
        assert!(old_p11kit(Some(1), || false));
        assert!(old_p11kit(Some(0), || false));
        assert!(old_p11kit(None, || true));
        assert!(!old_p11kit(None, || false));
    }

    #[test]
    fn old_p11kit_by_distribution() {
        let old = [
            "ID=debian\nVERSION_ID=\"13\"\nVERSION_CODENAME=trixie\n",
            "ID=debian\nVERSION_ID=\"12\"\nVERSION_CODENAME=bookworm\n",
            // MX keeps Debian's os-release.
            "PRETTY_NAME=\"MX 25\"\nID=debian\nVERSION_CODENAME=trixie\n",
            "ID=linuxmint\nID_LIKE=debian\nVERSION_CODENAME=gigi\nDEBIAN_CODENAME=trixie\n",
            "ID=linuxmint\nID_LIKE=debian\nVERSION_CODENAME=faye\nDEBIAN_CODENAME=bookworm\n",
            "ID=ubuntu\nID_LIKE=debian\nVERSION_ID=\"24.04\"\nVERSION_CODENAME=noble\nUBUNTU_CODENAME=noble\n",
            "ID=ubuntu\nID_LIKE=debian\nVERSION_CODENAME=jammy\nUBUNTU_CODENAME=jammy\n",
            "ID=linuxmint\nID_LIKE=\"ubuntu debian\"\nVERSION_CODENAME=wilma\nUBUNTU_CODENAME=noble\n",
            "ID=pop\nID_LIKE=\"ubuntu debian\"\nVERSION_CODENAME=noble\nUBUNTU_CODENAME=noble\n",
        ];
        for r in old {
            assert!(old_p11kit_by_release(r), "{r}");
        }
        let fine = [
            "ID=ubuntu\nID_LIKE=debian\nVERSION_ID=\"26.04\"\nVERSION_CODENAME=resolute\nUBUNTU_CODENAME=resolute\n",
            "ID=ubuntu\nID_LIKE=debian\nVERSION_CODENAME=questing\nUBUNTU_CODENAME=questing\n",
            "ID=debian\nVERSION_CODENAME=forky\n",
            "ID=fedora\nVERSION_ID=44\n",
            "ID=cachyos\nID_LIKE=arch\n",
            "ID=opensuse-tumbleweed\nID_LIKE=\"opensuse suse\"\n",
            "",
        ];
        for r in fine {
            assert!(!old_p11kit_by_release(r), "{r}");
        }
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

    /// Run in CI on several distributions (Fedora, Debian, Ubuntu, Arch,
    /// openSUSE), each in its own container with its default packages:
    /// what Tessera deduces must match the system.
    ///
    /// - The path written into PJeOffice's list.a3, chosen from os-release,
    ///   exists.
    /// - A fresh NSS database, made the way a browser makes its own: Tessera
    ///   calls it ready without Set up (by the distribution, nss-sysinit, or
    ///   p11-kit already there) exactly when NSS, opening it, loads a p11-kit
    ///   module.
    ///
    /// Needs certutil and modutil (NSS's tools) and p11-kit.
    #[test]
    #[ignore = "run in CI, in a container of each distribution"]
    fn detection_matches_this_system() {
        use std::process::Command;
        let release = fs::read_to_string("/etc/os-release").unwrap_or_default();
        let distro = release
            .lines()
            .find_map(|l| l.strip_prefix("PRETTY_NAME="))
            .unwrap_or("?")
            .trim_matches('"')
            .to_string();

        let library = host_p11kit_library();
        let exists = std::path::Path::new(library).is_file();
        println!("REPORT: {distro}: PJeOffice list.a3 path {library}: exists={exists}");
        assert!(exists, "{library} does not exist on {distro}");

        let dir = std::env::temp_dir().join(format!("tessera-distro-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let db = format!("sql:{}", dir.display());
        let made = Command::new("certutil")
            .args(["-N", "-d", &db, "--empty-password"])
            .status()
            .expect("certutil (NSS's tools) is needed");
        assert!(made.success(), "certutil -N failed");
        let listing = Command::new("modutil")
            .args(["-list", "-dbdir", &db])
            .stdin(std::process::Stdio::null())
            .output()
            .expect("modutil (NSS's tools) is needed");
        let listing = String::from_utf8_lossy(&listing.stdout);
        let nss_loads_p11kit = listing.lines().any(|l| {
            let l = l.to_lowercase();
            l.contains("library name:") && l.contains("p11-kit") && !l.contains("trust")
        });
        let policy_file = fs::read_to_string(NSS_POLICY)
            .map(|t| t.contains("p11-kit-proxy"))
            .unwrap_or(false);
        let tessera_says_ready =
            system_nss_loads_p11kit() || nss::has_p11kit(&dir) || nss::uses_sysinit(&dir);
        println!(
            "REPORT: {distro}: fresh NSS database: crypto-policies file lists p11-kit={policy_file} sysinit={} p11-kit registered={} NSS loads p11-kit={nss_loads_p11kit} Tessera says ready={tessera_says_ready}",
            nss::uses_sysinit(&dir),
            nss::has_p11kit(&dir),
        );
        let _ = fs::remove_dir_all(&dir);
        assert_eq!(
            tessera_says_ready, nss_loads_p11kit,
            "on {distro}, Tessera's verdict differs from what NSS loads:\n{listing}"
        );
    }
}
