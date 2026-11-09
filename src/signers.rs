//! Web signers: the programs behind the signing plugins of the browsers
//! (Lacuna Web PKI, Softplan WebSigner, Certisign WebSigner), published as
//! Flatpak extensions of Tessera (io.github.pedrohqb.Tessera.Signer.<Name>,
//! see flatpak/extensions and docs/signers.md).
//!
//! Tessera does not run them. A browser's plugin starts its program by
//! native messaging: the browser runs the program named in a small JSON
//! manifest, inside the browser's own sandbox. So each Flatpak browser
//! needs, once per signer:
//!
//! - read access to where Flatpak deploys the extension on the host
//!   (<installation>/runtime/<extension>), so it can run the program there;
//! - the program's data folder kept (--persist), and for Firefox the X11
//!   socket, which the Web PKI engine's dialogs need;
//! - the manifest, in the browser's own folder.
//!
//! Tessera cannot do that from its sandbox (it would need write access to
//! every browser's folder and to Flatpak's overrides), so the window shows
//! the one command that does it, and whether the browser's override already
//! has it. The program then reaches Tessera's tokens like the browser does,
//! through the system's p11-kit server (step 1 of the browser's row).

use serde::Deserialize;
use std::path::{Path, PathBuf};

use crate::apps::{self, ClientApp};
use crate::paths;

/// The prefix of the extensions' ids.
pub const EXTENSION_PREFIX: &str = "io.github.pedrohqb.Tessera.Signer.";

#[derive(Debug, Clone, Deserialize)]
struct SignerFile {
    name: String,
    #[serde(default)]
    vendor: String,
    host: String,
    exec: String,
    #[serde(default)]
    chrome_extensions: Vec<String>,
    #[serde(default)]
    firefox_extensions: Vec<String>,
    #[serde(default)]
    persist: Option<String>,
    #[serde(default)]
    firefox_x11: bool,
    #[serde(default)]
    option: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Signer {
    /// The extension's folder (WebPKI): its id is EXTENSION_PREFIX + this.
    pub folder: String,
    pub name: String,
    pub vendor: String,
    /// The native messaging host's name.
    pub host: String,
    /// The start script, relative to the extension's files.
    pub exec: String,
    pub chrome_extensions: Vec<String>,
    pub firefox_extensions: Vec<String>,
    pub persist: Option<String>,
    pub firefox_x11: bool,
    /// What to tick, once, in the plugin's settings.
    pub option: Option<String>,
    /// Where Tessera sees it (/app/lib/tessera/signers/<folder>).
    pub dir: PathBuf,
}

impl Signer {
    pub fn extension(&self) -> String {
        format!("{EXTENSION_PREFIX}{}", self.folder)
    }
}

/// Where the signer extensions are mounted in Tessera's sandbox.
pub fn signers_dir() -> PathBuf {
    std::env::var_os("TESSERA_SIGNERS_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/app/lib/tessera/signers"))
}

fn load(dir: &Path) -> Option<Signer> {
    let text = std::fs::read_to_string(dir.join("signer.toml")).ok()?;
    let f: SignerFile = toml::from_str(&text).ok()?;
    Some(Signer {
        folder: dir.file_name()?.to_string_lossy().into_owned(),
        name: f.name,
        vendor: f.vendor,
        host: f.host,
        exec: f.exec,
        chrome_extensions: f.chrome_extensions,
        firefox_extensions: f.firefox_extensions,
        persist: f.persist.filter(|p| !p.is_empty()),
        firefox_x11: f.firefox_x11,
        option: f.option.filter(|o| !o.is_empty()),
        dir: dir.to_path_buf(),
    })
}

