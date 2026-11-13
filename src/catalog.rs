//! The drivers published as Flatpak extensions of Tessera
//! (io.github.pedrohqb.Tessera.Driver.<Name>, see flatpak/extensions): what
//! the window offers to install when one is missing, and which one a
//! connected token seems to need.
//!
//! The vendors' drivers are not part of Tessera's own package; each user
//! installs the one for their token. A token is recognized, roughly, by the
//! name of its reader (a USB token is its own reader, named after its USB
//! product string) or by bytes of its ATR. The rules are deliberately few:
//! a wrong guess only shows a suggestion.

use crate::drivers::Driver;

pub struct Known {
    /// The driver's id once installed (the extension's folder, lowercased).
    pub id: &'static str,
    pub name: &'static str,
    pub vendor: &'static str,
    /// The tokens it is for, in the user's words.
    pub tokens: &'static str,
    pub extension: &'static str,
    /// Words of a reader's name that mean this driver's tokens.
    reader_words: &'static [&'static str],
    /// Byte runs of an ATR that mean this driver's cards.
    atr_bytes: &'static [&'static [u8]],
    /// Still being tried out: the window marks it "Experimental".
    pub experimental: bool,
}

pub const KNOWN: &[Known] = &[
    Known {
        id: "safesign",
        name: "SafeSign IC",
        vendor: "A.E.T. Europe",
        tokens: "GD Burti (StarSign) and other SafeSign tokens",
        extension: "io.github.pedrohqb.Tessera.Driver.SafeSign",
        reader_words: &["StarSign", "Giesecke", "G&D", "GD Burti"],
        // "SCE6"/"SCE7": G&D Sm@rtCafé Expert 6 and 7, the StarSign tokens'
        // chips; "sfte": SafeSign's own applet.
        atr_bytes: &[b"SCE6", b"SCE7", b"sfte"],
        experimental: false,
    },
    Known {
        id: "safenet",
        name: "SafeNet Authentication Client",
        vendor: "Thales",
        tokens: "SafeNet eToken 5100/5110 and IDPrime cards",
        extension: "io.github.pedrohqb.Tessera.Driver.SafeNet",
        reader_words: &["eToken", "SafeNet", "IDPrime"],
        // IDPrime's historical bytes (80 31 80 65 B0), and the eToken Java
        // cards' (80 73 C8 21 10).
        atr_bytes: &[
            &[0x80, 0x31, 0x80, 0x65, 0xB0],
            &[0x80, 0x73, 0xC8, 0x21, 0x10],
        ],
        experimental: false,
    },
    Known {
        id: "serproid",
        name: "SerproID",
        vendor: "Serpro",
        tokens: "Serpro's cloud certificate (no token to connect)",
        extension: "io.github.pedrohqb.Tessera.Driver.SerproID",
        reader_words: &[],
        atr_bytes: &[],
        experimental: false,
    },
    // An independent driver (GPL-3.0, github.com/LLawli/RemoteID-linux),
    // not Certisign's own, packaged on FlatPark by its developer (see
    // docs/drivers.md).
    Known {
        id: "remoteid",
        name: "RemoteID",
        vendor: "Certisign",
        tokens: "Certisign's cloud certificate, independent driver (no token to connect)",
        extension: "io.github.pedrohqb.Tessera.Driver.RemoteID",
        reader_words: &[],
        atr_bytes: &[],
        experimental: true,
    },
];

/// The known driver with this id, installed or not.
pub fn known(id: &str) -> Option<&'static Known> {
    KNOWN.iter().find(|k| k.id == id)
}

/// The known drivers that are not installed.
pub fn missing(installed: &[Driver]) -> Vec<&'static Known> {
    KNOWN
        .iter()
        .filter(|k| !installed.iter().any(|d| d.id == k.id))
        .collect()
}

/// The known driver a reader (and the card in it) seems to need.
pub fn for_reader(name: &str, atr: &[u8]) -> Option<&'static Known> {
    let lower = name.to_lowercase();
    KNOWN.iter().find(|k| {
        k.reader_words
            .iter()
            .any(|w| lower.contains(&w.to_lowercase()))
            || k.atr_bytes
                .iter()
                .any(|run| !atr.is_empty() && atr.windows(run.len()).any(|w| w == *run))
    })
}

/// The driver's page on FlatPark's site (its install button opens the app
/// store; the store also lists it as an add-on on Tessera's page).
pub fn store_page(k: &Known) -> String {
    store_page_of(k.extension)
}

