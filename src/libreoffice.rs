//! LibreOffice: which NSS database it signs with, and how Tessera points it
//! at one.
//!
//! LibreOffice opens one NSS database per run (xmlsecurity's
//! nssinitializer.cxx). The first of these wins: the MOZILLA_CERTIFICATE_FOLDER
//! variable; the user's "Certificate Path" (Tools → Options → Security), kept
//! as CertDir in registrymodifications.xcu, a plain path that must exist;
//! the default profile of Thunderbird, then Firefox, under ~/.thunderbird
//! and ~/.mozilla/firefox only (not Firefox 147's ~/.config/mozilla); and
//! otherwise a temporary database that is deleted on exit. It never loads
//! p11-kit by itself: only the modules in the database's pkcs11.txt, plus the
//! root-certificates module (libnssckbi.so) when the database has no roots.
//!
//! So Tessera sets CertDir, which does not depend on which browsers or mail
//! clients are there:
//!
//! - Flatpak (org.libreoffice.LibreOffice, freedesktop runtime, system NSS):
//!   an empty database in the app's own folder, like Papers. Its
//!   libnssckbi.so is p11-kit's trust module, which reaches the system's
//!   p11-kit server once step 1 is done; registering p11-kit-client.so as
//!   well would list each token twice.
//! - Native: a database in Tessera's own data folder with p11-kit-proxy.so,
//!   unless CertDir already names a database, which then gets the module
//!   instead (the user's choice is kept).
//!
//! LibreOffice writes registrymodifications.xcu again when it closes, so it
//! must be closed while Tessera edits it. Its profile has a .lock file while
//! it runs.

use std::path::{Path, PathBuf};

use crate::paths;

pub const FLATPAK_ID: &str = "org.libreoffice.LibreOffice";

/// The native LibreOffice's user profile.
pub fn native_profile() -> PathBuf {
    paths::host_config_home().join("libreoffice/4/user")
}

/// The Flatpak's user profile (its $XDG_CONFIG_HOME is in its own folder).
pub fn flatpak_profile() -> PathBuf {
    paths::home()
        .join(".var/app")
        .join(FLATPAK_ID)
        .join("config/libreoffice/4/user")
}

/// The empty database Tessera makes for the Flatpak, in the app's own
/// folder (the same path inside its sandbox).
pub fn flatpak_db() -> PathBuf {
    paths::home()
        .join(".var/app")
        .join(FLATPAK_ID)
        .join("data/tessera-nssdb")
}

/// The database Tessera makes for the native LibreOffice, in Tessera's own
/// data folder (in the Flatpak, ~/.var/app/io.github.pedrohqb.Tessera/data,
/// which the native LibreOffice can read).
pub fn native_db() -> PathBuf {
    paths::data_home().join("tessera/libreoffice-nssdb")
}

fn settings(profile: &Path) -> PathBuf {
    profile.join("registrymodifications.xcu")
}

/// Is LibreOffice running with this profile? (A crash may leave the lock.)
pub fn running(profile: &Path) -> bool {
    profile.join(".lock").exists()
}

const ITEM: &str =
    r#"<item oor:path="/org.openoffice.Office.Common/Security/Scripting"><prop oor:name="CertDir""#;

/// The CertDir in this registrymodifications.xcu, if set.
pub fn cert_dir_in(xcu: &str) -> Option<String> {
    let line = xcu.lines().find(|l| l.trim_start().starts_with(ITEM))?;
    let start = line.find("<value>")? + "<value>".len();
    let end = line[start..].find("</value>")? + start;
    let v = unescape(&line[start..end]);
    (!v.is_empty()).then_some(v)
}

/// The profile's CertDir: None when the profile is not there yet (LibreOffice
/// has never run), Some(None) when it is there without a CertDir.
pub fn cert_dir(profile: &Path) -> Option<Option<String>> {
    let text = std::fs::read_to_string(settings(profile)).ok()?;
    Some(cert_dir_in(&text))
}

/// The settings with CertDir set to `dir`: any CertDir item is replaced, the
/// rest is kept, and the item goes before </oor:items>.
pub fn with_cert_dir(xcu: &str, dir: &Path) -> Option<String> {
    let item = format!(
        r#"{ITEM} oor:op="fuse"><value>{}</value></prop></item>"#,
        escape(&dir.to_string_lossy())
    );
    let close = xcu.rfind("</oor:items>")?;
    let (head, tail) = xcu.split_at(close);
    let mut out: String = head
        .lines()
        .filter(|l| !l.trim_start().starts_with(ITEM))
        .map(|l| format!("{l}\n"))
        .collect();
    out.push_str(&item);
    out.push('\n');
    out.push_str(tail);
    Some(out)
}

/// Write CertDir into the profile. The caller checks that LibreOffice is
/// closed.
pub fn set_cert_dir(profile: &Path, dir: &Path) -> Result<(), String> {
    let file = settings(profile);
    let text = std::fs::read_to_string(&file).map_err(|e| format!("{}: {e}", file.display()))?;
    let new = with_cert_dir(&text, dir)
        .ok_or_else(|| format!("{}: not a LibreOffice settings file", file.display()))?;
    std::fs::write(&file, new).map_err(|e| format!("{}: {e}", file.display()))
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn unescape(s: &str) -> String {
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

#[cfg(test)]
mod tests {
    use super::*;

    const XCU: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<oor:items xmlns:oor="http://openoffice.org/2001/registry" xmlns:xs="http://www.w3.org/2001/XMLSchema" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
<item oor:path="/org.openoffice.Office.Common/Misc"><prop oor:name="FirstRun" oor:op="fuse"><value>false</value></prop></item>
</oor:items>
"#;

    #[test]
    fn cert_dir_is_read_and_written() {
        assert_eq!(cert_dir_in(XCU), None);
        let one = with_cert_dir(XCU, Path::new("/home/a & b/nssdb")).unwrap();
        assert_eq!(cert_dir_in(&one).as_deref(), Some("/home/a & b/nssdb"));
        assert!(one.contains("<value>/home/a &amp; b/nssdb</value>"));
        assert!(one.contains(r#"<prop oor:name="FirstRun""#));
        assert!(one.trim_end().ends_with("</oor:items>"));
        // Replaced, not repeated.
        let two = with_cert_dir(&one, Path::new("/x/db")).unwrap();
        assert_eq!(cert_dir_in(&two).as_deref(), Some("/x/db"));
        assert_eq!(two.matches(r#"oor:name="CertDir""#).count(), 1);
        assert_eq!(with_cert_dir("not xml", Path::new("/x")), None);
    }
}
