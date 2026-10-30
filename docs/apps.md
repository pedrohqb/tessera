# Apps

In the window, each app has up to two steps, with the same names for all of them:

1. **Step 1 · access to the system's p11-kit server:** the `--filesystem=xdg-run/p11-kit/pkcs11` permission. The window shows the command with a copy button and marks ✓ once the `override` exists (without restarting Tessera). After running the command, you need to close and reopen the app.
2. **Step 2 · certificate database:** getting the app's NSS to reach p11-kit (the runtime's `p11-kit-client.so`, `/usr/lib/x86_64-linux-gnu/pkcs11/p11-kit-client.so`, or the roots module). How this is done varies from app to app, and it is what the **Set up** button takes care of. In Chromium browsers there is no step 2, except to remove a duplicate module ("Step 2 · remove the duplicate module", **Remove** button).

A completed step appears with ✓ and no button. Each app's summary says "Ready", "Step 1 missing", "Step 2 missing" or "Steps 1 and 2 missing".

First of all, the system server must be turned on, once per user: `systemctl --user enable --now p11-kit-server.socket`. The window's "System connection" row checks this end to end: through the system socket, Tessera lists the tokens an app would see and looks for its own (by name and serial number). It turns red if nobody answers or if Tessera's tokens are not there. Just seeing the socket is not enough: with the system server off, Flatpak's own trust server answers on the same path, and that is what fooled the first version of this row. The check repeats every 15 s while the window is open and whenever the token list changes; `tessera status` shows the same result in the terminal.

## PJeOffice Pro (FlatPark)

`br.jus.cnj.PJeOfficePro`. **Nothing to do.** The FlatPark package already has `xdg-run/p11-kit/pkcs11` and loads `p11-kit-client.so`. All it needs is the system server turned on. One note: a PJeOffice opened before Tessera does not see the drivers published afterwards; just reopen it.

## Firefox

`org.mozilla.firefox`.

1. `flatpak override --user --filesystem=xdg-run/p11-kit/pkcs11 org.mozilla.firefox`
2. Close Firefox and press **Set up**. Tessera reads `profiles.ini` (in `~/.var/app/org.mozilla.firefox/config/mozilla/firefox` in Firefox 147 and later, or in `.mozilla/firefox` in earlier versions) and registers the "Tessera (p11-kit)" module in the NSS database of **all** profiles. With Firefox open, the profile is locked (an `fcntl` lock on the `.parentlock` file; the `lock` link may be left over from a Firefox that closed badly and does not count), and Tessera warns you instead of touching it.

You can also do it by hand: Settings → Privacy & Security → Security Devices → Load, file `/usr/lib/x86_64-linux-gnu/pkcs11/p11-kit-client.so`.

In the Device Manager, one module appears with the slots for SerproID, SafeSign and the system trust. The "Builtin Roots Module" is the set of roots that ships with Firefox itself, not p11-kit, so there is no duplicate.

## Chromium, Google Chrome, Microsoft Edge and Brave

`org.chromium.Chromium`, `com.google.Chrome`, `com.microsoft.Edge`, `com.brave.Browser`. **Step 1 is enough.** When it starts NSS, Chromium loads the root certificates module, `libnssckbi.so`, on its own (`LoadNSSModule("Root Certs", "libnssckbi.so")` in `crypto/nss_util.cc`). In the Flatpak runtime, that file is p11-kit, which with the `override` reaches the system server and, therefore, Tessera's tokens. It is the same path as Papers.

In the first version, **Set up** also registered `p11-kit-client.so` in the browser's NSS database. Each token then appeared twice, and the browser asked for the SafeSign PIN once per copy: twice in Chrome, Edge and Brave, three times in Chromium (whose real `~/.pki/nssdb` may have yet another entry). Now **Set up** only removes that entry, if it exists. Step 2 shows as ready when there is nothing to remove.

