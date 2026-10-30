# Development

## Repository structure

```
src/
  main.rs       command line; check_drivers (publish, restart, stop) and Health
  ui.rs         the window, supervision without the window, the tray and the Background portal
  server.rs     one p11-kit-server per driver (process group, PDEATHSIG, restart)
  remote.rs     `tessera remote`: the process that loads the driver on each connection
  stable.rs     stable copy of the mechanism parameters (the SerproID defect)
  trace.rs      call recorder (TESSERA_REMOTE_TRACE=1)
  status.rs     readers (pcsc), token listing, probe-module, probe-sign
  apps.rs       the apps, the tessera-*.module files, overrides
  nss.rs        NSS databases: registering p11-kit-client.so, the Papers database
  drivers.rs    discovery and reading of the driver.toml files
  paths.rs      all the paths (with the development environment variables)
  tray.rs       the tray icon (ksni)
  settings.rs   $XDG_CONFIG_HOME/tessera/settings.toml, in the Flatpak
                ~/.var/app/io.github.pedrohqb.Tessera/config/tessera/settings.toml
  i18n.rs       translation table for Brazilian Portuguese
flatpak/
  io.github.pedrohqb.Tessera.yml   the manifest (permissions, pcsc-lite, extra-data)
  apply_extra.sh                   unpacks the drivers at installation
  drivers/<driver>/driver.toml     the description of each driver
data/          .desktop, metainfo, icons (normal and symbolic), screenshot
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
python3 scripts/pin-drivers.py      # checks the sha256 values and fills in the sizes
flatpak-builder --user --install-deps-from=flathub --force-clean --install \
  build-dir flatpak/io.github.pedrohqb.Tessera.yml
```

The manifest uses the `org.gnome.Platform` 51 runtime, Rust from the `rust-stable` extension, and builds pcsc-lite 2.5.3 (only the client library, `--libdir=lib`), because the runtime does not have it. The pcsc-lite protocol is compatible with the system's `pcscd`.

## Development environment variables

| Variable | Default | Use |
|---|---|---|
| `TESSERA_DRIVERS_PATH` | `/app/lib/tessera/drivers:/app/lib/tessera/extensions` | where to look for `driver.toml` |
| `TESSERA_SOCKET_DIR` | `$XDG_RUNTIME_DIR/tessera` | the servers' sockets |
| `TESSERA_HOST_MODULES` | `~/.config/pkcs11/modules` | where to write `tessera-*.module` |
| `TESSERA_P11KIT_SERVER` | the first that exists of `/usr/libexec/p11-kit/p11-kit-server` and similar | the p11-kit helper |
| `TESSERA_CLIENT_MODULE` | `/usr/lib/x86_64-linux-gnu/pkcs11/p11-kit-client.so` | the client module that `list-tokens` and `status` load (registration in the NSS databases always uses the default path) |
| `TESSERA_REMOTE_TRACE` | — | `1`: record the calls to the driver |
| `TESSERA_REMOTE_LOAD` | `managed` | `raw`: serve the driver's own table |
| `TESSERA_PIN`, `TESSERA_MECH` | — | `probe-sign`: PIN and mechanism filter |

To test outside Flatpak, use a regular user: as root, p11-kit ignores the user configuration (`~/.config/pkcs11/modules`).

## Tests

- **Unit:** `cargo test`.
- **The full path with SoftHSM:** start a `p11-kit-server` with `P11_KIT_PRIVATEDIR` pointing to a `p11-kit-remote` that runs `target/debug/tessera remote "$@"`, and sign with `scripts/sign-test.py` through `p11-kit-client.so` and `P11_KIT_SERVER_ADDRESS`. With the environment variable `PIN=1234`, the script logs in to SoftHSM.
- **The SerproID defect:** `gcc -shared -fPIC -o libfake.so tests/fake-driver/keeps-mechanism-pointer.c` and use it as `--provider`. Through the original `p11-kit-remote`, the server log shows `fake: C_Sign sees … hash=<garbage> -> WRONG`; through `tessera remote`, `hash=0x250 … -> OK`.
- **The window:** `tools/gui-test/run.sh` opens Tessera in an Xvfb with its own D-Bus session, a test SoftHSM driver and a fake tray (`fake-watcher.py`, which records the icon and the menu). From there, with `. /tmp/tessera-gui/env.sh`, `xdotool` clicks and `import -window root` takes the screenshot. This is how the buttons, Restart, the switches, closing and reopening, the tray and Quit were tested. The driver `tests/fake-driver/slow-slot-list.c` (a `C_GetSlotList` that takes 12 s), compiled and placed in a folder with a `driver.toml` in `TESSERA_DRIVERS_PATH`, tests the three attempts.

## Continuous integration

`.github/workflows/ci.yml`, on every push to `main`, on every pull request, on every `v*` tag and when triggered by hand:

1. **Build and test:** `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test`.
2. **Flatpak bundle:** `pin-drivers.py`, `flatpak-builder` (with the last lines of the log in an annotation, if it fails), `flatpak build-update-repo`, and the artifacts `tessera-repo.tar.gz` (test repository with `install-test-repo.sh`) and `tessera-x86_64.tar.gz` (the whole `/app` tree, for anyone who wants to package it as extra-data).
3. **With a `vX.Y.Z` tag:** creates the release on GitHub with both files.

## Publishing a release

1. Update the version in `Cargo.toml` and add the `<release>` entry in the metainfo.
2. `git tag v0.1.0 && git push origin v0.1.0`.
3. Continuous integration publishes the release at https://github.com/pedrohqb/tessera/releases.

## Adding a driver

A driver is a folder with a `driver.toml` (the format is in [Drivers](drivers.md)). The ones that come with Tessera live in `flatpak/drivers/` and in `apply_extra.sh`. A third-party driver can come as a Flatpak extension `io.github.pedrohqb.Tessera.Driver.<Name>`, mounted at `/app/lib/tessera/extensions/<Name>/`, without changing Tessera. For a new driver, it is worth checking:

- whether it loads on its own (`probe-module`) and through p11-kit (`list-tokens` with a server at hand);
- whether it signs directly and through p11-kit, with `sign-test.py` (valid PSS, and not PKCS#1 v1.5);
- whether it needs any file or environment variable (`[env]`, `create_dirs`, `library_path`);
- how it behaves without a token: if it hangs, supervision stops it after three attempts.

## Toward Flathub

Still to do: publishing on Flathub, which requires a review of the permissions (the folders of other apps and the tray's `talk-name` need justification) and a current screenshot; and testing SafeNet and the Chromium browsers with real tokens.
