# Tessera documentation

Tessera is a PKCS#11 token hub for Flatpak. It runs the token drivers (SafeSign/GD Burti, SafeNet and SerproID) inside its own sandbox and hands the tokens to the other apps through p11-kit, without installing any vendor package on the system. It was made for immutable systems (Bluefin, Aurora, Bazzite, Fedora Silverblue and Kinoite), where installing a vendor's `.deb` or `.rpm` on the system is hard or impossible.

This documentation records how Tessera works, why it is the way it is, and everything that was discovered while making it work with real tokens.

| Document | What it contains |
|---|---|
| [Architecture](architecture.md) | How the tokens leave Tessera's sandbox and reach the apps; why there is no simpler shortcut; supervision of the drivers |
| [Drivers](drivers.md) | Each driver: origin, verification, "clean" unpacking, dependencies and the quirks found (above all in SerproID) |
| [Apps](apps.md) | PJeOffice Pro, Firefox, Chromium, Chrome, Edge, Brave, Papers and Okular: what each one needs and why |
| [Installation and usage](usage.md) | Preparing the system, installing, the window, the tray, automatic start and the command line |
| [Troubleshooting](troubleshooting.md) | Where the logs are, the test tools and the path for investigating a problem |
| [History](history.md) | The sequence of problems found in the tests of October 28 and 29, 2026 and how each one was solved |
| [Development](development.md) | Code structure, building, tests, continuous integration, releases and how to add a driver |

## In one sentence, for each person

- **For users:** install Tessera, run `systemctl --user enable --now p11-kit-server.socket` once, and follow the two steps for each app in the window. PJeOffice Pro from FlatPark needs nothing.
- **For maintainers of a Flatpak package that uses a token:** give the app `--filesystem=xdg-run/p11-kit/pkcs11` and load the runtime's `p11-kit-client.so`. This is what PJeOffice Pro from FlatPark already does.
- **For those who need Web PKI or WebSigner (e-SAJ):** use [Sigillum](https://github.com/pedrohqb/sigillum), which installs them in the Flatpak browsers and connects them to Tessera's tokens.
- **For those who package SerproID elsewhere:** read the SerproID section in [Drivers](drivers.md#serproid). There are four quirks that make signing fail in ways that are hard to understand.

## Status

Tested on October 29, 2026 on Bluefin, with a GD Burti token (SafeSign, OAB certificate) and a SerproID cloud certificate:

- PJeOffice Pro (FlatPark): signing with SerproID and SafeSign.
- Firefox (Flathub): certificate login (TLS 1.3) with SerproID and SafeSign.
- Papers: PDF signing with both, each certificate listed once.
- SafeSign's Token Administration and the SerproID app opened through Tessera.

- Chromium, Chrome, Edge and Brave: certificate login with SerproID and SafeSign, with the PIN asked once.
- Okular: PDF signing.
