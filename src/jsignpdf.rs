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

use std::path::PathBuf;

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

/// Write the file, keeping any other one as pkcs11.cfg.before-tessera.
pub fn set_up(slot_index: usize) -> Result<(), String> {
    let dir = config_dir();
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let file = config_file();
    if let Ok(old) = std::fs::read_to_string(&file) {
        let (library, _) = parse(&old);
        if library.as_deref() != Some(paths::CLIENT_MODULE) {
            let keep = dir.join("pkcs11.cfg.before-tessera");
            std::fs::write(&keep, old).map_err(|e| format!("{}: {e}", keep.display()))?;
        }
    }
    std::fs::write(&file, config_text(slot_index)).map_err(|e| format!("{}: {e}", file.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

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
