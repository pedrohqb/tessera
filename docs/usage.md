# Installation and usage

## Preparing the system (once)

- **`pcscd` with the CCID driver.** Most distributions ship both, and the atomic Fedora and Universal Blue images (Bluefin, Aurora, Bazzite) already come with them. To check: `systemctl status pcscd.socket`.
- **The system's p11-kit server:**
  ```sh
  systemctl --user enable --now p11-kit-server.socket
  ```
  On Fedora-based systems, the unit comes in the `p11-kit-server` package, which the atomic images already ship; on openSUSE, in `p11-kit-server` too; on Arch, in `p11-kit` itself.

  **Ubuntu 26.04 and the rest of the Debian family** ship the server (`p11-kit server`, package `p11-kit`) but no systemd units for it, and build it without socket activation (no libsystemd), so the unit above does not exist there (and from p11-kit 0.25.6 on the server could take the socket, but the unit is still not shipped). Only systems with p11-kit 0.25.6 or newer are supported: Debian 12 and 13, LMDE 6 and 7, Ubuntu 24.04 and Linux Mint 22 have older versions, with which signing fails ([History](history.md) item 38). A plain user service takes its place: it starts at login and creates the socket itself, at the same path (`%t/p11-kit/pkcs11`). The README has it as a file; the window's copy button gives the same as one line. Its content:
  ```ini
  [Unit]
  Description=p11-kit server
  Documentation=man:p11-kit(8)

  [Service]
  ExecStartPre=/bin/mkdir -p -m 700 %t/p11-kit
  ExecStart=/usr/bin/p11-kit server -f -n %t/p11-kit/pkcs11 pkcs11:
  Restart=on-failure

  [Install]
  WantedBy=default.target
  ```
  in `~/.config/systemd/user/p11-kit-server.service`, then `systemctl --user daemon-reload && systemctl --user enable --now p11-kit-server.service`. To try it without systemd, `mkdir -p -m 700 "$XDG_RUNTIME_DIR/p11-kit" && p11-kit server -f -n "$XDG_RUNTIME_DIR/p11-kit/pkcs11" pkcs11:` serves the tokens while the terminal stays open.

Nothing else is installed on the system.

## Installing from FlatPark

The standard install, with updates through `flatpak update`. The GNOME runtime comes from Flathub, and Flatpak only downloads it from a remote in the same installation as Tessera (`--user` here; for a system install, `--system` in all three lines):

```sh
flatpak --user remote-add --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo
flatpak --user remote-add --if-not-exists flatpark https://dl.flatpark.org/flatpark.flatpakrepo
flatpak --user install flatpark io.github.pedrohqb.Tessera
```

Then the driver for the token, if it is not one OpenSC (built in) serves; the window shows which one and the command:

```sh
flatpak --user install flatpark io.github.pedrohqb.Tessera.Driver.SafeSign   # GD Burti and other SafeSign tokens
flatpak --user install flatpark io.github.pedrohqb.Tessera.Driver.SafeNet    # SafeNet eToken 5100/5110, IDPrime
flatpak --user install flatpark io.github.pedrohqb.Tessera.Driver.SerproID   # Serpro's cloud certificate
```

The FlatPark package (`registry/io.github.pedrohqb.Tessera` in [flatpark/flatpark](https://github.com/flatpark/flatpark)) installs the `tessera-x86_64.tar.gz` of each GitHub release, unmodified. Each driver extension has its own entry there, built from `flatpak/extensions/<Name>`, with the same pins.

## Installing the published release directly

Each release (tag `vX.Y.Z`) publishes on [Releases](https://github.com/pedrohqb/tessera/releases) the `tessera-repo.tar.gz`, a small Flatpak repository with the `install-test-repo.sh` script:

```sh
mkdir -p ~/Downloads/tessera && cd ~/Downloads/tessera
curl -fLO https://github.com/pedrohqb/tessera/releases/latest/download/tessera-repo.tar.gz
tar -xzf tessera-repo.tar.gz
./install-test-repo.sh SafeSign      # or: SafeNet, SerproID, all, or none for Tessera alone
```

The GNOME runtime comes from Flathub, which must be configured.

## Installing the test build

Each push to `main` produces, in continuous integration, the `tessera-x86_64` artifact. It contains `tessera-repo.tar.gz`, a small Flatpak repository. A standalone `.flatpak` bundle does not work, because it does not carry the driver extensions' extra-data: the installation fails with "Extra data missing in detached metadata". The repository holds Tessera and the three driver extensions; `install-test-repo.sh` installs Tessera and the drivers named after it (`all` for every one).

```sh
mkdir -p ~/Downloads/tessera-teste && cd ~/Downloads/tessera-teste
unzip ~/Downloads/tessera-x86_64.zip
tar -xzf tessera-repo.tar.gz
./install-test-repo.sh all
```

The script adds the folder as the local repository `tessera-test` and installs or updates Tessera and the drivers asked for. While a driver installs, Flatpak downloads the vendor's files and checks each sha256. The window's install commands name the `flatpark` remote; with the test build, use `tessera-test` instead.

After updating, quit Tessera (menu → **Quit**) and open it again.

## The window

- **Service:** the state of the driver servers, with **Start** or **Restart**. Right below, **System connection**, which checks end to end whether the apps see Tessera's tokens through the host's p11-kit server, with the activation command to copy (on Debian and Ubuntu, the one that writes the user service, too long to show). Last, **Smart card service (pcscd)**.
- **Readers:** the readers that `pcscd` sees and whether there is a card.
- **Tokens:** each token with its driver and certificates (holder, issuer, validity). An expired certificate is highlighted.
- **Drivers:** each driver with its on/off switch and its state: "Published: the apps can use it", "Starting…", "No answer; trying again (1 of 3)", "Stopped: …" or "Switched off". The buttons for the vendor tools (Token Administration, SerproID) are here.
- **Apps:** two tabs. **Flatpak:** the two steps for each app (see [Apps](apps.md)), with the state "Ready", "Step 1 missing", "Step 2 missing" or "Steps 1 and 2 missing". **Native:** the apps installed from the distribution (Firefox, the Chromium-based browsers, Okular and Papers, PJeOffice Pro), each with its one step, **Set up** (Okular and Papers follow Firefox) (see [Native apps](apps.md#native-apps)). A completed step appears with ✓ and no button, and the states update on their own, without restarting Tessera.
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
flatpak run --command=tessera io.github.pedrohqb.Tessera opensc TOOL [ARGS]
                                                       one of OpenSC's tools (pkcs15-tool --change-pin, …),
                                                       with the configuration of Tessera's OpenSC driver
```

`serve` cannot run together with the window, because both would use the same sockets. Quit Tessera first. The troubleshooting tools are in [Troubleshooting](troubleshooting.md).
