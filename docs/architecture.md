# Architecture

## The path from a token to the app

```
USB token ──► pcscd (system)
                 │  --socket=pcsc
                 ▼
┌──────────────── Tessera's sandbox ─────────────────┐
│ one p11-kit server per driver:                     │
│   $XDG_RUNTIME_DIR/tessera/safesign  ◄─ libaetpkss │
│   $XDG_RUNTIME_DIR/tessera/safenet   ◄─ libeToken  │
│   $XDG_RUNTIME_DIR/tessera/serproid  ◄─ libserproidp11 ──► Serpro cloud
│ each connection is handled by `tessera remote`     │
└────────────────────────────────────────────────────┘
                 ▲  "remote: unix:path=…" in
                 │  ~/.config/pkcs11/modules/tessera-<driver>.module
┌──────────────── system ────────────────────────────┐
│ p11-kit-server.socket ($XDG_RUNTIME_DIR/p11-kit/pkcs11)
│   serves: system trust + system modules            │
│           (OpenSC, if any) + Tessera's drivers     │
└────────────────────────────────────────────────────┘
                 ▲  --filesystem=xdg-run/p11-kit/pkcs11
                 │  + the runtime's p11-kit-client.so
   PJeOffice Pro · Firefox · Chromium · Chrome · Edge · Brave · Papers · Okular
```

1. Tessera starts one p11-kit server for each enabled driver, each with its own socket in `$XDG_RUNTIME_DIR/tessera/<driver>`.
2. For each driver that answers, it writes the file `~/.config/pkcs11/modules/tessera-<driver>.module` on the system, with a single meaningful line: `remote: unix:path=/run/user/<uid>/tessera/<driver>`. This way the system's p11-kit starts including these tokens among its modules.
3. The system's own p11-kit server (`p11-kit-server.socket`, activated once by the user) serves all of this at `$XDG_RUNTIME_DIR/p11-kit/pkcs11`: the system trust, the system modules and Tessera's drivers.
4. A Flatpak app that is given `--filesystem=xdg-run/p11-kit/pkcs11` finds that socket in the same place and uses it through the runtime's `p11-kit-client.so`. Native apps see it through `p11-kit-proxy.so`.

No vendor package is installed on the system. The only things Tessera writes outside its own sandbox are the `tessera-*.module` files and, in the apps, the module's registration in the NSS database (see [Apps](apps.md)).

## Why not a simpler shortcut

The path has one extra hop (the system server) because two Flatpak rules make the direct path impossible:

