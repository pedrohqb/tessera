# Development

## Repository structure

```
src/
  main.rs       command line; check_drivers (publish, restart, stop) and Health
  ui/
    mod.rs      the app: Hub (the state that outlives the window), actions, Background portal
    window.rs   the window: sidebar and pages (Home, Drivers, Apps, System)
    app_rows.rs the Apps page: each app and its steps
    signer_rows.rs  the window's Web signers section: each Flatpak browser and its commands
    supervise.rs  supervision with or without the window, the tray and the background status
  server.rs     one p11-kit-server per driver (process group, PDEATHSIG, restart)
  remote.rs     `tessera remote`: the process that loads the driver on each connection
  stable.rs     stable copy of the mechanism parameters (the SerproID defect)
  trace.rs      call recorder (TESSERA_REMOTE_TRACE=1)
  status.rs     readers (pcsc), token listing, probe-module, probe-sign
  apps.rs       the Flatpak apps, the tessera-*.module files, overrides
  native.rs     the native apps: where their NSS databases are, their state, Set up
  nss.rs        NSS databases: registering p11-kit-client.so, the Papers database
  drivers.rs    discovery and reading of the driver.toml files
  catalog.rs    the driver extensions not installed, and which one a reader needs
  signers.rs    the web signer extensions: signer.toml, where each is on the host, the browsers' commands
  paths.rs      all the paths (with the development environment variables)
  tray.rs       the tray icon (ksni)
  settings.rs   $XDG_CONFIG_HOME/tessera/settings.toml, in the Flatpak
                ~/.var/app/io.github.pedrohqb.Tessera/config/tessera/settings.toml
  i18n.rs       translation table for Brazilian Portuguese
flatpak/
  io.github.pedrohqb.Tessera.yml   the manifest (permissions, pcsc-lite, OpenSC, extension points)
  fallback-icons.sh                Adwaita icons for desktops whose theme lacks them (Plasma)
  drivers/opensc/                  the built-in driver: driver.toml and OpenSC's configurations
  extensions/<Name>/               each vendor driver's extension: manifest (extra-data),
                                   apply_extra.sh, driver.toml, metainfo, icon; or each web
                                   signer's: the same with start and signer.toml
data/          .desktop, metainfo, icons (normal and symbolic), screenshot;
               app-icons/: the free-software icons of some apps, for when they are not installed
scripts/       pin-drivers.py, install-test-repo.sh, sign-test.py
tests/fake-driver/   fake PKCS#11 drivers: one keeps the mechanism pointer,
                     another (slow-slot-list.c) never answers in time
tools/         spy.c (LD_PRELOAD spy), gui-test/ (the window in Xvfb)
docs/          this documentation
```

## Building

Development, outside Flatpak (needs the development files for GTK 4, libadwaita and pcsc-lite):

```sh
cargo build
cargo clippy -- -D warnings
cargo test
```

The Flatpak package:

```sh
python3 scripts/pin-drivers.py      # checks the extensions' sha256 values and fills in the sizes
flatpak-builder --user --install-deps-from=flathub --force-clean --install \
  --default-branch=stable build-dir flatpak/io.github.pedrohqb.Tessera.yml
# a driver extension, against the app just installed (branch stable, as the
# extensions' runtime-version says):
flatpak-builder --user --force-clean --install --default-branch=stable \
  build-ext flatpak/extensions/SafeSign/io.github.pedrohqb.Tessera.Driver.SafeSign.yml
```

The manifest uses the `org.gnome.Platform` 51 runtime, Rust from the `rust-stable` extension, and builds pcsc-lite 2.5.3 (only the client library, `--libdir=lib`), because the runtime does not have it. The pcsc-lite protocol is compatible with the system's `pcscd`.

## The test build

Each push to `main` produces, in continuous integration, the `tessera-x86_64` artifact. It contains `tessera-repo.tar.gz`, a small Flatpak repository with Tessera and every extension (the drivers and the web signers), and the `install-test-repo.sh` script. A standalone `.flatpak` bundle does not work, because it does not carry the extensions' extra-data: the installation fails with "Extra data missing in detached metadata".

```sh
mkdir -p ~/Downloads/tessera-teste && cd ~/Downloads/tessera-teste
unzip ~/Downloads/tessera-x86_64.zip
tar -xzf tessera-repo.tar.gz
sh install-test-repo.sh SafeSign WebPKI     # Tessera and those extensions; "all" for every one, nothing for Tessera alone
```

The script adds the folder as the local repository `tessera-test`, in the user installation, and installs or updates Tessera and the extensions named. While an extension installs, Flatpak downloads the vendor's files and checks each sha256. The window's install commands name the `flatpark` remote; with the test build, use `tessera-test` instead. After updating, quit Tessera (menu → **Quit**) and open it again.