/// The installed signers, by name.
pub fn installed() -> Vec<Signer> {
    let Ok(entries) = std::fs::read_dir(signers_dir()) else {
        return vec![];
    };
    let mut out: Vec<Signer> = entries.flatten().filter_map(|e| load(&e.path())).collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// The signers published so far, for the ones not installed.
pub struct Known {
    pub folder: &'static str,
    pub name: &'static str,
    pub vendor: &'static str,
    /// What it is for, in the user's words.
    pub used_by: &'static str,
}

impl Known {
    pub fn extension(&self) -> String {
        format!("{EXTENSION_PREFIX}{}", self.folder)
    }
}

pub const KNOWN: &[Known] = &[
    Known {
        folder: "WebPKI",
        name: "Lacuna Web PKI",
        vendor: "Lacuna Software",
        used_by: "Signing sites that use the Web PKI plugin",
    },
    Known {
        folder: "SoftplanWebSigner",
        name: "Softplan WebSigner",
        vendor: "Softplan",
        used_by: "e-SAJ and other Softplan systems",
    },
    Known {
        folder: "CertisignWebSigner",
        name: "Certisign WebSigner",
        vendor: "Certisign",
        used_by: "Certisign's signing portals",
    },
];

/// The known signers that are not installed.
pub fn missing(installed: &[Signer]) -> Vec<&'static Known> {
    KNOWN
        .iter()
        .filter(|k| !installed.iter().any(|s| s.folder == k.folder))
        .collect()
}

/// How a browser reads native messaging manifests.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Firefox,
    Chrome,
}

pub struct Browser {
    pub app: &'static ClientApp,
    pub kind: Kind,
    /// Its manifest folders, in ~/.var/app/<id>/.
    pub dirs: &'static [&'static str],
}

/// The Flatpak browsers whose plugins can start a signer.
pub const BROWSERS: [Browser; 5] = [
    Browser {
        app: &apps::FIREFOX,
        kind: Kind::Firefox,
        // Firefox 147+ reads its user folders under config/, older
        // releases under .mozilla/: both.
        dirs: &[
            ".mozilla/native-messaging-hosts",
            "config/mozilla/native-messaging-hosts",
        ],
    },
    Browser {
        app: &apps::CHROMIUM,
        kind: Kind::Chrome,
        dirs: &["config/chromium/NativeMessagingHosts"],
    },
    Browser {
        app: &apps::CHROME,
        kind: Kind::Chrome,
        dirs: &["config/google-chrome/NativeMessagingHosts"],
    },
    Browser {
        app: &apps::EDGE,
        kind: Kind::Chrome,
        dirs: &["config/microsoft-edge/NativeMessagingHosts"],
    },
    Browser {
        app: &apps::BRAVE,
        kind: Kind::Chrome,
        dirs: &["config/BraveSoftware/Brave-Browser/NativeMessagingHosts"],
    },
];

/// The installed Flatpak browsers.
pub fn installed_browsers() -> Vec<&'static Browser> {
    BROWSERS
        .iter()
        .filter(|b| paths::flatpak_installed(b.app.id))
        .collect()
}

/// Is the Flatpak installation the user's (true) or the system's (false)?
/// Flatpak keeps an extension under <installation>/runtime/<id>.
fn installation_root(user: bool) -> PathBuf {
    if user {
        paths::host_data_home().join("flatpak")
    } else {
        PathBuf::from("/var/lib/flatpak")
    }
}

/// From the source of the folder's mount (a line of /proc/self/mountinfo,
/// field 4: the path inside its file system), the installation the
/// extension is in: Some(true) for the user's, Some(false) for another.
fn installation_of_mount(root: &str) -> Option<bool> {
    if !root.contains("/flatpak/runtime/") {
        return None;
    }
    Some(root.contains("/.local/share/flatpak/runtime/"))
}

/// The mount source of a folder in Tessera's sandbox, from mountinfo.
fn mount_root(mountinfo: &str, mount_point: &Path) -> Option<String> {
    let unescape = |s: &str| s.replace("\\040", " ").replace("\\011", "\t");
    let wanted = mount_point.to_string_lossy();
    mountinfo.lines().rev().find_map(|line| {
        let fields: Vec<&str> = line.split(' ').collect();
        (fields.len() > 4 && unescape(fields[4]) == wanted).then(|| unescape(fields[3]))
    })
}

