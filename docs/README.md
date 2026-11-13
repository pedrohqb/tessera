# Tessera documentation

Tessera is a PKCS#11 token hub for Flatpak. It runs the token drivers (OpenSC, built in, and, as extensions, the vendors' SafeSign/GD Burti, SafeNet and SerproID, and RemoteID, an experimental independent driver made by reverse engineering Certisign's service) inside its own sandbox and hands the tokens to the other apps through p11-kit, without installing any vendor package on the system. It was made for immutable systems (Bluefin, Aurora, Bazzite, Fedora Silverblue and Kinoite), where installing a vendor's `.deb` or `.rpm` on the system is hard or impossible.

This documentation records how Tessera works, why it is the way it is, and everything that was discovered while making it work with real tokens. Installing it is covered in the [README](../README.md#english); the questions users ask most are in the [FAQ](../FAQ.md#english).

| Document | What it contains |
|---|---|
| [Usage](usage.md) | The window, the tray, automatic start and the command line |
| [Apps](apps.md) | PJeOffice Pro, Assinador SERPRO, Firefox, Thunderbird, Evolution, LibreOffice, JSignPdf, Master PDF Editor, the Chromium browsers, Papers and Okular: what each one needs and why |
| [Web signers](signers.md) | Web PKI, Softplan WebSigner and Certisign WebSigner as extensions: how the Flatpak browsers run them with Tessera's tokens, and the one command per browser |
| [Drivers](drivers.md) | Each driver: origin, verification, "clean" unpacking, dependencies and the quirks found (above all in SerproID) |
| [Troubleshooting](troubleshooting.md) | Where the logs are, common problems, the test tools and the path for investigating a problem |
| [Compatibility](compatibility.md) | What is checked automatically on each push, and what has been tested by hand with real tokens |
| [Architecture](architecture.md) | How the tokens leave Tessera's sandbox and reach the apps; why there is no simpler shortcut; supervision of the drivers |
| [Development](development.md) | Code structure, building, the test build, tests, continuous integration, releases and how to add a driver or a web signer |
| [History](history.md) | The problems found in testing, version by version, and how each one was solved |

## In one sentence, for each person

- **For users:** install Tessera and your token's driver, run `systemctl --user enable --now p11-kit-server.socket` once (on Debian and Ubuntu, a user service instead: see the [README](../README.md#1-prepare-the-system-once)), and follow the steps for each app in the window. PJeOffice Pro and Assinador SERPRO from FlatPark need nothing.
- **For maintainers of a Flatpak package that uses a token:** give the app `--filesystem=xdg-run/p11-kit/pkcs11` and load the runtime's `p11-kit-client.so`. This is what PJeOffice Pro from FlatPark already does ([Apps](apps.md#for-flatpak-package-maintainers)).
- **For those who need Web PKI or WebSigner (e-SAJ):** in Flatpak browsers, Tessera's web signer extensions, set up per browser with one command the window shows ([Web signers](signers.md)). In native browsers, the vendors' official packages, with `p11-kit-proxy.so` added under the plugin's custom options ([Web signers](signers.md#in-native-browsers)).
- **For those who package SerproID elsewhere:** read the SerproID section in [Drivers](drivers.md#serproid). There are four quirks that make signing fail in ways that are hard to understand.

## Status

What works, on which system and with which token, is in [Compatibility](compatibility.md). What is still to be tested is in the README, under [To do](../README.md#to-do).
