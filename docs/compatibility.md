# Compatibility

What has been checked, and how. "Automatically" means continuous integration checks it on every push; "by hand" means someone used it with a real token. Anything not listed is expected to work but has not been confirmed.

To add a line, open an issue with the **Compatibility report** template (or tell the [Telegram group](https://t.me/advogados_linux)): distribution and desktop, token or certificate, app (Flatpak or native) and what happened.

## Checked automatically (CI)

| What | Where | How |
|---|---|---|
| Serving a token to apps, signing with PSS and PKCS#1 v1.5, refusing a wrong PIN | Ubuntu 24.04 | SoftHSM token through `tessera serve`, `tessera remote` and `p11-kit-client.so` (`tests/drivers.rs`) |
| The SerproID defect (a driver that keeps the mechanism pointer) | Ubuntu 24.04 | Fake driver that fails without Tessera's copy (`tests/drivers.rs`) |
| A driver that never answers is not published | Ubuntu 24.04 | Fake driver (`tests/drivers.rs`) |
| The window's buttons: native Firefox, Okular and Papers, PJeOffice Pro, and the Firefox Flatpak's step 2 | Ubuntu 24.04 | Through the accessibility tree, on a made-up home folder (`tests/gui/test_window.py`) |
| The p11-kit path written into PJeOffice Pro's `list.a3` exists | Fedora, Debian 13, Ubuntu 24.04, Arch Linux, openSUSE Tumbleweed | Containers with each distribution's default packages (`detection_matches_this_system`) |
| Whether NSS already loads p11-kit (no Set up needed) matches Tessera's verdict | Same five | A fresh NSS database, as a browser makes it, listed with `modutil`. On Fedora, NSS loads p11-kit through crypto-policies; on the other four, it does not |
| The vendors' downloads still match their pinned SHA256 | — | Weekly workflow (`check-downloads.yml`); a mismatch opens an issue |
| The Flatpak builds, with every icon the window uses | GNOME 51 runtime | `flatpak-builder` |
| Each driver extension builds, installs on the real vendor downloads, and leaves its module and tools where `driver.toml` says | GNOME 51 runtime | `flatpak-builder`, then `flatpak install` (`ci.yml`) |
| A token's reader or ATR points to the right driver extension | — | `src/catalog.rs` tests (eToken 5110, StarSign CUT S, ePass2003) |

## Tested by hand

| Desktop | Token or certificate | App | Result |
|---|---|---|---|
| Bluefin (GNOME) | SafeSign (GD Burti) | Firefox, Chromium, Google Chrome, Microsoft Edge and Brave (Flatpak) | Token visible, PIN asked once (after the fix to the duplicate module, [History](history.md) item 35) |
| Bluefin (GNOME) | SafeSign (GD Burti) | Okular and Papers (Flatpak) | Signs PDFs |
| Bluefin (GNOME) | SafeSign (GD Burti) | PJeOffice Pro (FlatPark) | Signs, with no steps |
| Bluefin (GNOME) | SerproID | Firefox (Flatpak), TLS 1.3 sites | Works since the fix in `stable.rs` (see [Drivers](drivers.md)) |
| Bluefin (GNOME) | a real token (type not recorded) | Tessera 0.2.x: Flatpak and native apps | Works |
| Fedora 44 (VM) | SerproID | Native Firefox (RPM), nothing set up | Token visible through the system's own p11-kit (crypto-policies); Tessera shows it as ready, with no button |
| KDE Plasma | — | Window, icons and tray icon on a dark panel | Works |
| KDE Plasma | SerproID | Flatpak apps, several signatures in a row | Each "approve on your phone" notification shows (since the portal notification gets a new ID each time, [History](history.md) item 36) |
| KDE Plasma | a real token (type not recorded) | Native Firefox and Chromium-based browsers | Works after Set up |
| KDE Plasma | a real token (type not recorded) | Native PJeOffice Pro (`list.a3` with p11-kit) | Works |
| KDE Plasma | a real token (type not recorded) | Native Okular and Papers, through the native Firefox's database | Sign PDFs |
| Arch Linux (a user's report) | SafeNet | PJeOffice Pro | Works |
| openSUSE Tumbleweed | a real token (type not recorded) | Tessera from FlatPark, with `p11-kit-server` and its socket | Works |
| openSUSE Tumbleweed | a real token (type not recorded) | Native Firefox | Shows **Set up** (openSUSE's NSS does not load p11-kit on its own, as the CI check found); sees the token after Set up |
| Ubuntu 24.04 | — | The host's p11-kit server as a user service (Ubuntu ships no `p11-kit-server.socket`; see [usage](usage.md)) | Works (the connection only). Signing fails there as on MX Linux below, by p11-kit version (0.25.3): Ubuntu 24.04 is not supported |
| Ubuntu 26.04 | a real token (type not recorded) | p11-kit 0.26.2, with the server as a user service (no `p11-kit-server.socket`) | Works |
| Bluefin (GNOME) | SafeSign (GD Burti), through the SafeSign extension (test build) | Tessera's window and apps | Works. The token first hung while Tessera's OpenSC was on; OpenSC now ignores the "StarSign" readers ([History](history.md) item 42), and with OpenSC on again the token shows once, signs and no longer hangs |
| MX Linux (Debian 13), systemd | SafeSign and SerproID | Native Firefox, Brave (Flatpak) | **Signing fails** (`SSL_ERROR_HANDSHAKE_FAILED`; `sign-test.py` through the system's p11-kit gives `CKR 0x71` with SafeSign and `CKR 0x63` with SerproID, at once). The tokens show and the PIN is asked. Cause: the system's p11-kit 0.25.5 ([History](history.md) item 38). Not supported with MX's own p11-kit |
| MX Linux (Debian 13), systemd | SafeSign and SerproID | Native Firefox, Brave (Flatpak) | Works after updating p11-kit to 0.26.x from the MX Test Repo (backported by MX on request), which confirms the cause above |
| CachyOS | SafeSign (GD Burti), through the SafeSign extension (test build) | Master PDF Editor (Flatpak) | Signs, with step 2 done by Tessera's **Set up** (`pkcs11-helper-provider`) |
| CachyOS | SerproID (cloud), through the SerproID extension (test build) | Master PDF Editor (Flatpak) | Does not sign: Master PDF Editor apparently does not wait for the approval on the phone (to be confirmed) |
| CachyOS | SafeSign (GD Burti), through the SafeSign extension (test build) | JSignPdf (Flatpak) | Signs, with the keystore type PKCS11 and the PIN as the keystore password. With OpenSC on and no card in it, the token was missing for every app until Tessera listed its drivers without a token last ([History](history.md) item 43) |
| KDE Plasma (CachyOS) | a real token (type not recorded) | Web PKI and WebSigner, vendors' official packages, native browsers | Work with p11-kit's library (`/lib/libp11-kit.so`, the same module as `p11-kit-proxy.so` there) added by hand under the plugin's custom options |

Not yet confirmed by hand:

- **The apps added in November 2026** (see [Apps](apps.md)): Thunderbird and Evolution, as Flatpaks and native; LibreOffice, as a Flatpak and native; Assinador SERPRO, as a Flatpak. Their set-up is checked by unit tests on a made-up home folder, but they have not been tried with a real token yet (JSignPdf and Master PDF Editor have, above).
- **The OpenSC driver** (November 2026; see [Drivers](drivers.md#opensc)): built into Tessera, it serves every card OpenSC knows, the ePass2003 among them. Checked without a token (served, listed and published with no reader); not yet tried with a real one.
- **Snap** packages (Ubuntu's Firefox and Chromium): not supported.
