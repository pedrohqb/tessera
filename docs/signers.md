# Web signers

Some signing sites need a browser plugin with a program of its own behind it: **Lacuna Web PKI**, **Softplan WebSigner** (e-SAJ) and **Certisign WebSigner** (Certisign's signing portals). Each is published as a Flatpak extension of Tessera, so the Flatpak browsers can use them with Tessera's tokens, without a container and without packages installed on the system. They replace [Sigillum](https://github.com/pedrohqb/sigillum), now archived, which did the same with a script (to move from it, run `./sigillum.sh remover` first, then each browser's command), and the web signers of [distrobox-adv-br](https://github.com/pedrohqb/distrobox-adv-br).

| Extension | Plugin | Vendor file (sha256) |
|---|---|---|
| `io.github.pedrohqb.Tessera.Signer.WebPKI` | Lacuna Web PKI 2.17.0 | `setup-deb-64` (`cf284641…c2f4`) |
| `io.github.pedrohqb.Tessera.Signer.SoftplanWebSigner` | Softplan WebSigner 2.15.0 | `webpki-chrome-64-deb` (`04fa41e9…8289`) |
| `io.github.pedrohqb.Tessera.Signer.CertisignWebSigner` | Certisign WebSigner, program 2.9.5 (download path 2.17.7) | `setup-deb-64` (`04981f07…abfb`) |

Softplan's and Certisign's sha256 values are the same as in distrobox-adv-br (and Softplan's as in Sigillum). Web PKI's is 2.17.0's, released by Lacuna on November 9, 2026, the day after the extension was first pinned at distrobox-adv-br's 2.16.1.

## How it works

A plugin has two parts: the browser extension, which talks to the site, and a program, which the extension starts by **native messaging**. The browser runs the program named in a small JSON manifest and talks to it over its standard input and output. The program loads the PKCS#11 module, shows the dialogs (certificate, PIN) and signs.

```
site ─► browser extension ─► (native messaging) ─► start ─► vendor's program
                                                     │ LD_LIBRARY_PATH=<extension>/lib
                                                     ▼
                                 libaetpkss.so.3 ─► the runtime's p11-kit-client.so
                                                     │ $XDG_RUNTIME_DIR/p11-kit/pkcs11
                                                     ▼
                                    the system's p11-kit server ─► Tessera's drivers
```

1. **The program runs inside the browser's sandbox,** not Tessera's. Flatpak deploys the extension on the host at `<installation>/runtime/io.github.pedrohqb.Tessera.Signer.<Name>/` (`/var/lib/flatpak` for a system installation, `~/.local/share/flatpak` for a user one). The browser gets read access to that folder through its override, and runs `x86_64/stable/active/files/start` there. `active` is Flatpak's link to the current version, so an update of the extension needs nothing in the browser.
2. **`start` puts the extension's `lib/` first** in the library search and runs the program. `lib/` has the library name the plugin asks for: for Web PKI and Softplan's WebSigner a link to `/usr/lib/x86_64-linux-gnu/pkcs11/p11-kit-client.so`, which is in every browser's runtime (the freedesktop one and those built on it; anywhere else the link is dangling, and that is expected); for Certisign's, a small module that wraps it (below).
3. **The plugin loads that name,** and so p11-kit's client, which reaches the system's p11-kit server through the socket step 1 of the browser's row gives it. All of Tessera's tokens are there.
4. **Tessera does not run the program** and has no permission for this. It reads the extension's `signer.toml` (mounted at `/app/lib/tessera/signers/<Name>`) to show what each browser needs.

### The names the plugins load

- **Web PKI and Softplan WebSigner** (the same Lacuna engine, a single self-contained .NET file of about 140 MB) load `libaetpkss.so.3` when the option **"Dispositivos SafeSign AET"** is ticked in the plugin's settings ("Cripto Dispositivos" tab). It is off by default, and the plugin keeps the setting in the browser profile's extension storage, which no file or policy can set beforehand: the user ticks it once per browser profile. One name is enough, because p11-kit serves every token; with `libeToken.so` too ("Tokens SafeNet"), and both options ticked, each token would show twice.
- **Certisign WebSigner** (an older C program, version 2.9.5 from 2019, about 2 MB, GTK 3) loads the modules its extension (2.18.0) names: the option **"SafeSign"** loads `libaetpkss.so.3` too, and **"SafeNet"** loads `libeToken.so`, which it also looks for in `/usr/lib`. Its `lib/` has `libaetpkss.so.3` only, and it is not a link but a small module of the extension's own (below); the SafeNet option finds nothing and does no harm.

### What the extensions hold

`flatpak/extensions/<Name>/`: the manifest (`build-extension`, with Tessera as its runtime, branch `stable`), the vendor's package as extra-data, `apply_extra.sh`, `start`, `signer.toml`, an AppStream `addon` component and the vendor's icon, the one its browser extension shows (the Linux packages carry none), which FlatPark lists and the extension installs as `icon.png`: Tessera's window shows it next to the signer, and, for a signer not installed, the one in Flatpak's AppStream data, if any. The extension point is in Tessera's manifest: `io.github.pedrohqb.Tessera.Signer`, directory `lib/tessera/signers`, with `subdirectories`, `no-autodownload` and `autodelete`.

`apply_extra.sh` takes only the program (and Certisign's dialogs): not the package's native messaging manifests, which name `/opt`, nor its maintainer scripts, which only copy and remove those manifests and the user's data folder.

```toml
# flatpak/extensions/WebPKI/signer.toml
name = "Lacuna Web PKI"
vendor = "Lacuna Software"
host = "com.lacunasoftware.webpki"          # the native messaging host's name
exec = "start"                              # relative to the extension's files
chrome_extensions = ["dcngeagmmhegagicpcmpinaoklddcgon", "nedeegdmhlnmboboahchfpkmdnnemapd"]
firefox_extensions = ["webpki@lacunasoftware.com", "webpki-beta@lacunasoftware.com"]
persist = ".lacuna-webpki"                  # the program's data folder, in the home folder
firefox_x11 = true                          # its dialogs draw only through X11
option = "Dispositivos SafeSign AET"        # what to tick once in the plugin
```

### Certisign's dialogs

Certisign's program opens its dialogs from a fixed place, `/opt/certisign-websigner/res/<name>.glade` (seven files, compiled into the binary), which no browser sandbox has. `apply_extra.sh` overwrites each of those paths in the binary, in place, with `res/<name>.glade` and a NUL: shorter, so nothing else moves. `start` then runs the program from the extension's `extra/` folder, which has `res/`. The program keeps its log and data in `~/.certisign-websigner`; if it cannot write there it goes on without the log.

### Certisign and the trust store

Through p11-kit's client, the browser reaches the system's p11-kit server, which serves Tessera's tokens and also the system's trust store (p11-kit's trust module): a token of some 150 CA certificates, with no keys. Certisign's program crashes (SIGSEGV) while listing that token's certificates, after twenty or so, before it reaches the real tokens; its log, `~/.certisign-websigner/WebPKIForChrome.log` in the browser's folder, says `Error while trying to list p11 certs … SIGSEGV`. Reproduced outside the browser with the program, its native messaging protocol and p11-kit's trust module. Web PKI and Softplan's WebSigner, a newer engine, do not crash there.

So Certisign's `libaetpkss.so.3` is `hide-trust.c`, built with the extension: a PKCS#11 module that loads `p11-kit-client.so` from the browser's runtime and hands it every call, except `C_GetSlotList`, which leaves out the slots whose token model is `p11-kit-trust` ("System Trust", "Default Trust"). The program then lists only Tessera's tokens, and asks for the PIN in its own dialog (`res/pin.glade`, which the rewritten path finds).

### The downloads

Flatpak downloads each package at install time and checks the pinned sha256. Two notes:

- **The vendors keep only their current package.** Lacuna's server ignores the version in the path and redirects to `blob.lacuna.net/webpki/webpki-setup-64.deb`, its current release; Softplan's does the same. So a new vendor release changes the file at the pinned URL: installing the extension fails (Flatpak's sha256 check) until the pin is updated, while installed copies keep working. The weekly download check opens an issue when that happens, and CI fails on the next push; updating means checking the new package (the program's path, the manifests' extension ids) and changing the URL, sha256 and size in the manifest (`scripts/pin-drivers.py` fills in the size), plus the version in the metainfo. Web PKI went from 2.16.1 to 2.17.0 that way the day after the extension was made. On FlatPark, each extension's `resolve-update.sh` reads the newest version from the package itself, from the first 64 KiB of the `.deb` (its control file, fetched with a Range request), so FlatPark's updater re-pins it by itself, twice a day, after running `apply_extra.sh` on the new download.
- **Softplan:** `websigner.softplan.com.br` sends an incomplete certificate chain (its certificate twice, without the Thawte intermediate), which Flatpak's downloader, like curl, rejects; distrobox-adv-br and Sigillum downloaded it without checking the certificate. The site is a CloudFront distribution, and its own address, `d1qzl0w4epswjx.cloudfront.net`, serves the same file with a valid certificate: that is the pinned URL. The version in the path is ignored by Softplan's server, which always sends its current package; a new version would change the sha256, and the weekly download check would say so.

## Setting up a browser

Each Flatpak browser needs, once per signer:

- read access to the extension's folder on the host;
- the program's data folder kept in the browser's own folder (`--persist`): the programs keep their data in `~/.<name>`, which Chrome, Edge and Brave in Flatpak do not see;
- for Firefox, with Web PKI and Softplan's WebSigner, the X11 socket: their dialogs (Avalonia) draw only through X11, and Firefox's Flatpak has only `fallback-x11`, which in a Wayland session does not give it;
- the manifest, in the browser's folder for them.

Tessera cannot do that from its sandbox: it would need write access to each browser's folder and to Flatpak's overrides. So the window's **Web signers** section shows, for each installed Flatpak browser and each installed signer, whether it is set up, and a button that copies the one command that sets it up. For Web PKI in Firefox, with a system installation:

```sh
flatpak override --user --filesystem=/var/lib/flatpak/runtime/io.github.pedrohqb.Tessera.Signer.WebPKI:ro --persist=.lacuna-webpki --socket=x11 org.mozilla.firefox && mkdir -p ~/.var/app/org.mozilla.firefox/.mozilla/native-messaging-hosts ~/.var/app/org.mozilla.firefox/config/mozilla/native-messaging-hosts && printf '%s\n' '{"name": "com.lacunasoftware.webpki", "description": "PKI Connector", "path": "/var/lib/flatpak/runtime/io.github.pedrohqb.Tessera.Signer.WebPKI/x86_64/stable/active/files/start", "type": "stdio", "allowed_extensions": ["webpki@lacunasoftware.com", "webpki-beta@lacunasoftware.com"]}' | tee ~/.var/app/org.mozilla.firefox/.mozilla/native-messaging-hosts/com.lacunasoftware.webpki.json ~/.var/app/org.mozilla.firefox/config/mozilla/native-messaging-hosts/com.lacunasoftware.webpki.json > /dev/null
```

The command has no loop and no bash-only syntax, so it runs the same in bash, zsh and fish. `tessera status` prints the same commands.

- **Which installation:** the extension is where Flatpak mounted it from, read from Tessera's `/proc/self/mountinfo` (`…/.local/share/flatpak/runtime/…` is the user's); otherwise Tessera's own installation, where the install command puts it.
- **The manifest folders** (in `~/.var/app/<id>/`):

  | Browser | Folder |
  |---|---|
  | Firefox (`org.mozilla.firefox`) | `.mozilla/native-messaging-hosts` and `config/mozilla/native-messaging-hosts` (Firefox 147 moved its user folders to `config/`) |
  | Chromium (`org.chromium.Chromium`) | `config/chromium/NativeMessagingHosts` |
  | Google Chrome (`com.google.Chrome`) | `config/google-chrome/NativeMessagingHosts` |
  | Microsoft Edge (`com.microsoft.Edge`) | `config/microsoft-edge/NativeMessagingHosts` |
  | Brave (`com.brave.Browser`) | `config/BraveSoftware/Brave-Browser/NativeMessagingHosts` |

- **The manifest** is the vendor's own, with the start script as its `path`. The Chromium-based browsers get every Chrome extension id of the signer: in Edge, Web PKI from the Edge Add-ons store has an id of its own.
- **Whether it is done** is read from the browser's user override (`~/.local/share/flatpak/overrides/<id>`, which Tessera already reads for step 1): the folder, the data folder and, where needed, X11. Tessera does not see the browser's manifest folders, and the same command writes both.

Then, in the browser: install the plugin's extension from the browser's store if it is not there, restart the browser, and, for Web PKI and Softplan's WebSigner, tick "Dispositivos SafeSign AET" once in the plugin's settings.

### Undoing it

`flatpak override --user --reset <browser>` removes everything the browser's user override has, step 1 included (do step 1 again afterwards). The manifests are the `<host>.json` files in the folders above. Uninstalling the extension leaves the browser's manifest pointing at a missing program, which only makes the plugin say the program is not installed.

## In native browsers

Native browsers need none of this: use the vendors' official packages and, in the plugin, add your distribution's p11-kit proxy (`p11-kit-proxy.so`) under "Cripto Dispositivos" → "Opções personalizadas" (see the README).

## Status

- CI builds the three extensions, installs them (so each `apply_extra.sh` runs on the real download) and checks the start script, the program, the library link and, for Certisign, that no dialog path is left at `/opt` and that every `res/` file the binary names exists.
- Sigillum's approach (the same programs, links and manifests, from a folder in the home) was tested on October 29, 2026 on Bluefin with a GD Burti token and a SerproID certificate: Web PKI and Softplan's WebSigner worked in Firefox, Chromium, Chrome, Edge and Brave.
- November 9, 2026, Bluefin, Firefox and Brave (Flatpak), Tessera's test build: Web PKI and Softplan's WebSigner worked at once. Certisign's WebSigner started, but crashed on the trust store (above) with its SafeNet option and found no library with its SafeSign option; with the module that hides the trust store's slots, under the SafeSign name, it worked too.
