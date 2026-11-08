# Drivers

## How the drivers reach Tessera

Tessera's own package carries one driver, [OpenSC](#opensc), free software and built in. The vendors' drivers are not part of it: their licenses do not allow redistributing them, and each user needs only the one for their token. Each is a **Flatpak extension** of Tessera, installed by hand:

| Extension | For | Vendor files (sha256) |
|---|---|---|
| `io.github.pedrohqb.Tessera.Driver.SafeSign` | GD Burti (G+D StarSign) and other SafeSign tokens | SafeSign IC 4.5.0.0 `SafeSignICStandardLinux4.5.0.0-AET.000ub2404x86_64.deb` (`7742e21e…1b91`) and, for Token Administration, `…redhat10x86_64.rpm` (`2f5ea462…3378`); GDBM from Ubuntu 24.04, `libgdbm6t64_1.23-5.1build1_amd64.deb` (`18d6d74a…645e`) and `libgdbm-compat4t64_1.23-5.1build1_amd64.deb` (`fb8564af…ee65`) |
| `io.github.pedrohqb.Tessera.Driver.SafeNet` | SafeNet eToken 5100/5110, IDPrime cards | SafeNet Authentication Client 10.9, `Linux_SAC_10.9_GA.zip` from DigiCert (`46759cfe…013e`) |
| `io.github.pedrohqb.Tessera.Driver.SerproID` | Serpro's cloud certificate | `SerproID-2.1.6-amd64.deb` (`0ffa9ffe…e588`) |

```bash
flatpak install flatpark io.github.pedrohqb.Tessera.Driver.SafeSign
```

After installing one, quit Tessera from its menu and open it again: Flatpak mounts extensions when the app starts.

- **The extension point** is in Tessera's manifest: `io.github.pedrohqb.Tessera.Driver`, directory `lib/tessera/extensions`, with `subdirectories`, `no-autodownload` and `autodelete`. Each extension is mounted at `/app/lib/tessera/extensions/<Name>` (`SafeSign` for `…Driver.SafeSign`), where Tessera looks for a `driver.toml`. The driver's id is the folder's name in lowercase (`safesign`). Nothing is installed with Tessera on its own: the user picks the driver, and third parties publish theirs the same way (Certisign's RemoteID, by its own developer, is one).
- **What an extension holds** (`flatpak/extensions/<Name>/`): its manifest (`build-extension`, with Tessera as its `runtime`, branch `stable`), the vendor's files as *extra-data*, `apply_extra.sh`, `driver.toml`, an AppStream `addon` component and its icon. It has no permissions of its own: it runs in Tessera's sandbox with Tessera's.
- **Extra-data:** at install time, Flatpak downloads each file from the vendor's site, checks the pinned sha256, and only then runs the extension's `apply_extra.sh`, without network, in the extension's `extra/` folder. The sha256 values are the same as in [distrobox-adv-br](https://github.com/pedrohqb/distrobox-adv-br). Flatpak also requires each file's size; `scripts/pin-drivers.py` downloads each one, checks the sha256 and fills in the size. Continuous integration runs it before every build, then builds each extension and installs it, so `apply_extra.sh` runs on the real downloads and the module it should leave is checked. A weekly workflow checks the pins again.
- **Each driver separately:** a vendor that changes or removes a file breaks only its own extension, and a new vendor version is a new version of that extension alone.

### Which one to install

The window lists the drivers that are not installed, each with a button to its FlatPark page (whose install button hands it to the app store, where it is also an add-on on Tessera's page) and one that copies the install command. When a token is connected and its driver is missing, its reader says which one: the reader's name (a USB token is its own reader, named after its product: "SafeNet eToken 5100", "StarSign CUT S") or bytes of the card's ATR (IDPrime's `80 31 80 65 B0`, the eToken Java cards' `80 73 C8 21 10`, the G+D Sm@rtCafé Expert 6 and 7 of the StarSign tokens, `SCE6`/`SCE7`) point to SafeNet or SafeSign (`src/catalog.rs`). The command uses the installation Tessera is in (`--user` or `--system`, from `/.flatpak-info`). The rules are few on purpose: a wrong guess only shows a suggestion. Nothing is suggested while a running driver serves a token in that reader (by the slot's description, which OpenSC and SafeNet name after the reader): an IDPrime card that the built-in OpenSC serves needs no SafeNet. `tessera status` prints the readers' ATRs and the same suggestions.