/// An extension's page on FlatPark's site (drivers and web signers).
pub fn store_page_of(extension: &str) -> String {
    format!("https://flatpark.org/apps/{extension}/")
}

/// The command that installs a driver, in the installation Tessera itself
/// is in (an extension of a user installation's app goes there too).
pub fn install_command(k: &Known) -> String {
    install_command_of(k.extension)
}

/// The command that installs an extension of Tessera (a driver or a web
/// signer), in Tessera's installation.
pub fn install_command_of(extension: &str) -> String {
    let scope = match tessera_installation() {
        Some(true) => " --user",
        Some(false) => " --system",
        None => "",
    };
    format!("flatpak install{scope} flatpark {extension}")
}

/// Is Tessera's Flatpak in the user installation (Some(true)), the default
/// system one (Some(false)), or elsewhere: another installation, or not a
/// Flatpak (None)? /.flatpak-info's app-path says.
pub fn tessera_installation() -> Option<bool> {
    let info = std::fs::read_to_string("/.flatpak-info").ok()?;
    let path = info.lines().find_map(|l| l.strip_prefix("app-path="))?;
    installation_of(path, &crate::paths::host_data_home())
}

fn installation_of(app_path: &str, user_data: &std::path::Path) -> Option<bool> {
    let p = std::path::Path::new(app_path);
    if p.starts_with(user_data.join("flatpak")) {
        Some(true)
    } else if p.starts_with("/var/lib/flatpak") {
        Some(false)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_are_recognized_by_reader_or_atr() {
        let id = |n: &str, atr: &[u8]| for_reader(n, atr).map(|k| k.id);
        assert_eq!(
            id("SafeNet eToken 5100 [eToken 5110 SC] 00 00", &[]),
            Some("safenet")
        );
        assert_eq!(
            id("Giesecke & Devrient GmbH StarSign CUT S 00 00", &[]),
            Some("safesign")
        );
        // A card in a plain reader: by its ATR (eToken 5110 SC; StarSign CUT S).
        let etoken = [
            0x3B, 0xFF, 0x96, 0x00, 0x00, 0x81, 0x31, 0xFE, 0x43, 0x80, 0x31, 0x80, 0x65, 0xB0,
            0x85, 0x59, 0x56, 0xFB, 0x12, 0x0F, 0xFE, 0x82, 0x90, 0x00, 0x00,
        ];
        assert_eq!(
            id("Generic USB Smart Card Reader 00 00", &etoken),
            Some("safenet")
        );
        let starsign = [
            0x3B, 0xF9, 0x96, 0x00, 0x00, 0x81, 0x31, 0xFE, 0x45, 0x53, 0x43, 0x45, 0x37, 0x20,
            0x0E, 0x00, 0x20, 0x20, 0x28,
        ];
        assert_eq!(
            id("Generic USB Smart Card Reader 00 00", &starsign),
            Some("safesign")
        );
        // OpenSC's own (ePass2003) and unknown ones: nothing to install.
        assert_eq!(id("Feitian ePass2003 00 00", &[0x3B, 0x9F, 0x95]), None);
        assert_eq!(id("Generic USB Smart Card Reader 00 00", &[]), None);
    }

    #[test]
    fn the_installation_comes_from_the_app_path() {
        let home = std::path::Path::new("/home/a/.local/share");
        let user =
            "/home/a/.local/share/flatpak/app/io.github.pedrohqb.Tessera/x86_64/stable/abc/files";
        assert_eq!(installation_of(user, home), Some(true));
        let custom = std::path::Path::new("/data/xdg");
        assert_eq!(
            installation_of("/data/xdg/flatpak/app/x/files", custom),
            Some(true)
        );
        assert_eq!(
            installation_of("/var/lib/flatpak/app/x/files", home),
            Some(false)
        );
        assert_eq!(installation_of("/opt/flatpak/app/x/files", home), None);
    }

    #[test]
    fn remoteid_is_listed_as_experimental() {
        let k = known("remoteid").unwrap();
        assert!(k.experimental);
        assert_eq!(k.extension, "io.github.pedrohqb.Tessera.Driver.RemoteID");
        assert!(KNOWN.iter().filter(|k| k.experimental).count() == 1);
        // A cloud certificate: no reader means it.
        assert_eq!(for_reader("RemoteID", &[]).map(|k| k.id), None);
    }

    #[test]
    fn the_command_names_the_extension() {
        let c = install_command(&KNOWN[1]);
        assert!(c.starts_with("flatpak install"));
        assert!(c.ends_with("flatpark io.github.pedrohqb.Tessera.Driver.SafeNet"));
    }
}