Each release (tag `vX.Y.Z`) publishes the same `tessera-repo.tar.gz` on [Releases](https://github.com/pedrohqb/tessera/releases), so a release can be installed the same way without FlatPark (`curl -fLO https://github.com/pedrohqb/tessera/releases/latest/download/tessera-repo.tar.gz`), though without updates. The GNOME runtime comes from Flathub, which must be configured in the user installation.

To remove a test build: `flatpak uninstall --user io.github.pedrohqb.Tessera` (its extensions go with it), `flatpak remote-delete --user tessera-test`, and the folder.

## Development environment variables

| Variable | Default | Use |
|---|---|---|
| `TESSERA_DRIVERS_PATH` | `/app/lib/tessera/drivers:/app/lib/tessera/extensions` | where to look for `driver.toml` |
| `TESSERA_SIGNERS_PATH` | `/app/lib/tessera/signers` | where to look for the web signers' `signer.toml` (`flatpak/extensions` works, to see the section outside Flatpak) |
| `TESSERA_SOCKET_DIR` | `$XDG_RUNTIME_DIR/tessera` | the servers' sockets |
| `TESSERA_HOST_MODULES` | `~/.config/pkcs11/modules` | where to write `tessera-*.module` |
| `TESSERA_P11KIT_SERVER` | the first that exists of `/usr/libexec/p11-kit/p11-kit-server` and similar | the p11-kit helper |
| `TESSERA_OS_RELEASE` | `/run/host/os-release`, then `/etc/os-release` | the host's distribution, which decides p11-kit's path for PJeOffice and whether the system's NSS already loads p11-kit; for the tests |
| `TESSERA_CLIENT_MODULE` | `/usr/lib/x86_64-linux-gnu/pkcs11/p11-kit-client.so` | the client module that `list-tokens` and `status` load (registration in the NSS databases always uses the default path) |
| `TESSERA_REMOTE_TRACE` | — | `1`: record the calls to the driver |
| `TESSERA_REMOTE_LOAD` | `managed` | `raw`: serve the driver's own table |
| `TESSERA_PIN`, `TESSERA_MECH` | — | `probe-sign`: PIN and mechanism filter |

To test outside Flatpak, use a regular user: as root, p11-kit ignores the user configuration (`~/.config/pkcs11/modules`).

## Tests

`cargo test` runs all the automated tests; CI runs them on every push with `TESSERA_TEST_REQUIRE=1`, so a missing tool fails the run instead of skipping a test.

- **Unit tests:** the native apps' states on a made-up home folder, including the Fedora case and PJeOffice's `list.a3` (`native.rs`), driver descriptions (`drivers.rs`), the Firefox profile lock and registering/removing the client module in an NSS database (`nss.rs`, needs `libnss3.so`), and the translations: every string passed to `t(…)` has a Portuguese entry, and none has two (`i18n.rs`).
- **End to end (`tests/drivers.rs`):** `tessera serve` with test drivers, each connection served by `tessera remote`, and `tessera probe-sign` signing through `p11-kit-client.so`, as the apps do:
  - a SoftHSM token with an RSA key: published to the host modules folder, signs with PSS and PKCS#1 v1.5 with its PIN, and refuses a wrong PIN;
  - `tests/fake-driver/keeps-mechanism-pointer.c`, which behaves like SerproID: signs PSS only because `stable.rs` hands it parameters that are still valid in `C_Sign` (without that copy the test fails, with `hash=<garbage> -> WRONG` in the driver's log);
  - `tests/fake-driver/slow-slot-list.c`: never published, and its first timeout reported.

  They need softhsm2, openssl, a C compiler, p11-kit's server helper, client module and headers (on Ubuntu: `softhsm2 p11-kit p11-kit-modules libp11-kit-dev libnss3 openssl gcc`). Without them, a test is skipped with a message.
- **By hand, the full path with SoftHSM:** start a `p11-kit-server` with `P11_KIT_PRIVATEDIR` pointing to a `p11-kit-remote` that runs `target/debug/tessera remote "$@"`, and sign with `scripts/sign-test.py` through `p11-kit-client.so` and `P11_KIT_SERVER_ADDRESS`. With the environment variable `PIN=1234`, the script logs in to SoftHSM.
- **By hand, the SerproID defect:** `gcc -shared -fPIC -o libfake.so tests/fake-driver/keeps-mechanism-pointer.c` and use it as `--provider`. Through the original `p11-kit-remote`, the server log shows `fake: C_Sign sees … hash=<garbage> -> WRONG`; through `tessera remote`, `hash=0x250 … -> OK`.
- **On each distribution (CI only):** `cargo test --bin tessera -- --ignored detection_matches_this_system`, in containers of Fedora, Debian, Ubuntu, Arch and openSUSE with their default packages: the p11-kit path Tessera writes for PJeOffice exists; a fresh NSS database, made with `certutil`, is called ready by Tessera exactly when NSS (`modutil -list`) loads a p11-kit module; which Tessera, inside a Flatpak, can only deduce from the distribution's `os-release`. Its findings go into an annotation per distribution. This is how the Fedora case was found (a fresh database there lists nothing, yet NSS loads p11-kit-proxy through crypto-policies), and that openSUSE ships the same crypto-policies file but its NSS does not load it.
- **The window, automatically (`tests/gui/test_window.py`):** opens Tessera on a made-up home folder and drives it through the accessibility tree (AT-SPI), as a screen reader would: the Native tab's Set up for Firefox (p11-kit-proxy.so in the profile, row Ready), Okular and Papers (no button, "Set up Firefox first", then Ready with Firefox), PJeOffice Pro (`list.a3` with this distribution's path, the other settings kept), and the Flatpak tab's step 2 for Firefox (p11-kit-client.so in the Flatpak's profile). An expander row offers no AT-SPI action, so the test reaches it with Tab and opens it with Space (`xdotool`). Run it with `xvfb-run -a dbus-run-session -- python3 tests/gui/test_window.py` after `cargo build`; it needs `at-spi2-core python3-pyatspi xvfb xdotool libnss3-tools`. `TESSERA_GUI_DUMP=1` prints the accessibility tree on a failure, `TESSERA_GUI_KEEP=1` keeps the home folder and Tessera's log.
- **The window, by hand:** `tools/gui-test/run.sh` opens Tessera in an Xvfb with its own D-Bus session, a test SoftHSM driver and a fake tray (`fake-watcher.py`, which records the icon and the menu). From there, with `. /tmp/tessera-gui/env.sh`, `xdotool` clicks and `import -window root` takes the screenshot. This is how the buttons, Restart, the switches, closing and reopening, the tray and Quit were tested. The driver `tests/fake-driver/slow-slot-list.c` (a `C_GetSlotList` that takes 12 s), compiled and placed in a folder with a `driver.toml` in `TESSERA_DRIVERS_PATH`, tests the three attempts.