## The "clean" unpacking

Each `apply_extra.sh` follows the same "cleanups" as distrobox-adv-br:

- only the package's files are extracted (the `.deb`'s `data.tar`, read with `bsdtar`, because the runtime has neither `dpkg` nor `ar`);
- **no vendor script runs** (no `postinst`/`postrm`);
- the package's `/etc` is discarded;
- everything goes in the extension's `extra/` folder, and nothing is written outside it; links inside it are relative, and the launchers name its final place, `/app/lib/tessera/extensions/<Name>/extra`;
- a driver that cannot be unpacked fails its own installation, with the reason in the output; a missing optional tool (SafeSign's Token Administration, the SerproID app) is only logged.

An excerpt from SerproID's `driver.toml` (paths relative to the extension's folder):

```toml
name = "SerproID"
vendor = "Serpro"
description = "Cloud certificate; sign in with the SerproID app"
module = "extra/libserproidp11.so"
library_path = ["extra/usr/lib"]                         # the driver's LD_LIBRARY_PATH
create_dirs = ["~/.config/serproid/certificados"]        # created before starting

[env]                                                    # environment of the driver's server
LD_PRELOAD = "libgcc_s.so.1"
XDG_CONFIG_HOME = "~/.config"
SSL_CERT_FILE = "/etc/ssl/certs/ca-certificates.crt"
SSL_CERT_DIR = "/etc/ssl/certs"

[[tools]]                                                # buttons in the window
name = "SerproID"
exec = ["extra/serproid"]
```

### Icons

