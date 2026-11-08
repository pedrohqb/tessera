//! Master PDF Editor (Flathub, net.code_industry.MasterPDFEditor, Qt 6): it
//! loads one PKCS#11 library, chosen in Tools → Settings → Certificates →
//! "PKCS#11 provider", and keeps it in its settings file (Qt's INI format),
//! ~/.var/app/<id>/config/Code Industry/Master PDF Editor 5.conf, as
//! `pkcs11-helper-provider=` in [General]. Tessera writes p11-kit-client.so
//! there, which reaches the system's p11-kit server after step 1.
//!
//! Master PDF Editor writes the whole file again when it closes, so it must
//! be closed first. Tessera changes only that line, and keeps the file as it
//! was before its first change.

use std::path::PathBuf;

use crate::i18n::t;
use crate::paths;

pub const FLATPAK_ID: &str = "net.code_industry.MasterPDFEditor";

const KEY: &str = "pkcs11-helper-provider";

/// The settings folder (Tessera sees it only if it was there when Tessera
/// started).
pub fn config_dir() -> PathBuf {
    paths::home()
        .join(".var/app")
        .join(FLATPAK_ID)
        .join("config/Code Industry")
}

pub fn config_file() -> PathBuf {
    config_dir().join("Master PDF Editor 5.conf")
}

/// The library [General] names, if any.
pub fn provider(text: &str) -> Option<String> {
    let mut general = false;
    let mut found = None;
    for line in text.lines().map(str::trim) {
        if line.starts_with('[') {
            general = line == "[General]";
        } else if general {
            if let Some((k, v)) = line.split_once('=') {
                if k.trim() == KEY {
                    found = Some(v.trim().to_string());
                }
            }
        }
    }
    found
}

pub fn is_set_up(text: &str) -> bool {
    provider(text).as_deref() == Some(paths::CLIENT_MODULE)
}

/// The file with p11-kit-client.so as the provider, everything else as it
/// was: the line replaced where it is, else added at the end of [General]
/// (made first in the file when there is none).
pub fn with_client(text: &str) -> String {
    let wanted = format!("{KEY}={}", paths::CLIENT_MODULE);
    let lines: Vec<&str> = text.lines().collect();
    let mut out: Vec<String> = Vec::new();
    let mut general = false;
    let mut in_general = false;
    let mut placed = false;
    for line in &lines {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            if in_general && !placed {
                // Before the blank lines that end [General].
                let at = out
                    .iter()
                    .rposition(|l| !l.trim().is_empty())
                    .map_or(out.len(), |n| n + 1);
                out.insert(at, wanted.clone());
                placed = true;
            }
            in_general = trimmed == "[General]";
            general |= in_general;
        } else if in_general
            && trimmed
                .split_once('=')
                .is_some_and(|(k, _)| k.trim() == KEY)
        {
            if !placed {
                out.push(wanted.clone());
                placed = true;
            }
            continue;
        }
        out.push(line.to_string());
    }
    if !placed {
        if general {
            let at = out
                .iter()
                .rposition(|l| !l.trim().is_empty())
                .map_or(out.len(), |n| n + 1);
            out.insert(at, wanted);
        } else {
            out.insert(0, "[General]".to_string());
            out.insert(1, wanted);
            if lines.first().is_some_and(|l| !l.trim().is_empty()) {
                out.insert(2, String::new());
            }
        }
    }
    let mut s = out.join("\n");
    s.push('\n');
    s
}

/// Set p11-kit-client.so as the provider, keeping the file as it was as
/// "Master PDF Editor 5.conf.before-tessera" (once: an earlier copy is the
/// user's own setting).
pub fn set_up() -> Result<(), String> {
    let file = config_file();
    let Ok(old) = std::fs::read_to_string(&file) else {
        return Err(format!(
            "Master PDF Editor: {}",
            t("open the app once, then restart Tessera and try again")
        ));
    };
    if is_set_up(&old) {
        return Ok(());
    }
    let keep = config_dir().join("Master PDF Editor 5.conf.before-tessera");
    if !keep.exists() {
        std::fs::write(&keep, &old).map_err(|e| format!("{}: {e}", keep.display()))?;
    }
    std::fs::write(&file, with_client(&old)).map_err(|e| format!("{}: {e}", file.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sets_the_provider_in_general() {
        let client = format!("{KEY}={}", paths::CLIENT_MODULE);
        let conf = "[General]\nlang=pt\npkcs11-helper-provider=/usr/lib/libaetpkss.so\nzoom=1\n\n[Signature]\nx=1\n";
        assert_eq!(provider(conf).as_deref(), Some("/usr/lib/libaetpkss.so"));
        assert!(!is_set_up(conf));
        let ours = with_client(conf);
        assert!(is_set_up(&ours));
        assert_eq!(
            ours,
            format!("[General]\nlang=pt\n{client}\nzoom=1\n\n[Signature]\nx=1\n")
        );
        // Added at the end of [General], before the next section.
        assert_eq!(
            with_client("[General]\nlang=pt\n\n[Signature]\nx=1\n"),
            format!("[General]\nlang=pt\n{client}\n\n[Signature]\nx=1\n")
        );
        assert_eq!(
            with_client("[General]\nlang=pt\n"),
            format!("[General]\nlang=pt\n{client}\n")
        );
        // A key of the same name elsewhere is not the provider.
        let other = "[Signature]\npkcs11-helper-provider=/x.so\n";
        assert_eq!(provider(other), None);
        assert_eq!(
            with_client(other),
            format!("[General]\n{client}\n\n[Signature]\npkcs11-helper-provider=/x.so\n")
        );
        assert!(is_set_up(&with_client("")));
    }
}
