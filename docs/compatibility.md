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

## Tested by hand

| Desktop | Token or certificate | App | Result |
|---|---|---|---|
| Bluefin (GNOME) | SafeSign (GD Burti) | Firefox, Chromium, Google Chrome, Microsoft Edge and Brave (Flatpak) | Token visible, PIN asked once (after the fix to the duplicate module, [History](history.md) item 35) |
| Bluefin (GNOME) | SafeSign (GD Burti) | Okular and Papers (Flatpak) | Signs PDFs |
| Bluefin (GNOME) | SafeSign (GD Burti) | PJeOffice Pro (FlatPark) | Signs, with no steps |
| Bluefin (GNOME) | SerproID | Firefox (Flatpak), TLS 1.3 sites | Works since the fix in `stable.rs` (see [Drivers](drivers.md)) |
| KDE Plasma | — | Window, icons and tray icon on a dark panel | Works |
| KDE Plasma | a real token (type not recorded) | Native Firefox and Chromium-based browsers | Works after Set up |
| KDE Plasma | a real token (type not recorded) | Native PJeOffice Pro (`list.a3` with p11-kit) | Works |

Not yet confirmed by hand:

- **SafeNet eToken 5100/5110** with a real token. Its download is checked every week, but no test with the token itself is recorded yet.
- **Native Okular and Papers** signing through Firefox's database.
- **Fedora's native apps** with nothing set up (crypto-policies): confirmed by CI on a fresh database, not yet in the browsers themselves.
- **Snap** packages (Ubuntu's Firefox and Chromium): not supported.
