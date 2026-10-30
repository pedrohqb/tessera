# Installation and usage

## Preparing the system (once)

- **`pcscd` with the CCID driver.** Most distributions ship both, and the atomic Fedora and Universal Blue images (Bluefin, Aurora, Bazzite) already come with them. To check: `systemctl status pcscd.socket`.
- **The system's p11-kit server:**
  ```sh
  systemctl --user enable --now p11-kit-server.socket
  ```
  On Fedora-based systems, the unit comes in the `p11-kit-server` package, which the atomic images already ship.

Nothing else is installed on the system.

## Installing from FlatPark

The standard install, with updates through `flatpak update`:

```sh
flatpak --user remote-add --if-not-exists flatpark https://dl.flatpark.org/flatpark.flatpakrepo
flatpak --user install flatpark io.github.pedrohqb.Tessera
```

The FlatPark package (`registry/io.github.pedrohqb.Tessera` in [flatpark/flatpark](https://github.com/flatpark/flatpark)) installs the `tessera-x86_64.tar.gz` of each GitHub release, unmodified, and the same vendor drivers with the same pins as Tessera's own manifest. Its `apply_extra.sh` is this repository's plus the unpacking of that tarball.

## Installing the published release directly

Each release (tag `vX.Y.Z`) publishes on [Releases](https://github.com/pedrohqb/tessera/releases) the `tessera-repo.tar.gz`, a small Flatpak repository with the `install-test-repo.sh` script:

```sh
mkdir -p ~/Downloads/tessera && cd ~/Downloads/tessera
curl -fLO https://github.com/pedrohqb/tessera/releases/latest/download/tessera-repo.tar.gz
tar -xzf tessera-repo.tar.gz
./install-test-repo.sh
```

The GNOME runtime comes from Flathub, which must be configured.

## Installing the test build

Each push to `main` produces, in continuous integration, the `tessera-x86_64` artifact. It contains `tessera-repo.tar.gz`, a small Flatpak repository. A standalone `.flatpak` bundle does not work, because it does not carry the drivers' extra-data: the installation fails with "Extra data missing in detached metadata".

```sh
mkdir -p ~/Downloads/tessera-teste && cd ~/Downloads/tessera-teste
unzip ~/Downloads/tessera-x86_64.zip
tar -xzf tessera-repo.tar.gz
./install-test-repo.sh
```

The script adds the folder as the local repository `tessera-test` and installs or updates Tessera. During installation, Flatpak downloads the vendors' drivers and checks each sha256.

After updating, quit Tessera (menu → **Quit**) and open it again.

## The window

- **Service:** the state of the driver servers, with **Start** or **Restart**. Right below, **System connection**, which checks end to end whether the apps see Tessera's tokens through `p11-kit-server.socket`, with the activation command to copy. Last, **Smart card service (pcscd)**.
- **Readers:** the readers that `pcscd` sees and whether there is a card.
- **Tokens:** each token with its driver and certificates (holder, issuer, validity). An expired certificate is highlighted.
- **Drivers:** each driver with its on/off switch and its state: "Published: the apps can use it", "Starting…", "No answer; trying again (1 of 3)", "Stopped: …" or "Switched off". The buttons for the vendor tools (Token Administration, SerproID) are here.
- **Apps:** the two steps for each app (see [Apps](apps.md)), with the state "Ready", "Step 1 missing", "Step 2 missing" or "Steps 1 and 2 missing". A completed step appears with ✓ and no button, and the states update on their own, without restarting Tessera.
- The refresh button, in the top left corner, runs a check right away. The menu has **Start automatically at login**, **Show an icon in the tray**, **About Tessera** and **Quit** (Ctrl+Q).

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
```

`serve` cannot run together with the window, because both would use the same sockets. Quit Tessera first. The troubleshooting tools are in [Troubleshooting](troubleshooting.md).