- **Flatpak turns off p11-kit's user configuration in every sandbox.** It writes `/etc/pkcs11/pkcs11.conf` with `user-config: none`. So no process inside a sandbox can load a p11-kit configuration of its own (for example, a list of Tessera's modules). That is why each driver has its own server, which needs no configuration at all (`--provider <driver>`), and joining the drivers together is left to the system's p11-kit, which reads `~/.config/pkcs11/modules`.
- **Flatpak occupies `$XDG_RUNTIME_DIR/p11-kit` with its own trust server** (from `flatpak-session-helper`) and removes `P11_KIT_SERVER_ADDRESS` from the environment. That is why Tessera cannot serve at the default address. The attempt to mount `xdg-run/p11-kit` inside Tessera's sandbox failed with `bwrap: Can't mount on symlink destination /run/user/1000/p11-kit`. The sockets live in `xdg-run/tessera`.

The `xdg-run/p11-kit/pkcs11` permission given to an app replaces, for that app only, Flatpak's trust server with the system server. Since the system server also serves the system trust, the app does not lose the root certificates.

## The process that loads the driver

`p11-kit-server` creates one process per connection and, in that process, runs the `p11-kit-remote` executable from the `P11_KIT_PRIVATEDIR` directory. Tessera points that variable to `$XDG_DATA_HOME/tessera/libexec`, where it writes a `p11-kit-remote` that only runs `tessera remote "$@"`. This way, the one that loads the driver is Tessera itself (`src/remote.rs`). Before loading the driver, `tessera remote`:

- resets all signals to their default behavior and unblocks the mask (`p11-kit-server` ignores some, and ignored signals carry across `exec`);
- moves the p11-kit channel off standard input and output, which become `/dev/null` and the log, so that a driver that writes to standard output does not corrupt the protocol;
- wraps the driver's function table to hand it stable copies of the mechanism parameters (see [SerproID](drivers.md#serproid)). This is the fix that made SerproID work through p11-kit;
- with `TESSERA_REMOTE_TRACE=1`, records each call in the log (see [Troubleshooting](troubleshooting.md)).

Each driver's environment comes from the `[env]` of its `driver.toml` (for example, SerproID's `LD_PRELOAD`), and `LD_LIBRARY_PATH` comes from `library_path`.

## Supervision of the drivers

Supervision runs in the service, with or without the window (`supervise` in `src/ui.rs`, `check_drivers` in `src/main.rs`):

- Every 4 s with the window open, every 16 s with it closed (and every 10 s in `tessera serve`), Tessera lists the tokens of each running driver. It does this in a separate process (`tessera list-tokens`), with a 10 s limit, so that a hung driver does not hang the window.
- The drivers that answer are published on the system. Those that do not answer within 10 s stop being published and are restarted; those that answer with an error only stop being published. After 3 times in a row without an answer, the driver is stopped, and the reason appears in its row in the window. A hung driver published on the system would hang every app that loads p11-kit. That is what happened with SafeNet without a token in the first test in Firefox.
- Each (re)start of the service (opening, Start/Restart, switching a driver on or off, Quit) increments a "generation". A listing that started before is discarded and redone right away. Before this rule, the stale result switched off the driver that had just come back.
- **Quit** stops the servers and deletes the `tessera-*.module` files, so the apps do not wait for servers that do not exist.

## Window, tray and background

- The service is an `adw::Application` with a "hold": it keeps running when the window closes.
- While the service runs, with the window open or closed, there is a tray icon (it can be removed from the menu, under **Show an icon in the tray**) (StatusNotifierItem, through the `ksni` library) with the summary ("Serving · 2/2 driver(s) · 2 published · 2 token(s)"), **Open Tessera** and **Quit (stop serving the tokens)**. GNOME shows this icon with the AppIndicator extension, which Bluefin, Aurora and Ubuntu already ship; KDE shows it without an extension. The same summary goes to the Background portal, which GNOME shows in the list of background apps.
- `tessera --background` starts only the service. It is the command the Background portal uses when automatic start is on.

## Flatpak permissions

| Permission | What for |
|---|---|
| `--share=ipc`, `--device=dri` | The window (X11 shared memory, graphics acceleration) |
| `--socket=pcsc` | Talking to the system's `pcscd`, which owns the USB readers |
| `--share=network` | SerproID (cloud) and revocation checks by the vendor tools |
| `--socket=x11`, `--socket=wayland` | The window; full X11 because SerproID (JavaFX) and Token Administration only draw through X11 |
| `--filesystem=xdg-run/tessera:create` | The sockets of the driver servers |
| `--filesystem=xdg-config/pkcs11:create` | Writing the `tessera-*.module` files; no other file is touched |
| `--filesystem=xdg-run/p11-kit/pkcs11` | Checking whether the apps see Tessera's tokens through the system server (see [Apps](apps.md)) |
| `--filesystem=xdg-data/flatpak/overrides:ro` | Showing whether step 1 of each app has already been done |
| `--filesystem=xdg-data/flatpak/exports/share/icons:ro`, `xdg-data/flatpak/app:ro` and the same under `/var/lib/flatpak` | Showing each app with its own icon. The exported icons are links to each app's folder in `app/`, so that folder is needed too, read-only |
| `--filesystem=~/.var/app/org.gnome.Papers/config/mozilla:create`, `~/.var/app/org.kde.okular/config/mozilla:create` | The own NSS database of Papers and of Okular |
| `--filesystem=~/.var/app/org.mozilla.firefox/config/mozilla/firefox` and `.../.mozilla/firefox` | Registering the module in the Firefox profiles |
| `--filesystem=~/.pki/nssdb` and, for Chromium, Brave, Chrome and Edge, `~/.var/app/<browser>/data/pki/nssdb` and `~/.var/app/<browser>/.pki/nssdb` | Registering the module in the NSS database of the Chromium-based browsers (the new one, since Chromium 146, and the old one) |
| `--talk-name=org.kde.StatusNotifierWatcher` | The tray icon |
| `--persist=.config/serproid` | SerproID's certificate folder and log stay in Tessera's own data |

The browser folders do not get `:create`. The browser creates its own database the first time it runs, and Tessera does not create folders for browsers that are not installed.