/// Is the extension in the user's installation? Its mount in Tessera's
/// sandbox says; else it is where Tessera is (the install command puts it
/// there), else the system's.
fn in_user_installation(s: &Signer) -> bool {
    std::fs::read_to_string("/proc/self/mountinfo")
        .ok()
        .and_then(|m| mount_root(&m, &s.dir))
        .and_then(|root| installation_of_mount(&root))
        .or_else(crate::catalog::tessera_installation)
        .unwrap_or(false)
}

/// Where Flatpak keeps the extension on the host (every version of it):
/// what a browser is given read access to.
pub fn host_root(s: &Signer) -> PathBuf {
    host_root_in(s, in_user_installation(s))
}

fn host_root_in(s: &Signer, user: bool) -> PathBuf {
    installation_root(user).join("runtime").join(s.extension())
}

/// The start script on the host, through the extension's current version
/// (Flatpak's "active" link, which follows its updates).
fn host_start_in(s: &Signer, user: bool) -> PathBuf {
    host_root_in(s, user)
        .join(std::env::consts::ARCH)
        .join("stable/active/files")
        .join(&s.exec)
}

fn json_string(s: &str) -> String {
    let mut out = String::from('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn json_list(items: &[String]) -> String {
    let items: Vec<String> = items.iter().map(|i| json_string(i)).collect();
    format!("[{}]", items.join(", "))
}

/// The native messaging manifest, on one line: the vendor's own, with the
/// path of the start script on the host.
pub fn manifest(s: &Signer, kind: Kind, start: &Path) -> String {
    let allowed = match kind {
        Kind::Chrome => format!(
            "\"allowed_origins\": {}",
            json_list(
                &s.chrome_extensions
                    .iter()
                    .map(|id| format!("chrome-extension://{id}/"))
                    .collect::<Vec<_>>()
            )
        ),
        Kind::Firefox => format!(
            "\"allowed_extensions\": {}",
            json_list(&s.firefox_extensions)
        ),
    };
    format!(
        "{{\"name\": {}, \"description\": \"PKI Connector\", \"path\": {}, \"type\": \"stdio\", {allowed}}}",
        json_string(&s.host),
        json_string(&start.to_string_lossy())
    )
}

/// The override's options for this signer in this browser.
fn override_options(s: &Signer, b: &Browser, root: &Path) -> Vec<String> {
    let mut opts = vec![format!("--filesystem={}:ro", root.display())];
    if let Some(p) = &s.persist {
        opts.push(format!("--persist={p}"));
    }
    if s.firefox_x11 && b.kind == Kind::Firefox {
        opts.push("--socket=x11".into());
    }
    opts
}

/// The command that sets up a signer in a browser, for a terminal on the
/// host: the browser's override, then the manifest in each of its folders.
/// It runs the same in bash, zsh and fish (no loops, no bash-only syntax).
pub fn command(s: &Signer, b: &Browser) -> String {
    let user = in_user_installation(s);
    command_with(s, b, &host_root_in(s, user), &host_start_in(s, user))
}

fn command_with(s: &Signer, b: &Browser, root: &Path, start: &Path) -> String {
    let dirs: Vec<String> = b
        .dirs
        .iter()
        .map(|d| format!("~/.var/app/{}/{d}", b.app.id))
        .collect();
    let files: Vec<String> = dirs
        .iter()
        .map(|d| format!("{d}/{}.json", s.host))
        .collect();
    format!(
        "flatpak override --user {} {} && mkdir -p {} && printf '%s\\n' '{}' | tee {} > /dev/null",
        override_options(s, b, root).join(" "),
        b.app.id,
        dirs.join(" "),
        manifest(s, b.kind, start),
        files.join(" ")
    )
}

/// The values of one key of the [Context] group of an override file.
fn override_values(text: &str, key: &str) -> Vec<String> {
    let mut context = false;
    let mut out = vec![];
    for line in text.lines().map(str::trim) {
        if line.starts_with('[') {
            context = line == "[Context]";
        } else if context {
            if let Some(v) = line.strip_prefix(key).and_then(|r| r.strip_prefix('=')) {
                out.extend(
                    v.split(';')
                        .map(str::trim)
                        .filter(|x| !x.is_empty())
                        .map(String::from),
                );
            }
        }
    }
    out
}

/// Does the browser's override (its text) have what this signer needs?
/// The manifest itself is in the browser's folder, which Tessera does not
/// see: the same command writes both.
fn override_has(text: &str, s: &Signer, b: &Browser, root: &Path) -> bool {
    let fs = override_values(text, "filesystems");
    let root_s = root.to_string_lossy();
    let home = paths::home();
    let tilde = root
        .strip_prefix(&home)
        .ok()
        .map(|r| format!("~/{}", r.to_string_lossy()));
    let readable = fs.iter().any(|f| {
        let path = f.strip_suffix(":ro").unwrap_or(f);
        path == root_s || tilde.as_deref() == Some(path)
    });
    let persisted = s
        .persist
        .as_ref()
        .map_or(true, |p| override_values(text, "persistent").contains(p));
    let x11 = !(s.firefox_x11 && b.kind == Kind::Firefox)
        || override_values(text, "sockets").iter().any(|v| v == "x11");
    readable && persisted && x11
}

/// Is this signer set up in this browser (its user override)?
pub fn is_set_up(s: &Signer, b: &Browser) -> bool {
    std::fs::read_to_string(paths::user_override(b.app.id))
        .is_ok_and(|text| override_has(&text, s, b, &host_root(s)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn webpki() -> Signer {
        Signer {
            folder: "WebPKI".into(),
            name: "Lacuna Web PKI".into(),
            vendor: "Lacuna Software".into(),
            host: "com.lacunasoftware.webpki".into(),
            exec: "start".into(),
            chrome_extensions: vec!["dcng".into(), "nede".into()],
            firefox_extensions: vec!["webpki@lacunasoftware.com".into()],
            persist: Some(".lacuna-webpki".into()),
            firefox_x11: true,
            option: Some("Dispositivos SafeSign AET".into()),
            dir: PathBuf::from("/app/lib/tessera/signers/WebPKI"),
        }
    }

    #[test]
    fn the_signer_files_in_the_repository_load() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("flatpak/extensions");
        let mut n = 0;
        for k in KNOWN {
            let s = load(&root.join(k.folder)).expect(k.folder);
            assert_eq!(s.name, k.name);
            assert_eq!(s.vendor, k.vendor);
            assert!(root.join(k.folder).join(&s.exec).is_file());
            assert!(!s.chrome_extensions.is_empty() && !s.firefox_extensions.is_empty());
            n += 1;
        }
        assert_eq!(n, 3);
    }

    #[test]
    fn the_host_path_follows_the_installation() {
        let s = webpki();
        assert_eq!(
            host_start_in(&s, false),
            PathBuf::from(format!(
                "/var/lib/flatpak/runtime/io.github.pedrohqb.Tessera.Signer.WebPKI/{}/stable/active/files/start",
                std::env::consts::ARCH
            ))
        );
        assert_eq!(
            installation_of_mount(
                "/home/a/.local/share/flatpak/runtime/io.github.pedrohqb.Tessera.Signer.WebPKI/x86_64/stable/abc/files"
            ),
            Some(true)
        );
        assert_eq!(
            installation_of_mount(
                "/ostree/deploy/default/var/lib/flatpak/runtime/io.github.pedrohqb.Tessera.Signer.WebPKI/x86_64/stable/abc/files"
            ),
            Some(false)
        );
        assert_eq!(installation_of_mount("/usr/lib"), None);
        let info = "1 0 0:1 / / rw - x x rw\n\
                    90 1 0:33 /var/lib/flatpak/runtime/io.github.pedrohqb.Tessera.Signer.WebPKI/x86_64/stable/abc/files /app/lib/tessera/signers/WebPKI ro,nosuid - btrfs /dev/x rw\n";
        assert_eq!(
            mount_root(info, Path::new("/app/lib/tessera/signers/WebPKI")).as_deref(),
            Some("/var/lib/flatpak/runtime/io.github.pedrohqb.Tessera.Signer.WebPKI/x86_64/stable/abc/files")
        );
        assert_eq!(
            mount_root(info, Path::new("/app/lib/tessera/signers/X")),
            None
        );
    }

    #[test]
    fn the_manifest_is_the_vendors_own() {
        let s = webpki();
        let start = Path::new("/r/files/start");
        assert_eq!(
            manifest(&s, Kind::Chrome, start),
            r#"{"name": "com.lacunasoftware.webpki", "description": "PKI Connector", "path": "/r/files/start", "type": "stdio", "allowed_origins": ["chrome-extension://dcng/", "chrome-extension://nede/"]}"#
        );
        assert!(manifest(&s, Kind::Firefox, start)
            .ends_with(r#""allowed_extensions": ["webpki@lacunasoftware.com"]}"#));
        assert_eq!(json_string("a\"b\\c"), r#""a\"b\\c""#);
    }

    /// The command runs in a POSIX shell (and so in bash and zsh; it has no
    /// loop or bash-only syntax, for fish): with a stand-in for flatpak, it
    /// writes the manifest, valid JSON, in each of the browser's folders.
    #[test]
    fn the_command_writes_the_manifests() {
        let s = webpki();
        let firefox = &BROWSERS[0];
        let root = Path::new("/var/lib/flatpak/runtime/io.github.pedrohqb.Tessera.Signer.WebPKI");
        let start = root.join("x86_64/stable/active/files/start");
        let cmd = command_with(&s, firefox, root, &start);
        assert!(cmd.starts_with(
            "flatpak override --user --filesystem=/var/lib/flatpak/runtime/io.github.pedrohqb.Tessera.Signer.WebPKI:ro --persist=.lacuna-webpki --socket=x11 org.mozilla.firefox && "
        ));
        assert!(!command_with(&s, &BROWSERS[2], root, &start).contains("x11"));

        let home = std::env::temp_dir().join(format!("tessera-signer-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        let bin = home.join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        let fake = bin.join("flatpak");
        std::fs::write(&fake, "#!/bin/sh\necho \"$@\" > \"$HOME/override\"\n").unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
        let ok = std::process::Command::new("sh")
            .arg("-c")
            .arg(&cmd)
            .env("HOME", &home)
            .env("PATH", format!("{}:/usr/bin:/bin", bin.display()))
            .status()
            .unwrap()
            .success();
        assert!(ok, "{cmd}");
        let want = format!("{}\n", manifest(&s, Kind::Firefox, &start));
        for d in firefox.dirs {
            let f = home
                .join(".var/app/org.mozilla.firefox")
                .join(d)
                .join("com.lacunasoftware.webpki.json");
            assert_eq!(std::fs::read_to_string(&f).unwrap(), want);
        }
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn the_override_shows_whether_it_is_set_up() {
        let s = webpki();
        let root = Path::new("/var/lib/flatpak/runtime/io.github.pedrohqb.Tessera.Signer.WebPKI");
        let (firefox, chrome) = (&BROWSERS[0], &BROWSERS[2]);
        let done = "[Context]\nfilesystems=xdg-run/p11-kit/pkcs11;/var/lib/flatpak/runtime/io.github.pedrohqb.Tessera.Signer.WebPKI:ro;\npersistent=.lacuna-webpki;\nsockets=x11;\n";
        assert!(override_has(done, &s, firefox, root));
        let no_x11 = done.replace("sockets=x11;\n", "");
        assert!(!override_has(&no_x11, &s, firefox, root));
        assert!(override_has(&no_x11, &s, chrome, root));
        let only_socket = "[Context]\nfilesystems=xdg-run/p11-kit/pkcs11;\n";
        assert!(!override_has(only_socket, &s, chrome, root));
        // The same keys in another group are not the context's.
        let elsewhere = done.replace("[Context]", "[Environment]");
        assert!(!override_has(&elsewhere, &s, chrome, root));
    }
}
