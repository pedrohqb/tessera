//! JSignPdf (Flathub, io.github.intoolswetrust.JSignPdf): a Java PDF signer
//! whose PKCS#11 keystore is Java's SunPKCS11 (and its JSignPKCS11 fork),
//! configured by one file, `<config>/pkcs11.cfg`. JSignPdf 3 takes its
//! config folder from $JSIGNPDF_CONFIG_DIR, else $XDG_CONFIG_HOME/jsignpdf;
//! in the Flatpak, ~/.var/app/<id>/config/jsignpdf. It reads the file once,
//! at startup.
//!
//! The file names one library, here the runtime's p11-kit-client.so, which
//! reaches the system's p11-kit server after step 1. SunPKCS11 opens a
//! single slot: without `slotListIndex`, the first of all the slots
//! (C_GetSlotList(CK_FALSE)), which on the system's server is p11-kit's
//! trust store, with no keys. So Tessera writes the position of the first
//! slot holding one of its tokens (status::host_slot_index). That position
//! changes when the system's modules or readers change; the row then shows
//! step 2 as missing again, and "Set up" writes the new one.
//!
//! The file only adds the PKCS11 keystore type: JSignPdf still opens its
//! default (JKS, a file) until "Keystore type" is set to PKCS11, which it
//! keeps in `<config>/config.properties` (`keystore.type`, Java properties).
//! Tessera sets that line too, keeping the rest of the file; JSignPdf
//! writes the whole file when it closes, so it must be closed first.

use std::path::{Path, PathBuf};

use crate::i18n::t;
use crate::paths;

pub const FLATPAK_ID: &str = "io.github.intoolswetrust.JSignPdf";

/// The app's config folder (the same path inside its sandbox).
pub fn config_dir() -> PathBuf {
    paths::home()
        .join(".var/app")
        .join(FLATPAK_ID)
        .join("config/jsignpdf")
}

pub fn config_file() -> PathBuf {
    config_dir().join("pkcs11.cfg")
}

/// JSignPdf's own settings (Java properties), where it keeps the keystore
/// type chosen in its window.
pub fn main_config_file() -> PathBuf {
    config_dir().join("config.properties")
}

const KSTYPE_KEY: &str = "keystore.type";
const PKCS11: &str = "PKCS11";

/// The logical lines of a properties file that set a key: (index of the
/// first physical line, index after the last, key, value). A line ending in
/// an odd number of backslashes goes on in the next one.
fn properties(text: &str) -> Vec<(usize, usize, String, String)> {
    let lines: Vec<&str> = text.lines().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let start = i;
        let mut logical = String::new();
        loop {
            let part = if i == start {
                lines[i]
            } else {
                lines[i].trim_start()
            };
            let trailing = part.len() - part.trim_end_matches('\\').len();
            i += 1;
            if trailing % 2 == 1 && i < lines.len() {
                logical.push_str(&part[..part.len() - 1]);
            } else {
                logical.push_str(part);
                break;
            }
        }
        let l = logical.trim_start();
        if l.is_empty() || l.starts_with('#') || l.starts_with('!') {
            continue;
        }
        let end = l
            .char_indices()
            .find(|&(_, c)| c == '=' || c == ':' || c.is_whitespace())
            .map_or(l.len(), |(n, _)| n);
        let key = l[..end].to_string();
        let rest = l[end..].trim_start();
        let rest = rest
            .strip_prefix('=')
            .or_else(|| rest.strip_prefix(':'))
            .unwrap_or(rest);
        out.push((start, i, key, rest.trim().to_string()));
    }
    out
}

/// Does JSignPdf's config.properties choose the PKCS11 keystore type?
pub fn chooses_pkcs11(text: &str) -> bool {
    properties(text)
        .iter()
        .rev()
        .find(|(_, _, k, _)| k == KSTYPE_KEY)
        .is_some_and(|(_, _, _, v)| v == PKCS11)
}

/// The file with `keystore.type=PKCS11` in place of its own keystore type
/// (or added at the end), everything else as it was.
pub fn choosing_pkcs11(text: &str) -> String {
    let wanted = format!("{KSTYPE_KEY}={PKCS11}");
    let found: Vec<(usize, usize)> = properties(text)
        .into_iter()
        .filter(|(_, _, k, _)| k == KSTYPE_KEY)
        .map(|(a, b, _, _)| (a, b))
        .collect();
    let mut out = Vec::new();
    let mut placed = false;
    for (n, line) in text.lines().enumerate() {
        match found.iter().find(|(a, b)| (*a..*b).contains(&n)) {
            Some((a, _)) if *a == n && !placed => {
                out.push(wanted.clone());
                placed = true;
            }
            Some(_) => {}
            None => out.push(line.to_string()),
        }
    }
    if !placed {
        out.push(wanted);
    }
    let mut s = out.join("\n");
    s.push('\n');
    s
}

