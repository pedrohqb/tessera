# Drivers

## How the drivers reach Tessera

The drivers are not part of Tessera, because the vendors' licenses do not allow redistributing them. They use Flatpak's *extra-data*: at install time, Flatpak downloads each file from the vendor's site, checks the sha256 pinned in the manifest, and only then runs `flatpak/apply_extra.sh`, without network, inside the sandbox.

| Driver | Vendor file | sha256 |
|---|---|---|
| SafeSign IC 4.5.0.0 (A.E.T. Europe) | `SafeSignICStandardLinux4.5.0.0-AET.000ub2404x86_64.deb` | `7742e21e…1b91` |
| SafeSign, Token Administration | `SafeSignICStandardLinux4.5.0.0-AET.000redhat10x86_64.rpm` | `2f5ea462…3378` |
| GDBM, for SafeSign | Ubuntu 24.04 `libgdbm6t64_1.23-5.1build1_amd64.deb` and `libgdbm-compat4t64_1.23-5.1build1_amd64.deb` | `18d6d74a…645e`, `fb8564af…ee65` |
| SafeNet Authentication Client 10.9 (Thales) | `Linux_SAC_10.9_GA.zip` (DigiCert) | `46759cfe…013e` |
| SerproID 2.1.6 (Serpro) | `SerproID-2.1.6-amd64.deb` | `0ffa9ffe…e588` |

The sha256 values are the same as in [distrobox-adv-br](https://github.com/pedrohqb/distrobox-adv-br). Flatpak also requires each file's size; `scripts/pin-drivers.py` downloads each one, checks the sha256 and fills in the size in the manifest. Continuous integration runs this script before every build, so a file replaced on the vendor's site breaks the build instead of going unnoticed.

## The "clean" unpacking

`apply_extra.sh` follows the same "cleanups" as distrobox-adv-br:

- only the package's files are extracted (the `.deb`'s `data.tar`, read with `bsdtar`, because the runtime has neither `dpkg` nor `ar`);
- **no vendor script runs** (no `postinst`/`postrm`);
- the package's `/etc` is discarded;
- everything goes in `/app/extra/drivers/<driver>`, and nothing is written outside that folder;
- a driver that fails to unpack does not stop the installation: it shows up in the window as "Driver files missing", and the others carry on.

Each driver has a `driver.toml` in `/app/lib/tessera/drivers/<driver>/`. An excerpt from SerproID's:

```toml
name = "SerproID"
vendor = "Serpro"
description = "Cloud certificate; sign in with the SerproID app"
module = "/app/extra/drivers/serproid/libserproidp11.so"
library_path = ["/app/extra/drivers/serproid/usr/lib"]   # the driver's LD_LIBRARY_PATH
create_dirs = ["~/.config/serproid/certificados"]        # created before starting

[env]                                                    # environment of the driver's server
LD_PRELOAD = "libgcc_s.so.1"
XDG_CONFIG_HOME = "~/.config"
SSL_CERT_FILE = "/etc/ssl/certs/ca-certificates.crt"
SSL_CERT_DIR = "/etc/ssl/certs"

[[tools]]                                                # buttons in the window
name = "SerproID"
exec = ["/app/extra/drivers/serproid/serproid"]
```

## SafeSign IC (GD Burti)