The extensions' icons, shown in FlatPark's catalog, come from the vendors where they publish one: SafeSign's is Token Administration's own (256 px, from its package), SerproID's is the official one from Serpro's [integration manual](https://neoid.estaleiro.serpro.gov.br/documentacao/suporte/icones-logomarcas-integracao/). SafeNet's Linux package only has a 32 px badge, so its extension has an icon of its own, a generic USB token in Tessera's colors, without the vendor's mark.

## SafeSign IC (GD Burti)

- **Library:** `libaetpkss.so`, linked at `extra/libaetpkss.so`.
- **GDBM:** the library needs `libgdbm_compat.so.4`, which the GNOME runtime does not have. The symptom was the driver not loading. The two Ubuntu 24.04 GDBM packages go to `extra/gdbm`, which is part of `library_path`. The AUR package (`safesignidentityclient`) also depends on `gdbm`.
- **Token Administration:** the `tokenadmin` from the Ubuntu `.deb` needs wxWidgets, which the runtime does not have. The one from the RHEL 10 RPM does not, and that is the one Tessera uses, like the AUR. It runs through a launcher (`extra/tokenadmin`) that puts the SafeSign libraries and GDBM on `LD_LIBRARY_PATH`. It needs full X11 (`--socket=x11`).
- **Observed behavior:** with the token connected, the first object search (`C_FindObjectsInit`) takes about 1.6 s. The OAB token has 4 certificates (the three CAs of the chain and the holder's). The driver advertises PSS with SHA-1/256/384/512, in addition to PKCS#1 v1.5.

## SafeNet Authentication Client (eToken 5100/5110, IDPrime)

- The zip contains packages for several distributions. As in distrobox-adv-br, only the Ubuntu "withoutUI" package is used (`safenetauthenticationclient-core_*_amd64.deb`).
- **Library:** `libeToken.so`.
- **Observed behavior:** with no token connected, the driver was slow to respond in the first test, and the system's p11-kit, when loading it, made Firefox show "Unable to add module". That is why supervision exists: a driver that does not respond is not published. Each driver can also be turned off in the window. It has not yet been tested with a real SafeNet token.

## OpenSC

[OpenSC](https://github.com/OpenSC/OpenSC) is the free PKCS#11 driver for smart cards and tokens: Feitian's **ePass2003**, PIV cards and YubiKeys, OpenPGP cards, CardOS, Gemalto IDPrime, SmartCard-HSM, MyEID and several national ID cards (the list is OpenSC's `internal_card_drivers`, in `src/libopensc/ctx.c`). All of it is served, not only the ePass2003.

- **Built in, not downloaded.** OpenSC is LGPL 2.1, so the manifest builds it from the release tarball (0.27.1, pinned by sha256), right after pcsc-lite, which it uses to reach the readers. OpenSSL and secure messaging are required (`--enable-openssl --enable-sm`), so the build fails rather than leave out the drivers that need them, the ePass2003's among them. Its `opensc.module` for p11-kit would go to the SDK's read-only `/usr`; it is sent to `/app/share/p11-kit` and cleaned up, since Tessera publishes the module its own way. Its files are in Tessera's `/app` tree: `opensc-pkcs11.so` and `libopensc.so` in `/app/lib`, the command-line tools in `/app/bin`. They also go in the release tarball, so FlatPark's package (which unpacks that tree to `/app/extra/tessera`) has them too. `pkcs11-register`, which adds OpenSC to the browsers' NSS databases by itself, is left out.
- **`driver.toml`** names the module by a path relative to its own folder (`../../../opensc-pkcs11.so`), which resolves to `/app/lib/opensc-pkcs11.so` in Tessera's Flatpak and to `/app/extra/tessera/lib/opensc-pkcs11.so` in FlatPark's. A leading `./` in an `env` value is the driver's folder too.
- **Configuration.** `OPENSC_CONF` points at `flatpak/drivers/opensc/opensc.conf`, installed next to `driver.toml`, so OpenSC never reads `/app/etc` (which FlatPark's package does not have). It keeps OpenSC's defaults. Card contents are cached as OpenSC does by default, in the sandbox's `~/.var/app/io.github.pedrohqb.Tessera/cache/opensc`.
- **SafeNet's cards.** OpenSC's `idprime` driver knows the cards SafeNet's driver serves (IDPrime, eToken 5110). With both on, the same token would show twice. `driver.toml` has a `[when_on.safenet.env]` section: while SafeNet's server runs, `OPENSC_CONF` is `opensc-without-idprime.conf`, which lists every OpenSC card driver but `idprime`. Switch SafeNet off, or let it fail or be stopped by supervision, and OpenSC is started again with every driver and serves those cards (`settle` in `server.rs`); when SafeNet comes back, OpenSC is started again without `idprime`. The list follows OpenSC's; it must be checked when OpenSC is updated. Whether OpenSC's `starcos` or `cardos` drivers also claim some SafeSign or SafeNet tokens is not known yet; the duplicate warning below would show it, and the same kind of section would fix it.
- **GD Burti's tokens are left alone.** OpenSC has no driver for the G+D StarSign CUT S (one is proposed, [PR #3764](https://github.com/OpenSC/OpenSC/pull/3764), not merged). Without one, it probes the token with its card drivers' selects and polls its status, and the token hangs: SafeSign then cannot use it either. It was seen on Fedora with the system's OpenSC, and again with Tessera's: with OpenSC switched off in the window, the token worked through SafeSign at once. Both configurations have `ignored_readers = "StarSign"`: a reader whose name holds that text (the token is its own reader, "… StarSign CUT S …") is never opened by OpenSC. A G+D card in an ordinary reader is not covered (OpenSC ignores by reader name only); for it, switch OpenSC off in the window.
- **Duplicates in general.** A token two of Tessera's drivers serve (found by the holder's certificate, issuer and serial number, not the CA chain both may carry) gets a warning in the window and in `tessera status`. So does a token the apps see more often through the system's p11-kit server than Tessera serves it: the system has a driver of its own for it, typically its OpenSC (Fedora and Bluefin ship it, registered with p11-kit). Then Tessera's OpenSC should be switched off.
- **No slots.** With no reader connected (an ePass2003 is its own reader, so unplugged means none), OpenSC lists no slot at all. The `cryptoki` crate then asks again with an empty buffer, and through p11-kit's RPC that answers `CKR_BUFFER_TOO_SMALL`, which it retried without end: the listing hung until its time limit and supervision would have stopped the driver. Tessera now asks for the count first (`status.rs`, `slot_count`), for its own servers only: through `p11-kit-client.so` the count takes a second connection, so a second load of the driver, and the system's server always has slots (its trust store). No other driver had shown it, because they all keep at least one slot.
- **Tools.** OpenSC has no windowed tools; the command-line ones (a fixed list, OpenSC's own) run with `flatpak run io.github.pedrohqb.Tessera opensc TOOL [ARGS]` (`pkcs15-tool --change-pin`, `--unblock-pin`, `--list-certificates`, `opensc-tool --list-readers`, `pkcs11-tool --list-slots`…), with the driver's configuration; `pkcs11-tool` gets the module unless `--module` is given. `pkcs15-init`, which initializes blank cards, reads its profiles from `/app/share/opensc`, a path fixed at build time: in FlatPark's package, where Tessera's tree is under `/app/extra/tessera`, it does not find them. Changing and unblocking the PIN (`pkcs15-tool`) need no profiles.
- **Debugging.** `flatpak run --env=OPENSC_DEBUG=3 io.github.pedrohqb.Tessera`: OpenSC writes its log to standard error, which goes to the driver's log (`data/tessera/logs/opensc.log` in Tessera's folder).
- **Not yet tested with a real token.** Checked so far: the configuration files (`opensc-tool -D` with each one, against OpenSC 0.25), and the driver served by `tessera serve` with no reader, listed and published.

## SerproID

SerproID is a cloud certificate. The driver (`libserproidp11.so`) does not talk to any reader: it reads the list of certificates downloaded by the SerproID app and, for each signature, asks for approval on the holder's phone through Serpro's cloud. It was the hardest driver. All the quirks below were found in testing, and most of them do not show up when the driver is loaded directly by an application (like Firefox in the distrobox).

### Configuration it requires

1. **The certificates folder must exist,** even if empty: `~/.config/serproid/certificados`. Without it, the SerproID app closes. distrobox-adv-br does the same. Tessera creates it (`create_dirs`) before starting the driver and before opening the app.
2. **The configuration folder:** the driver reads `serproid/certificados` and `serproid/ambiente` and writes `serproid/serproid.log` in the user's configuration folder. Flatpak sets `XDG_CONFIG_HOME` to the app's own configuration folder, so the driver and the app would look in different places. `driver.toml` sets `XDG_CONFIG_HOME=~/.config`, and the manifest has `--persist=.config/serproid`, which keeps that folder in `~/.var/app/io.github.pedrohqb.Tessera/.config/serproid`.
3. **CA certificates:** the bundled OpenSSL looks for the roots in `/usr/lib/ssl`, the Debian location, which the runtime does not have. `SSL_CERT_FILE=/etc/ssl/certs/ca-certificates.crt` and `SSL_CERT_DIR=/etc/ssl/certs`.
4. **The app:** the startup script and the app's configuration use absolute paths (`cd /usr/share/serproid-desktop; ./jre/bin/java …`). `apply_extra.sh` replaces these paths, only in the text files, with the copy in `/app/lib/tessera/extensions/SerproID/extra`. The app is JavaFX and only draws through X11 (the error was `Unable to open DISPLAY`).
5. **`libneoidp11.so`:** the module is also offered under this name, as in distrobox-adv-br, because some signers look for it that way.

### The four quirks that break signing

1. **`undefined symbol: _Unwind_Resume_or_Rethrow`.** The library uses the exception unwinder from `libgcc_s` without declaring it as a dependency. In a C++ or Java application, `libgcc_s` is already loaded, and the error does not appear. In `p11-kit-remote` (in C) or in Tessera (in Rust), `dlopen` fails and p11-kit only returns `CKR_DEVICE_ERROR`. The fix is `LD_PRELOAD=libgcc_s.so.1` in the driver's environment. The real error only showed up with `P11_KIT_DEBUG` on a server started by hand.
2. **It keeps the mechanism pointer.** In `C_SignInit`, PKCS#11 only guarantees that the `CK_MECHANISM` (and the PSS parameters) is valid during the call. SerproID keeps the pointer and only reads the parameters in `C_Sign`. An application that loads the driver directly keeps the parameters alive, and everything works. Through p11-kit, they live in the RPC message, which is reused as soon as `C_SignInit` returns. In `C_Sign`, the PSS hash field was corrupted. In a fake driver that reproduces the defect, `0xd963a0444bf2382c` arrived instead of `0x250` (SHA-256). The request went to the cloud with the wrong hash, the phone received the request, the approval gave "Ops! Estamos enfrentando problemas" ("Oops! We are experiencing problems"), and the driver kept retrying until it returned `CKR_FUNCTION_CANCELED` (`Retornando CKR_FUNCTION_CANCELED depois de receber 20 vezes AGUARDANDO.` in the log). The proof was `sign-test.py clobber`: with the parameters wiped right after `C_SignInit`, the direct driver failed immediately. The fix (`src/stable.rs`) is for `tessera remote` to hand the driver a copy of the mechanism that lasts until the next `*Init` on the same session. It applies to all drivers.
3. **With a size query, it signs in PKCS#1 v1.5.** If `C_Sign` is first called without a buffer (to learn the size, as the `cryptoki` library does), SerproID returns a PKCS#1 v1.5 signature of the data, even when the requested mechanism is PSS. TLS 1.3 requires PSS, and the site responds with `SSL_ERROR_DECRYPT_ERROR_ALERT`. An intermediate attempt, which added the size query to work around item 2, had this effect and was reverted. NSS (Firefox, Papers) calls `C_Sign` only once, with a buffer, which is the right path.
4. **Labels with NUL.** The token fields (label, manufacturer, model, serial) come padded with NUL bytes instead of spaces. Tessera cleans them up when displaying them.

### Other observed behaviors

- **Mechanisms:** only `CKM_RSA_PKCS` and `CKM_RSA_PKCS_PSS`. There is no `CKM_SHA256_RSA_PKCS` (`Mecanismo invalido: 0x40` in the log). Whoever signs must compute the hash first.
- **Interface:** PKCS#11 2.20, without `C_GetInterface`.
- **Token:** `CKF_PROTECTED_AUTHENTICATION_PATH` and no login. The "password" is the approval on the phone.
- **Application identification:** for each signature, the driver reads `/proc/self/exe` and writes the path to the log. The name appears on the phone as "Aplicação" (Application) ("Tessera", through p11-kit). Testing showed that the name does not matter for approval.
- **Authorization window:** right after an approval on the phone, new signatures from the same application go through without asking, in about 0.2 s, for a few minutes. This confused several tests, because a signature "that worked" may not have gone through approval.
- **Cached signatures:** the same digest signed again returns the same signature, in 0.2 s.
- **Notification:** for each request, it runs `sh -c "notify-send … 'Foi enviado um pedido de PIN para o seu dispositivo.'"`, with an image from its own folder that the desktop outside the sandbox cannot open. Tessera puts its own `notify-send` first on the drivers' `PATH` (in `~/.var/app/io.github.pedrohqb.Tessera/data/tessera/libexec/bin`), which strips the icon and images and shows Tessera's icon. It sends the notification to Flatpak's notification portal itself (`gdbus`), with a new ID each time: libnotify's `notify-send`, in a sandbox, names every notification after a counter that starts at 1 in each process, so each one replaced the previous one, and Plasma (unlike GNOME) only updates a replaced notification in its history. The second request in a row, say when signing several documents, went unannounced. If the portal call fails, it falls back to `notify-send` without the driver's image.
- **Log:** `~/.var/app/io.github.pedrohqb.Tessera/.config/serproid/serproid.log`. The file is appended to, not cleared. Lines like `Tipo de classe nao tratado em C_FindObjectsInit: 0x9` are NSS searches for objects the driver does not have, and do not indicate an error.
- **`maquinaId`:** a file in the same folder identifies the machine in the cloud. Tessera's is different from the distrobox's.

### What was not the cause (and was ruled out in testing)

The program name, the command line, the environment, signals, standard input and output, initialization with `CKF_OS_LOCKING_OK`, the finalize and re-initialize that p11-kit does before each connection, several instances of the driver at the same time, and the age of the connection. Each one was tested individually. The path is in [History](history.md).
