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
//! must be closed while Tessera edits it: while it runs there is a .lock
//! file next to the profile (libreoffice/4/.lock). Tessera changes only the
//! CertDir item, and keeps the file as it was before its first change.

use std::path::{Path, PathBuf};

use crate::i18n::t;
use crate::paths;

pub const FLATPAK_ID: &str = "org.libreoffice.LibreOffice";

/// The native LibreOffice's user profile. openSUSE's build keeps it under
/// libreoffice/4-suse; elsewhere it is libreoffice/4.
pub fn native_profile() -> PathBuf {
    let base = paths::host_config_home().join("libreoffice");
    let suse = base.join("4-suse/user");
    if !base.join("4/user").is_dir() && suse.is_dir() {
        return suse;
    }
    base.join("4/user")
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

/// Is LibreOffice running with this profile? Its lock file is next to the
/// profile (libreoffice/4/.lock, not in user/). A crash may leave it.
pub fn running(profile: &Path) -> bool {
    profile
        .parent()
        .is_some_and(|dir| dir.join(".lock").exists())
}

const PATH: &str = r#"oor:path="/org.openoffice.Office.Common/Security/Scripting""#;
const PROP: &str = r#"oor:name="CertDir""#;

/// Where the CertDir item is: from its `<item` to the end of its `</item>`,
/// wherever LibreOffice (or a hand edit) broke the lines.
fn cert_dir_span(xcu: &str) -> Option<(usize, usize)> {
    let mut from = 0;
    while let Some(i) = xcu[from..].find("<item ") {
        let start = from + i;
        let end = start + xcu[start..].find("</item>")? + "</item>".len();
        let item = &xcu[start..end];
        if item.contains(PATH) && item.contains(PROP) {
            return Some((start, end));
        }
        from = end;
    }
    None
}

/// The CertDir in this registrymodifications.xcu, if set.
pub fn cert_dir_in(xcu: &str) -> Option<String> {
    let (start, end) = cert_dir_span(xcu)?;
    let item = &xcu[start..end];
    let v_start = item.find("<value>")? + "<value>".len();
    let v_end = item[v_start..].find("</value>")? + v_start;
    let v = unescape(item[v_start..v_end].trim());
    (!v.is_empty()).then_some(v)
}

/// The profile's CertDir: None when the profile is not there yet (LibreOffice
/// has never run), Some(None) when it is there without a CertDir.
pub fn cert_dir(profile: &Path) -> Option<Option<String>> {
    let text = std::fs::read_to_string(settings(profile)).ok()?;
    Some(cert_dir_in(&text))
}

/// The settings with CertDir set to `dir`: the CertDir item, if any, is
/// taken out (only that item), the rest is kept as it is, and the new item
/// goes before </oor:items>.
pub fn with_cert_dir(xcu: &str, dir: &Path) -> Option<String> {
    let item = format!(
        r#"<item {PATH}><prop {PROP} oor:op="fuse"><value>{}</value></prop></item>"#,
        escape(&dir.to_string_lossy())
    );
    let mut text = xcu.to_string();
    while let Some((start, mut end)) = cert_dir_span(&text) {
        if text[end..].starts_with('\n') {
            end += 1;
        }
        text.replace_range(start..end, "");
    }
    let close = text.rfind("</oor:items>")?;
    let (head, tail) = text.split_at(close);
    let mut out = head.to_string();
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    out.push_str(&item);
    out.push('\n');
    out.push_str(tail);
    Some(out)
}

/// Write CertDir into the profile. The caller checks that LibreOffice is
/// closed. The file is replaced in one step (a new file renamed over it),
/// and the one LibreOffice wrote before Tessera first changed it is kept as
/// registrymodifications.xcu.before-tessera.
pub fn set_cert_dir(profile: &Path, dir: &Path) -> Result<(), String> {
    let file = settings(profile);
    let fail = |e: std::io::Error| format!("{}: {e}", file.display());
    let text = std::fs::read_to_string(&file).map_err(fail)?;
    let new = with_cert_dir(&text, dir).ok_or_else(|| {
        format!(
            "{}: {}",
            file.display(),
            t("not a LibreOffice settings file")
        )
    })?;
    let keep = profile.join("registrymodifications.xcu.before-tessera");
    if !keep.exists() {
        std::fs::write(&keep, &text).map_err(|e| format!("{}: {e}", keep.display()))?;
    }
    let tmp = profile.join("registrymodifications.xcu.tessera-new");
    std::fs::write(&tmp, new).map_err(|e| format!("{}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, &file).map_err(fail)
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

    #[test]
    fn only_the_cert_dir_item_is_replaced() {
        // The item broken over lines, and sharing a line with another one.
        let split = r#"<oor:items>
<item oor:path="/org.openoffice.Office.Common/Misc"><prop oor:name="A" oor:op="fuse"><value>1</value></prop></item><item oor:path="/org.openoffice.Office.Common/Security/Scripting"><prop oor:name="CertDir" oor:op="fuse">
  <value>/old/db</value>
</prop></item><item oor:path="/org.openoffice.Office.Common/Misc"><prop oor:name="B" oor:op="fuse"><value>2</value></prop></item>
<item oor:path="/org.openoffice.Office.Common/Security/Scripting"><prop oor:name="MacroSecurityLevel" oor:op="fuse"><value>2</value></prop></item>
</oor:items>"#;
        assert_eq!(cert_dir_in(split).as_deref(), Some("/old/db"));
        let new = with_cert_dir(split, Path::new("/new/db")).unwrap();
        assert_eq!(cert_dir_in(&new).as_deref(), Some("/new/db"));
        assert!(!new.contains("/old/db"));
        for kept in [
            r#"<prop oor:name="A" oor:op="fuse"><value>1</value></prop></item>"#,
            r#"<item oor:path="/org.openoffice.Office.Common/Misc"><prop oor:name="B" oor:op="fuse"><value>2</value></prop></item>"#,
            r#"<prop oor:name="MacroSecurityLevel" oor:op="fuse"><value>2</value></prop>"#,
        ] {
            assert!(new.contains(kept), "{kept} lost:\n{new}");
        }
        assert_eq!(new.matches(r#"oor:name="CertDir""#).count(), 1);
    }

    #[test]
    fn the_lock_is_next_to_the_profile_and_the_old_file_is_kept() {
        let dir = std::env::temp_dir().join(format!("tessera-lo-{}", std::process::id()));
        let profile = dir.join("4/user");
        std::fs::create_dir_all(&profile).unwrap();
        assert!(!running(&profile));
        std::fs::write(dir.join("4/.lock"), b"").unwrap();
        assert!(running(&profile));
        std::fs::write(profile.join("registrymodifications.xcu"), XCU).unwrap();
        set_cert_dir(&profile, Path::new("/a")).unwrap();
        set_cert_dir(&profile, Path::new("/b")).unwrap();
        assert_eq!(cert_dir(&profile), Some(Some("/b".to_string())));
        let kept =
            std::fs::read_to_string(profile.join("registrymodifications.xcu.before-tessera"))
                .unwrap();
        assert_eq!(kept, XCU);
        assert!(!profile
            .join("registrymodifications.xcu.tessera-new")
            .exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