- **Library:** `libaetpkss.so`, linked at `drivers/safesign/libaetpkss.so`.
- **GDBM:** the library needs `libgdbm_compat.so.4`, which the GNOME runtime does not have. The symptom was the driver not loading. The two Ubuntu 24.04 GDBM packages go to `drivers/safesign/gdbm`, which is part of `library_path`. The AUR package (`safesignidentityclient`) also depends on `gdbm`.
- **Token Administration:** the `tokenadmin` from the Ubuntu `.deb` needs wxWidgets, which the runtime does not have. The one from the RHEL 10 RPM does not, and that is the one Tessera uses, like the AUR. It runs through a launcher (`drivers/safesign/tokenadmin`) that puts the SafeSign libraries and GDBM on `LD_LIBRARY_PATH`. It needs full X11 (`--socket=x11`).
- **Observed behavior:** with the token connected, the first object search (`C_FindObjectsInit`) takes about 1.6 s. The OAB token has 4 certificates (the three CAs of the chain and the holder's). The driver advertises PSS with SHA-1/256/384/512, in addition to PKCS#1 v1.5.

## SafeNet Authentication Client (eToken 5100/5110, IDPrime)

- The zip contains packages for several distributions. As in distrobox-adv-br, only the Ubuntu "withoutUI" package is used (`safenetauthenticationclient-core_*_amd64.deb`).
- **Library:** `libeToken.so`.
- **Observed behavior:** with no token connected, the driver was slow to respond in the first test, and the system's p11-kit, when loading it, made Firefox show "Unable to add module". That is why supervision exists: a driver that does not respond is not published. Each driver can also be turned off in the window. It has not yet been tested with a real SafeNet token.

## SerproID

SerproID is a cloud certificate. The driver (`libserproidp11.so`) does not talk to any reader: it reads the list of certificates downloaded by the SerproID app and, for each signature, asks for approval on the holder's phone through Serpro's cloud. It was the hardest driver. All the quirks below were found in testing, and most of them do not show up when the driver is loaded directly by an application (like Firefox in the distrobox).

### Configuration it requires

1. **The certificates folder must exist,** even if empty: `~/.config/serproid/certificados`. Without it, the SerproID app closes. distrobox-adv-br does the same. Tessera creates it (`create_dirs`) before starting the driver and before opening the app.
2. **The configuration folder:** the driver reads `serproid/certificados` and `serproid/ambiente` and writes `serproid/serproid.log` in the user's configuration folder. Flatpak sets `XDG_CONFIG_HOME` to the app's own configuration folder, so the driver and the app would look in different places. `driver.toml` sets `XDG_CONFIG_HOME=~/.config`, and the manifest has `--persist=.config/serproid`, which keeps that folder in `~/.var/app/io.github.pedrohqb.Tessera/.config/serproid`.
3. **CA certificates:** the bundled OpenSSL looks for the roots in `/usr/lib/ssl`, the Debian location, which the runtime does not have. `SSL_CERT_FILE=/etc/ssl/certs/ca-certificates.crt` and `SSL_CERT_DIR=/etc/ssl/certs`.
4. **The app:** the startup script and the app's configuration use absolute paths (`cd /usr/share/serproid-desktop; ./jre/bin/java …`). `apply_extra.sh` replaces these paths, only in the text files, with the copy in `/app/extra/drivers/serproid`. The app is JavaFX and only draws through X11 (the error was `Unable to open DISPLAY`).
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
- **Notification:** for each request, it runs `sh -c "notify-send … 'Foi enviado um pedido de PIN para o seu dispositivo.'"`, with an image from its own folder that the desktop outside the sandbox cannot open. Tessera puts its own `notify-send` first on the drivers' `PATH` (in `~/.var/app/io.github.pedrohqb.Tessera/data/tessera/libexec/bin`), which strips the icon and images and shows Tessera's icon.
- **Log:** `~/.var/app/io.github.pedrohqb.Tessera/.config/serproid/serproid.log`. The file is appended to, not cleared. Lines like `Tipo de classe nao tratado em C_FindObjectsInit: 0x9` are NSS searches for objects the driver does not have, and do not indicate an error.
- **`maquinaId`:** a file in the same folder identifies the machine in the cloud. Tessera's is different from the distrobox's.

### What was not the cause (and was ruled out in testing)

The program name, the command line, the environment, signals, standard input and output, initialization with `CKF_OS_LOCKING_OK`, the finalize and re-initialize that p11-kit does before each connection, several instances of the driver at the same time, and the age of the connection. Each one was tested individually. The path is in [History](history.md).