## Continuous integration

`.github/workflows/ci.yml`, on every push to `main`, on every pull request, on every `v*` tag and when triggered by hand:

1. **Detection on each distribution:** the containers above.
2. **Build and test:** `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test`, then the window test (`tests/gui/test_window.py` in an Xvfb, with the end of its output in an annotation if it fails). Clippy also checks that every `unsafe` block and `unsafe fn` carries its `// SAFETY:` or `# Safety` note (the `[lints.clippy]` table in `Cargo.toml`).
3. **Flatpak bundle:** `pin-drivers.py`, `flatpak-builder` (with the last lines of the log in an annotation, if it fails), then each extension (drivers and web signers) built, installed from the build's repository, so its `apply_extra.sh` runs on the real download, and its files checked, `flatpak build-update-repo`, and the artifacts `tessera-repo.tar.gz` (test repository with `install-test-repo.sh`) and `tessera-x86_64.tar.gz` (the whole `/app` tree, for anyone who wants to package it as extra-data).
4. **With a `vX.Y.Z` tag:** creates the release on GitHub with both files.

## Publishing a release

1. Update the version in `Cargo.toml` and add the `<release>` entry in the metainfo.
2. `git tag v0.1.0 && git push origin v0.1.0`.
3. Continuous integration publishes the release at https://github.com/pedrohqb/tessera/releases.

## Adding a driver

A driver is a folder with a `driver.toml` (the format is in [Drivers](drivers.md)). OpenSC, built in, lives in `flatpak/drivers/opensc`. Every other driver is a Flatpak extension `io.github.pedrohqb.Tessera.Driver.<Name>`, mounted at `/app/lib/tessera/extensions/<Name>/`, without changing Tessera: the vendors' ones are in `flatpak/extensions/`, and anyone can publish one the same way (FlatPark's [extension guide](https://github.com/flatpark/flatpark/blob/main/docs/packagers/extensions.md)). To have the window suggest it for its tokens, add it to `src/catalog.rs`. For a new driver, it is worth checking:

- whether it loads on its own (`probe-module`) and through p11-kit (`list-tokens` with a server at hand);
- whether it signs directly and through p11-kit, with `sign-test.py` (valid PSS, and not PKCS#1 v1.5);
- whether it needs any file or environment variable (`[env]`, `create_dirs`, `library_path`);
- how it behaves without a token: if it hangs, supervision stops it after three attempts.

## Adding a web signer

A web signer is an extension `io.github.pedrohqb.Tessera.Signer.<Name>` with a `signer.toml` and a start script (see [Web signers](signers.md) and `flatpak/extensions/WebPKI`). The program must run in the browsers' runtimes (the freedesktop one), find its files relative to its start script, and load its PKCS#11 module by a name `lib/` can hold. To have the window offer it when it is not installed, add it to `KNOWN` in `src/signers.rs`. CI builds and installs every `flatpak/extensions/*/io.github.pedrohqb.Tessera.*.yml`.

## To do

The list is in the README, under [To do](../README.md#to-do).