/// What Tessera writes.
pub fn config_text(slot_index: usize) -> String {
    format!(
        "# Written by Tessera: its tokens through the system's p11-kit server.\n\
         name=Tessera\n\
         library={}\n\
         slotListIndex={slot_index}\n",
        paths::CLIENT_MODULE
    )
}

/// The library and slotListIndex a pkcs11.cfg names.
pub fn parse(text: &str) -> (Option<String>, Option<usize>) {
    let mut library = None;
    let mut index = None;
    for line in text.lines().map(str::trim) {
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        match k.trim() {
            "library" => library = Some(v.trim().to_string()),
            "slotListIndex" => index = v.trim().parse().ok(),
            _ => {}
        }
    }
    (library, index)
}

/// Is the file Tessera's, for this slot? `current` is the position Tessera
/// sees now, when it can tell (the system's server answering with one of
/// its tokens); without it, the library alone decides.
pub fn is_set_up(text: &str, current: Option<usize>) -> bool {
    let (library, index) = parse(text);
    library.as_deref() == Some(paths::CLIENT_MODULE)
        && index.is_some()
        && current.map_or(true, |c| index == Some(c))
}

/// Are both files Tessera's (see is_set_up)?
pub fn ready(current: Option<usize>) -> bool {
    std::fs::read_to_string(config_file()).is_ok_and(|t| is_set_up(&t, current))
        && std::fs::read_to_string(main_config_file()).is_ok_and(|t| chooses_pkcs11(&t))
}

/// Write pkcs11.cfg, keeping any other one as pkcs11.cfg.before-tessera,
/// and choose the PKCS11 keystore type in config.properties, keeping the
/// file as it was as config.properties.before-tessera.
pub fn set_up(slot_index: usize) -> Result<(), String> {
    let dir = config_dir();
    // Tessera sees the app's config folder only if it was there when
    // Tessera started (its sandbox mounts what exists).
    if !dir.parent().is_some_and(Path::is_dir) {
        return Err(format!(
            "JSignPdf: {}",
            t("open the app once, then restart Tessera and try again")
        ));
    }
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let file = config_file();
    if let Ok(old) = std::fs::read_to_string(&file) {
        let (library, _) = parse(&old);
        if library.as_deref() != Some(paths::CLIENT_MODULE) {
            let keep = dir.join("pkcs11.cfg.before-tessera");
            std::fs::write(&keep, old).map_err(|e| format!("{}: {e}", keep.display()))?;
        }
    }
    std::fs::write(&file, config_text(slot_index))
        .map_err(|e| format!("{}: {e}", file.display()))?;
    let main = main_config_file();
    let old = std::fs::read_to_string(&main).unwrap_or_default();
    if !chooses_pkcs11(&old) {
        if !old.is_empty() {
            let keep = dir.join("config.properties.before-tessera");
            std::fs::write(&keep, &old).map_err(|e| format!("{}: {e}", keep.display()))?;
        }
        std::fs::write(&main, choosing_pkcs11(&old))
            .map_err(|e| format!("{}: {e}", main.display()))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chooses_the_pkcs11_keystore_type() {
        assert!(!chooses_pkcs11(""));
        let saved = "#Properties saved by PropertyProvider\n#Sun Nov 08 01:40:00 BRT 2026\nkeystore.type=JKS\nkeystore.file=\nview.advanced=false\n";
        assert!(!chooses_pkcs11(saved));
        let ours = choosing_pkcs11(saved);
        assert!(chooses_pkcs11(&ours));
        assert_eq!(
            ours,
            "#Properties saved by PropertyProvider\n#Sun Nov 08 01:40:00 BRT 2026\nkeystore.type=PKCS11\nkeystore.file=\nview.advanced=false\n"
        );
        // Added when missing; other separators and continued lines read.
        assert_eq!(choosing_pkcs11("a=1\n"), "a=1\nkeystore.type=PKCS11\n");
        assert_eq!(choosing_pkcs11(""), "keystore.type=PKCS11\n");
        assert!(chooses_pkcs11("keystore.type : PKCS11\n"));
        assert!(!chooses_pkcs11("x=a\\\n  keystore.type=PKCS11\n"));
        assert_eq!(
            choosing_pkcs11("keystore.type=PKCS\\\n  12\nb=2\n"),
            "keystore.type=PKCS11\nb=2\n"
        );
    }

    #[test]
    fn the_config_names_the_client_and_the_slot() {
        let text = config_text(3);
        assert_eq!(
            parse(&text),
            (Some(paths::CLIENT_MODULE.to_string()), Some(3))
        );
        assert!(is_set_up(&text, Some(3)));
        assert!(is_set_up(&text, None));
        assert!(!is_set_up(&text, Some(2)));
        let other = "name=JSignPdf\nlibrary=/usr/lib/libeToken.so\nslot=1\n";
        assert!(!is_set_up(other, None));
        assert!(!is_set_up(
            &format!("library={}\n", paths::CLIENT_MODULE),
            None
        ));
    }
}