Where the database lives ([Chromium documentation](https://chromium.googlesource.com/chromium/src/+/main/docs/linux/cert_management.md)): since Chromium 146, in `$XDG_DATA_HOME/pki/nssdb`, which in Flatpak is `~/.var/app/<browser>/data/pki/nssdb`. If the old database, `~/.pki/nssdb`, exists, it wins. Tessera follows the same rule:

| Browser | Old database (wins, if it exists) | New database |
|---|---|---|
| Chromium | the real `~/.pki/nssdb` (the package can use the home folder) | `~/.var/app/org.chromium.Chromium/data/pki/nssdb` |
| Google Chrome | `~/.var/app/com.google.Chrome/.pki/nssdb` (only with `--persist=.pki`) | `~/.var/app/com.google.Chrome/data/pki/nssdb` |
| Microsoft Edge | `~/.var/app/com.microsoft.Edge/.pki/nssdb` (the package has `--persist=.pki`) | `~/.var/app/com.microsoft.Edge/data/pki/nssdb` |
| Brave | `~/.var/app/com.brave.Browser/.pki/nssdb` (the package has `--persist=.pki`) | `~/.var/app/com.brave.Browser/data/pki/nssdb` |

In the test on October 29, 2026, Chrome, Edge and Brave were using the new database. The first version of Tessera only looked for the old one and said there was no database.

If a browser still asks for the PIN twice, check whether its database registers the roots module by hand. In the test, the real `~/.pki/nssdb` (Chromium's) had a `name=Mozilla Root Certs` block with `library=/usr/lib/x86_64-linux-gnu/libnssckbi.so`, common in token installation instructions. Chromium already loads that module on its own, and with the entry it loaded it twice. With the browser closed, this keeps a copy and removes the block:

```sh
cd ~/.pki/nssdb && cp pkcs11.txt pkcs11.txt.antes && \
awk -v RS= -v ORS='\n\n' 'index($0, "libnssckbi.so") == 0' pkcs11.txt.antes > pkcs11.txt
```

Tessera needs these folders only to remove the old entry. Flatpak only shows it the folders that already existed when it started.

## Papers

`org.gnome.Papers`. Papers signs PDFs through poppler, which looks for the NSS database in this order: the most recent `*default*` profile with `cert9.db` in `$XDG_CONFIG_HOME/mozilla/firefox` (in Flatpak, `~/.var/app/org.gnome.Papers/config/mozilla/firefox`), then `~/.mozilla/firefox`, then `/etc/pkcs11/nssdb`, then `~/.pki/nssdb`.

1. `flatpak override --user --filesystem=xdg-run/p11-kit/pkcs11 org.gnome.Papers`
2. **Set up** creates an **empty** NSS database in `~/.var/app/org.gnome.Papers/config/mozilla/firefox/tessera.default`.

The Papers database stays empty because, when NSS opens a database, it loads the root certificates module (`libnssckbi.so`) on its own. In the Flatpak runtime, that file is `load-p11-kit-trust.so`, and the p11-kit trust module, with the `override`, points to the system server. In other words, Papers already reaches the tokens this way. With `p11-kit-client.so` registered as well, each certificate appeared **twice** in the signing window. The dedicated database keeps poppler from falling back to `~/.pki/nssdb`, which lists the roots module once more. If an earlier version of Tessera registered the module in this database, **Set up** removes it.

In one test, resetting Papers' permissions in Flatseal wiped the `override`, and no certificate appeared anymore. Just redo step 1.

## Okular

`org.kde.okular`. Okular also signs through poppler, so it gets the same treatment as Papers:

1. `flatpak override --user --filesystem=xdg-run/p11-kit/pkcs11 org.kde.okular`. The package has `--filesystem=host`, but that does not include the p11-kit socket.
2. **Set up** creates the empty NSS database in `~/.var/app/org.kde.okular/config/mozilla/firefox/tessera.default`, the first place poppler looks (Okular's `$XDG_CONFIG_HOME` in Flatpak). In Settings → Configure Okular → Digital Signatures, the certificate database should be left at the default.

Okular uses the KDE runtime, which has the same freedesktop base as the GNOME runtime and the same p11-kit `libnssckbi.so`. It has not yet been tested with a real token.

## Native apps

An app installed on the system (not Flatpak) that uses `p11-kit-proxy.so` sees Tessera's tokens with no configuration: the proxy reads `~/.config/pkcs11/modules`.

## For Flatpak package maintainers

What is enough for an app to use Tessera's tokens, without depending on it:

```yaml
finish-args:
  - --filesystem=xdg-run/p11-kit/pkcs11
```

and to load `/usr/lib/x86_64-linux-gnu/pkcs11/p11-kit-client.so` (or use the runtime's NSS, which loads it through the roots module). This is what FlatPark's PJeOffice Pro does.
