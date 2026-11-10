# Usage

Installing Tessera, preparing the system and installing a driver or a web signer are in the README ([Installation](../README.md#installation)), with the commands for each distribution. This page covers what comes after: the window, the tray, automatic start and the command line. A few notes the README leaves out:

- **Where the packages come from.** FlatPark's package (`registry/io.github.pedrohqb.Tessera` in [flatpark/flatpark](https://github.com/flatpark/flatpark)) installs the `tessera-x86_64.tar.gz` of each GitHub release, unmodified. Each extension, driver or web signer, has its own entry there, built from `flatpak/extensions/<Name>` with the same pins. The GNOME runtime comes from Flathub, and Flatpak only downloads it from a remote in the same installation as Tessera (`--user` or `--system`), which is why the README adds both remotes.
- **Why the Debian family needs a user service.** Ubuntu 26.04 and the rest of the Debian family ship the server (`p11-kit server`, package `p11-kit`) but no systemd units for it, and build it without socket activation (no libsystemd), so `p11-kit-server.socket` does not exist there. The README's user service starts it at login and creates the socket itself, at the same path (`%t/p11-kit/pkcs11`); the window's copy button gives the same as one line. Only p11-kit 0.25.6 or newer is supported: Debian 12 and 13, LMDE 6 and 7, Ubuntu 24.04 and Linux Mint 22 have older versions, with which signing fails ([History](history.md), item 38). On Fedora-based systems and openSUSE the socket unit comes in the `p11-kit-server` package (the atomic images already ship it); on Arch, in `p11-kit` itself.
- **Without systemd,** `mkdir -p -m 700 "$XDG_RUNTIME_DIR/p11-kit" && p11-kit server -f -n "$XDG_RUNTIME_DIR/p11-kit/pkcs11" pkcs11:` serves the tokens while the terminal stays open (see the [FAQ](../FAQ.md#does-it-work-with-snap-without-systemd)).
- **Test builds** and installing a release without FlatPark are in [Development](development.md#the-test-build).

## The window

- **Service:** the state of the driver servers, with **Start** or **Restart**. Right below, **System connection**, which checks end to end whether the apps see Tessera's tokens through the host's p11-kit server, with the activation command to copy (on Debian and Ubuntu, the one that writes the user service, too long to show). Then **Smart card service (pcscd)**, and the switch **Switch off the system's OpenSC**, on by default ([Drivers](drivers.md#opensc)).
- **Readers:** the readers that `pcscd` sees and whether there is a card.
- **Tokens:** each token with its driver and certificates (holder, issuer, validity). An expired certificate is highlighted.
- **Drivers:** each driver with its on/off switch and its state: "Published: the apps can use it", "Starting…", "No answer; trying again (1 of 3)", "Stopped: …" or "Switched off". The buttons for the vendor tools (Token Administration, SerproID) are here.
- **Apps:** two tabs. **Flatpak:** the two steps for each app (see [Apps](apps.md)), with the state "Ready", "Step 1 missing", "Step 2 missing" or "Steps 1 and 2 missing". **Native:** the apps installed from the distribution (Firefox, the Chromium-based browsers, Okular and Papers, PJeOffice Pro), each with its one step, **Set up** (Okular and Papers follow Firefox) (see [Native apps](apps.md#native-apps)). A completed step appears with ✓ and no button, and the states update on their own, without restarting Tessera.
- **Web signers:** for each Flatpak browser, the web signer extensions installed and whether each is set up in it, with the one command to copy; then the ones not installed, with their FlatPark page and install command (see [Web signers](signers.md)).
- The refresh button, in the top left corner, runs a check right away. The menu has **Start automatically at login**, **Show an icon in the tray**, **Restart Tessera** (needed after installing an extension, or when an app's folder appeared after Tessera started), **About Tessera** and **Quit** (Ctrl+Q).

Configuration errors appear in a dialog box, with the full text and what to do.

## Closing the window, the tray and the background

Closing the window does **not** stop the service: the tokens stay available. While the service runs, with the window open or closed, there is a tray icon (it can be removed from the menu, under **Show an icon in the tray**) with the summary, **Open Tessera** and **Quit (stop serving the tokens)**. A click on the icon opens the window. On GNOME, the tray depends on the AppIndicator extension, which Bluefin already ships.

**Quit**, from the menu or from the tray, stops the servers and removes the drivers from the system.

**Start automatically at login** asks the system, through the Background portal, to open Tessera in the background (`tessera --background`) at login: no window, only the service and the tray.

## Command line

```
flatpak run io.github.pedrohqb.Tessera                 the window (and the service)
flatpak run io.github.pedrohqb.Tessera --background    only the service
flatpak run --command=tessera io.github.pedrohqb.Tessera serve
                                                       only the service, in the terminal, with the messages
flatpak run --command=tessera io.github.pedrohqb.Tessera status
                                                       drivers, readers, tokens and certificates
flatpak run --command=tessera io.github.pedrohqb.Tessera nss-setup DIR
                                                       registers p11-kit-client.so in the NSS database DIR
flatpak run --command=tessera io.github.pedrohqb.Tessera opensc TOOL [ARGS]
                                                       one of OpenSC's tools (pkcs15-tool --change-pin, …),
                                                       with the configuration of Tessera's OpenSC driver
```

`serve` cannot run together with the window, because both would use the same sockets. Quit Tessera first. The troubleshooting tools are in [Troubleshooting](troubleshooting.md).
