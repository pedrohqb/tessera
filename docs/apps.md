# Apps

In the window, each app has up to two steps, with the same names for all of them:

1. **Step 1 · access to the system's p11-kit server:** the `--filesystem=xdg-run/p11-kit/pkcs11` permission. The window shows the command with a copy button and marks ✓ once the `override` exists (without restarting Tessera). After running the command, you need to close and reopen the app.
2. **Step 2 · certificate database:** getting the app's NSS to reach p11-kit (the runtime's `p11-kit-client.so`, `/usr/lib/x86_64-linux-gnu/pkcs11/p11-kit-client.so`, or the roots module). How this is done varies from app to app, and it is what the **Set up** button takes care of. In Chromium browsers there is no step 2, except to remove a duplicate module ("Step 2 · remove the duplicate module", **Remove** button).

A completed step appears with ✓ and no button. Each app's summary says "Ready", "Step 1 missing", "Step 2 missing" or "Steps 1 and 2 missing".

First of all, the system server must be turned on, once per user: `systemctl --user enable --now p11-kit-server.socket` (on Debian and Ubuntu, which ship no such unit, a plain user service: see [README](../README.md#1-prepare-the-system-once)). The window's "System connection" row checks this end to end: through the system socket, Tessera lists the tokens an app would see and looks for its own (by name and serial number). It turns red if nobody answers or if Tessera's tokens are not there. Just seeing the socket is not enough: with the system server off, Flatpak's own trust server answers on the same path, and that is what fooled the first version of this row. The check repeats every 15 s while the window is open and whenever the token list changes; `tessera status` shows the same result in the terminal.

## PJeOffice Pro (FlatPark)

`br.jus.cnj.PJeOfficePro`. **Nothing to do.** The FlatPark package already has `xdg-run/p11-kit/pkcs11` and loads `p11-kit-client.so`. All it needs is the system server turned on. One note: a PJeOffice opened before Tessera does not see the drivers published afterwards; just reopen it. PJeOffice tells tokens apart by serial number, and SerproID's (`1`) is also the trust store's, which the system's server lists first; Tessera gives SerproID a serial of its own, `SERPROID`, from 0.4.2 on ([Drivers](drivers.md#its-serial-number)).

## Assinador SERPRO (FlatPark)

`br.gov.serpro.desktop.assinador`. **Nothing to do**, as with PJeOffice. The FlatPark package already has `xdg-run/p11-kit/pkcs11`, and its wrapper sets `PKCS11_DRIVER` to a bridge of its own (`pkcs11-bridge.so`) that opens the runtime's `p11-kit-client.so`, hides p11-kit's trust slots and fixes two details the Assinador needs (`CKF_OS_LOCKING_OK`, the size of `CKA_TOKEN`).

## Firefox

`org.mozilla.firefox`.

1. `flatpak override --user --filesystem=xdg-run/p11-kit/pkcs11 org.mozilla.firefox`
2. Close Firefox and press **Set up**. Tessera reads `profiles.ini` (in `~/.var/app/org.mozilla.firefox/config/mozilla/firefox` in Firefox 147 and later, or in `.mozilla/firefox` in earlier versions) and registers the "Tessera (p11-kit)" module in the NSS database of **all** profiles. With Firefox open, the profile is locked (an `fcntl` lock on the `.parentlock` file; the `lock` link may be left over from a Firefox that closed badly and does not count), and Tessera warns you instead of touching it.

You can also do it by hand: Settings → Privacy & Security → Security Devices → Load, file `/usr/lib/x86_64-linux-gnu/pkcs11/p11-kit-client.so`.

In the Device Manager, one module appears with the slots for SerproID, SafeSign and the system trust. The "Builtin Roots Module" is the set of roots that ships with Firefox itself, not p11-kit, so there is no duplicate.

## Thunderbird

`org.mozilla.thunderbird`, and `org.mozilla.thunderbird_esr` for the ESR. Mozilla builds the Flatpak itself and moved it to these IDs in 2026; the old `org.mozilla.Thunderbird` stopped at 150.0b1 and 140.10.2esr. The ESR and the old ID show in the window only when installed (Tessera looks, read-only, at the Flatpak installation folders, `~/.local/share/flatpak/app` and `/var/lib/flatpak/app`). Like Firefox, Mozilla's build carries its own NSS, which does not load the runtime's p11-kit by itself, so:

1. `flatpak override --user --filesystem=xdg-run/p11-kit/pkcs11 org.mozilla.thunderbird`
2. Close Thunderbird and press **Set up**. Tessera registers the "Tessera (p11-kit)" module in every profile listed in `profiles.ini` under `~/.var/app/<id>/.thunderbird` (the package's `--persist=.thunderbird`; `config/thunderbird` too, in case a newer release moves it). The lock check is the same as Firefox's.

By hand: Settings → Privacy & Security → Security Devices → Load, file `/usr/lib/x86_64-linux-gnu/pkcs11/p11-kit-client.so`. Not yet tested with a real token.

## Evolution

`org.gnome.Evolution`. **Step 1 is enough**, as with Chromium. Evolution (evolution-data-server's `camel_init`) opens the real `~/.pki/nssdb` when there is one (the package has `--filesystem=~/.pki`), otherwise `~/.var/app/org.gnome.Evolution/data/pki/nssdb`. When it starts its S/MIME certificates, it adds NSS's root-certificates module (`libnssckbi.so`) itself, which in the runtime is p11-kit's trust module and reaches the system server after step 1. Step 2 only removes a "Tessera (p11-kit)" module, if one is there. Not yet tested with a real token.

## LibreOffice

`org.libreoffice.LibreOffice`. LibreOffice opens one NSS database per run (xmlsecurity's `nssinitializer.cxx`). The first of these wins: the `MOZILLA_CERTIFICATE_FOLDER` variable; the **certificate path** the user chose in Tools → Options → Security (`CertDir` in `registrymodifications.xcu`, a plain path that must exist); the default Thunderbird or Firefox profile under `~/.thunderbird` and `~/.mozilla/firefox` only; otherwise a temporary database, deleted on exit. The Flatpak uses the runtime's NSS and loads the root-certificates module (p11-kit, again) when the database has no roots.

1. `flatpak override --user --filesystem=xdg-run/p11-kit/pkcs11 org.libreoffice.LibreOffice`. The package has `--filesystem=host`, which does not include the p11-kit socket.
2. Close LibreOffice and press **Set up**. Tessera creates an **empty** NSS database in `~/.var/app/org.libreoffice.LibreOffice/data/tessera-nssdb` (as for Papers: the roots module reaches the tokens, and `p11-kit-client.so` as well would list each token twice) and writes it as `CertDir` in `~/.var/app/org.libreoffice.LibreOffice/config/libreoffice/4/user/registrymodifications.xcu`. LibreOffice rewrites that file when it closes, so Tessera waits while LibreOffice's `.lock` file is there (next to the profile, in `config/libreoffice/4/.lock`; a crash can leave it behind, and it can then be deleted). Tessera changes only the `CertDir` item, replaces the file in one step (a new file renamed over it) and keeps the one from before its first change as `registrymodifications.xcu.before-tessera`. If the user already chose a certificate path of their own, Set up keeps it and says so; to use Tessera's, clear it in LibreOffice first. Open LibreOffice once before, then start (or restart) Tessera: its sandbox only sees the app's folders that existed when it started.

The tokens' certificates then appear in File → Digital Signatures, and the PIN is asked when the list opens. Not yet tested with a real token.

## JSignPdf

`io.github.intoolswetrust.JSignPdf` (Flathub). JSignPdf is Java: its PKCS#11 keystore is Java's SunPKCS11 (or JSignPdf's fork of it), set up by one file, `pkcs11.cfg`, in its config folder (`~/.var/app/io.github.intoolswetrust.JSignPdf/config/jsignpdf`), read once at startup. That file only adds the PKCS11 keystore type: JSignPdf still opens its default, JKS (a file), until **Keystore type** says PKCS11, a choice it keeps in `config.properties` in the same folder.

1. `flatpak override --user --filesystem=xdg-run/p11-kit/pkcs11 io.github.intoolswetrust.JSignPdf`
2. Close JSignPdf, plug in the token and press **Set up**. Tessera writes:
   ```
   name=Tessera
   library=/usr/lib/x86_64-linux-gnu/pkcs11/p11-kit-client.so
   slotListIndex=N
   ```
   SunPKCS11 opens **one** slot, by default the first of all the slots, and on the system's p11-kit server the first ones are p11-kit's trust store, with no keys. So Tessera asks the system's server for its slots and writes the position of the first one holding one of Tessera's tokens. That position changes if the system's modules or the readers change: the button stays available to write it again. A `pkcs11.cfg` that was there before is kept as `pkcs11.cfg.before-tessera`.

   One slot is one token: with several of Tessera's tokens plugged in (the GD Burti and SerproID, say), the row shows a list to pick which one JSignPdf opens, and **Set up** writes that token's slot; the file keeps the choice in two comments. If the token picked is not the one the file was written for, the row asks for **Set up** again.

   It also sets `keystore.type=PKCS11` in `config.properties`, leaving the rest of the file as it was (the file as it was is kept as `config.properties.before-tessera`). JSignPdf writes that file again when it closes, which is why it must be closed first. Open JSignPdf once before, then start (or restart) Tessera: its sandbox only sees the app's config folder if it existed when Tessera started.

To sign: open the PDF, type the token's PIN in **Keystore password** (in Portuguese, "Palavra-passe do keystore"), press **Load keys**, pick the certificate under **Key alias** and press **Sign**. Leave **Keystore file** and **Key password** empty. Tested with the GD Burti (SafeSign extension). Without the PIN, or with the keystore type left at JKS, JSignPdf says only "No private key was found"

## Master PDF Editor

`net.code_industry.MasterPDFEditor` (Flathub). It loads one PKCS#11 library, chosen in Tools → Settings → Certificates → "PKCS#11 provider", and keeps it in its settings, a Qt file (`~/.var/app/net.code_industry.MasterPDFEditor/config/Code Industry/Master PDF Editor 5.conf`), as `pkcs11-helper-provider=` under `[General]` (seen in a user's file).

1. `flatpak override --user --filesystem=xdg-run/p11-kit/pkcs11 net.code_industry.MasterPDFEditor`
2. Close Master PDF Editor and press **Set up**: Tessera writes `pkcs11-helper-provider=/usr/lib/x86_64-linux-gnu/pkcs11/p11-kit-client.so`, leaving the rest of the file as it was (the file as it was before Tessera's first change is kept as `Master PDF Editor 5.conf.before-tessera`). Master PDF Editor writes that file again when it closes, which is why it must be closed first; it must also have been opened once, before Tessera started, so the file exists and Tessera sees it. The row still shows where the setting is, and the path to copy, for doing it by hand.

Not yet tested with a real token. The certificates of p11-kit's trust store may show in the signing list next to the token's.

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
2. **Set up** creates an **empty** NSS database in `~/.var/app/org.gnome.Papers/config/mozilla/firefox/tessera.default`. Open Papers once before, then start (or restart) Tessera: its sandbox only sees Papers' config folder if it existed when Tessera started.

The Papers database stays empty because, when NSS opens a database, it loads the root certificates module (`libnssckbi.so`) on its own. In the Flatpak runtime, that file is `load-p11-kit-trust.so`, and the p11-kit trust module, with the `override`, points to the system server. In other words, Papers already reaches the tokens this way. With `p11-kit-client.so` registered as well, each certificate appeared **twice** in the signing window. The dedicated database keeps poppler from falling back to `~/.pki/nssdb`, which lists the roots module once more. If an earlier version of Tessera registered the module in this database, **Set up** removes it.

In one test, resetting Papers' permissions in Flatseal wiped the `override`, and no certificate appeared anymore. Just redo step 1.

## Okular

`org.kde.okular`. Okular also signs through poppler, so it gets the same treatment as Papers:

1. `flatpak override --user --filesystem=xdg-run/p11-kit/pkcs11 org.kde.okular`. The package has `--filesystem=host`, but that does not include the p11-kit socket.
2. **Set up** creates the empty NSS database in `~/.var/app/org.kde.okular/config/mozilla/firefox/tessera.default`, the first place poppler looks (Okular's `$XDG_CONFIG_HOME` in Flatpak). In Settings → Configure Okular → Digital Signatures, the certificate database should be left at the default.

Okular uses the KDE runtime, which has the same freedesktop base as the GNOME runtime and the same p11-kit `libnssckbi.so`. It was tested with a real token ([History](history.md) item 35).

## Native apps

Apps installed from the distribution (not Flatpak) are on the window's **Native** tab. Every app, on both tabs, shows its own icon: an installed app's, else the one in the AppStream data Flatpak keeps for each remote (Flathub, FlatPark), which covers apps that are not installed, else Tessera's copy for the apps whose icon is free software (`data/app-icons`), else a generic one. The Chromium-based browsers always show Chromium's. They need no system connection: `p11-kit-proxy.so` reads `~/.config/pkcs11/modules`, where Tessera writes one `remote:` file per driver, and connects straight to Tessera's servers. What an NSS app needs is `p11-kit-proxy.so` in the database it opens; **Set up** registers it there (as `library=p11-kit-proxy.so`, by name: Fedora keeps it in `/usr/lib64`, Debian in `/usr/lib/x86_64-linux-gnu`, and `dlopen` finds it in either). Close the app first.

- **Firefox:** every profile listed in `profiles.ini`, under `~/.config/mozilla/firefox` (Firefox 147+) and `~/.mozilla/firefox`. A running Firefox is detected by the lock on its profile.
- **Chromium, Google Chrome, Microsoft Edge and Brave:** they share one database, `~/.pki/nssdb` if it exists, otherwise `~/.local/share/pki/nssdb` (Chromium 146+). The browsers are found by their folders in `~/.config` (outside Flatpak; the Flatpak does not look into the browsers' folders, and lists them generically). If the Chromium **Flatpak** is installed and the database is `~/.pki/nssdb`, Set up is refused: the Flatpak reads the real `~/.pki/nssdb` too and, since it already sees the tokens through its root-certificates module, it would list each token twice.
- **Thunderbird:** every profile listed in `profiles.ini`, under `~/.thunderbird` and `~/.config/thunderbird` (where a new install of a recent Thunderbird may go; Thunderbird keeps using `~/.thunderbird` while it exists).
- **Evolution:** the same database as the Chromium-based browsers (evolution-data-server: `~/.pki/nssdb` if it exists, otherwise `~/.local/share/pki/nssdb` from 3.62), so the same state and the same Set up. If the **Evolution Flatpak** is installed and the database is `~/.pki/nssdb`, Set up is refused, as with the Chromium Flatpak.
- **Okular and Papers:** poppler opens the most recent `*default*` Firefox profile that has a `cert9.db` (see Papers above). They depend on Firefox: the row has no button of its own, says "Set up Firefox first" until those profiles are set up, and "Needs Firefox" when there is no such profile.

- **LibreOffice:** with no certificate path of its own, it may open a Thunderbird profile, an old-style Firefox profile (not Firefox 147's `~/.config/mozilla/firefox`) or a temporary database, so Set up gives it one: a database in Tessera's own data folder (in the Flatpak, `~/.var/app/io.github.pedrohqb.Tessera/data/tessera/libreoffice-nssdb`) with `p11-kit-proxy.so`, written as `CertDir` in `~/.config/libreoffice/4/user/registrymodifications.xcu` (`4-suse/user` on openSUSE). If the user already chose a certificate path, that database gets the module instead (and if it is the real `~/.pki/nssdb` while the Chromium or Evolution Flatpak is installed, Set up is refused, as for those apps). The file is changed as for the Flatpak: only `CertDir`, in one step, with the earlier file kept as `registrymodifications.xcu.before-tessera`. LibreOffice must be closed (its `.lock` file, in `~/.config/libreoffice/4`), and opened once before.
- **PJeOffice Pro:** a Java program with its own list of PKCS#11 libraries, `list.a3=` in `~/.pjeoffice-pro/pjeoffice-pro.config` (written when PJeOffice closes; entries separated by `|`). Set up adds `p11-kit-proxy.so` there by full path, replacing any other p11-kit entry and keeping the other libraries. The path depends on the distribution, and a Flatpak cannot look into the host's `/usr`, so it comes from the host's `os-release`: `/usr/lib64` (Fedora, RHEL, openSUSE), `/usr/lib/x86_64-linux-gnu` (Debian, Ubuntu), `/usr/lib` (Arch and the rest). Only one path: PJeOffice skips paths that do not exist, but on Arch `/usr/lib64` is a link to `/usr/lib`, and both would load the module twice. PJeOffice (signer4j's `ConfigPersister`) also skips names that do not end in a known extension, such as `libp11-kit.so.0`; `/lib/libp11-kit.so`, set by hand, counts as done. Close PJeOffice first: it rewrites the file when it closes.

Before offering **Set up**, Tessera checks whether the app already gets p11-kit without it:

- **Fedora and its family (Bluefin, Aurora, Bazzite, RHEL…)** load p11-kit in every NSS database through crypto-policies: `/etc/crypto-policies/back-ends/nss.config` registers `p11-kit-proxy.so`, and NSS reads it whenever it opens a database. A fresh database lists nothing, yet NSS loads the proxy (CI found this; see [Development](development.md#tests)). openSUSE ships the same file, but its NSS does not load it, so the file alone does not tell: Tessera goes by the distribution in `os-release` (also all a Flatpak can see of the host), and CI checks on each distribution that this matches what NSS actually loads. The native Firefox, Thunderbird, Chromium-based browsers, Evolution, LibreOffice, Okular and Papers then show as **Ready** with no button. If Tessera's module was registered there earlier, each token shows twice, and the button becomes **Remove**, which takes out only Tessera's module (by its name, `Tessera (p11-kit)`).
- **Elsewhere,** a database that already lists a p11-kit module (the proxy, `libp11-kit.so`, the client module; not the trust module), whoever registered it, or that loads NSS's `nss-sysinit`, also shows as **Ready** with no button.

PJeOffice Pro does not use NSS, so it always needs its own Set up.

Snap packages (Ubuntu's Firefox and Chromium) are not supported: their profiles live under `~/snap`, and their confinement decides which modules they may load.

## For Flatpak package maintainers

What is enough for an app to use Tessera's tokens, without depending on it:

```yaml
finish-args:
  - --filesystem=xdg-run/p11-kit/pkcs11
```

and to load `/usr/lib/x86_64-linux-gnu/pkcs11/p11-kit-client.so` (or use the runtime's NSS, which loads it through the roots module). This is what FlatPark's PJeOffice Pro does.
