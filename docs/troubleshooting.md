# Troubleshooting

## Where to look first

| What | Where |
|---|---|
| Overall status | `flatpak run --command=tessera io.github.pedrohqb.Tessera status` |
| Output of each driver (and of `tessera remote`) | `~/.var/app/io.github.pedrohqb.Tessera/data/tessera/logs/<driver>.log` (recreated each time the driver starts) |
| OpenSC's own log | Start Tessera with `flatpak run --env=OPENSC_DEBUG=3 io.github.pedrohqb.Tessera`; it goes to `opensc.log` in the folder above. `flatpak run --command=tessera io.github.pedrohqb.Tessera opensc opensc-tool --list-readers` and `opensc pkcs15-tool --list-certificates` read the card directly |
| SerproID log | `~/.var/app/io.github.pedrohqb.Tessera/.config/serproid/serproid.log`: one line per signature, with the program that requested it; `CKR_FUNCTION_CANCELED depois de receber 20 vezes AGUARDANDO` when the approval does not arrive |
| What was published | `ls ~/.config/pkcs11/modules/` (`tessera-*.module` files) |
| What the host sees | `p11-kit list-modules` (on the host) |
| What an app sees | `flatpak run --command=sh <app> -c 'p11-kit list-modules'` |
| The host server | `systemctl --user status p11-kit-server.socket p11-kit-server.service`; the log in `journalctl --user -u p11-kit-server.service` |
| An app's permissions | `flatpak info --show-permissions <app>` and `~/.local/share/flatpak/overrides/<app>` |

## Common problems

The questions users ask most, with the answers in plain words, are in the [FAQ](../FAQ.md#english) (in Portuguese and English); this table is the short form, with the causes.

| Symptom | Likely cause | What to do |
|---|---|---|
| "Unit p11-kit-server.socket does not exist" | The Debian family (Ubuntu 26.04, say): its p11-kit has no systemd units | The user service in the [README](../README.md#1-prepare-the-system-once) (the window's copy button gives it) |
| Web PKI or WebSigner shows no token (Flatpak browser) | the browser's step 1 or its web signer command missing, the browser not restarted, or the plugin's SafeSign option not ticked | See the [FAQ](../FAQ.md#the-web-pki-or-websigner-plugin-does-not-show-my-token) and [Web signers](signers.md#setting-up-a-browser) |
| "System connection" red | `p11-kit-server.socket` off | `systemctl --user enable --now p11-kit-server.socket` (on Debian and Ubuntu, the user service in [README](../README.md#1-prepare-the-system-once); the window's copy button gives it); the row recovers on its own within 15 s, without restarting Tessera; `tessera status` shows the same result |
| The app sees no token at all | step 1 is missing, or the app was opened before step 1 or before the driver was published | Step 1, then close and reopen the app |
| Tessera sees the token, the app does not | the driver has not responded yet (not published) or the app was started before publication | Check the driver's state in the window; reopen the app |
| "System connection" red, or a token missing in the apps, only while a driver has no token (OpenSC with no card, say) | p11-kit stops listing at a module of Tessera's without a token (a bug in its RPC; [History](history.md) item 43) | Fixed in Tessera: the drivers without a token are listed last, updated within a few seconds of a token going in or out. Press refresh (top left) to check at once |
| An app's step 2 says to open the app and restart Tessera | the app's folder did not exist when Tessera started, and its sandbox does not see it | Open the app once, then **Restart Tessera** (the button on the row, or the menu) |
| A driver extension installed, but still "Not installed" | extensions are mounted when Tessera starts | **Restart Tessera** (menu) |
| JSignPdf: "No private key was found" | the keystore type is still JKS, or the PIN was not typed | Keystore type **PKCS11** (Tessera's **Set up** chooses it), the PIN in **Keystore password**, then **Load keys** (see [Apps](apps.md#jsignpdf)) |
| Driver "Stopped: No answer 3 times in a row" | the driver hangs (for example, with no token) | **Restart**, or switch the driver off if it is not used |
| Duplicate certificates in Papers | `p11-kit-client.so` registered in Papers' database | **Set up** again (the current version removes the registration) |
| SerproID missing from PJeOffice Pro's Flatpak, while the native PJeOffice and the other apps list it | its serial was `1`, the same as the system's trust store, and PJeOffice keeps one token per serial (versions before 0.4.2; [Drivers](drivers.md#its-serial-number)) | Update Tessera. A JSignPdf set up for SerproID asks to be set up again once |
| SerproID: phone asks, approving gives "Ops!", and the request repeats | corrupted mechanism parameters (versions before the fix) | Update Tessera |
| `SSL_ERROR_HANDSHAKE_FAILED` after the PIN (Debian 12 or 13, LMDE, Ubuntu 24.04, Linux Mint 22); `CKR 0x63` or `0x71` at once from `sign-test.py` through the system's p11-kit | The system's p11-kit is 0.25.5 or older (`dpkg -l libp11-kit0`), which sends the signing parameters in the format before 0.25.6 ([History](history.md) item 38) | Not supported: use [distrobox-adv-br](https://github.com/pedrohqb/distrobox-adv-br) |
| `SSL_ERROR_DECRYPT_ERROR_ALERT` in Firefox with SerproID | PKCS#1 v1.5 signature instead of PSS | Update Tessera (the test build that did the size query had this effect) |
| "Não foi possível adicionar módulo" in Firefox | a hung driver published on the host | Switch the driver off in the window; current supervision already prevents this |
| Firefox Set up does nothing | Firefox was open | Close Firefox and try again |

## Built-in tools

All of them run inside Tessera's sandbox: `flatpak run --command=tessera io.github.pedrohqb.Tessera <command>`.

- **`status`:** drivers (serving, stopped, missing files), readers, tokens and certificates.
- **`probe-module MODULE`:** loads a driver **directly**, without p11-kit, and reports each step: initialization (with `CKF_OS_LOCKING_OK` and, if that fails, without it), library information, slots, tokens and mechanisms. This is what showed that SerproID worked directly, but not through p11-kit.
- **`probe-sign MODULE`:** finds a private key and signs a fixed SHA-256 digest with `CKM_RSA_PKCS` and with `CKM_RSA_PKCS_PSS`. `TESSERA_PIN=…` logs in; `TESSERA_MECH=PSS` limits it to mechanisms with that text in their name. To use it through p11-kit, pass `p11-kit-client.so` as the module and `P11_KIT_SERVER_ADDRESS=unix:path=$XDG_RUNTIME_DIR/tessera/<driver>`. It uses the `cryptoki` library, which asks for the size before signing. With SerproID, this leads to the path that returns PKCS#1 v1.5 (see [Drivers](drivers.md#serproid)). To imitate a browser, use `sign-test.py`.
- **`list-tokens`:** what the window uses to list a driver's tokens; reads `P11_KIT_SERVER_ADDRESS`.

### Call tracer

With `TESSERA_REMOTE_TRACE=1` in the environment of a driver's server, `tessera remote` writes to the driver's log every PKCS#11 call that reaches it (except `C_GenerateKeyPair` and `C_UnwrapKey`, which have eight arguments): arguments (mechanisms, parameters, attribute templates, data), result and duration. Example of a server started by hand, inside the sandbox:

```sh
flatpak run --command=sh io.github.pedrohqb.Tessera -c '
export LD_PRELOAD=libgcc_s.so.1 XDG_CONFIG_HOME=$HOME/.config \
  SSL_CERT_FILE=/etc/ssl/certs/ca-certificates.crt SSL_CERT_DIR=/etc/ssl/certs \
  LD_LIBRARY_PATH=/app/lib/tessera/extensions/SerproID/extra/usr/lib \
  P11_KIT_PRIVATEDIR=$XDG_DATA_HOME/tessera/libexec TESSERA_REMOTE_TRACE=1
S=$XDG_RUNTIME_DIR/tessera/teste; rm -f $S
/usr/libexec/p11-kit/p11-kit-server --foreground --name $S \
  --provider /app/lib/tessera/extensions/SerproID/extra/libserproidp11.so pkcs11: 2>$XDG_CACHE_HOME/trace.log &
P=$!; sleep 1
P11_KIT_SERVER_ADDRESS=unix:path=$S tessera list-tokens
kill $P'
cat ~/.var/app/io.github.pedrohqb.Tessera/cache/trace.log
```

`TESSERA_REMOTE_LOAD=raw` serves the driver's own function table, without p11-kit's managed layer. It is there for comparison; it has never been needed.

### Host-side logging

The host's p11-kit logs a module's calls with `log-calls: yes` in the module file. With Tessera's window closed (otherwise it rewrites the file):

```sh
F=~/.config/pkcs11/modules/tessera-serproid.module
cp $F ~/tessera-serproid.module.bak && echo 'log-calls: yes' >> $F
# … use the app …
journalctl --user -u p11-kit-server.service -o cat > ~/p11-log.txt
cp ~/tessera-serproid.module.bak $F
```

This is how it was found that Firefox requests exactly the same `CKM_RSA_PKCS_PSS` (SHA-256, MGF1-SHA256, salt of 32) as the tests.

## `scripts/sign-test.py`

A Python script, using only the standard library, that signs along the path a browser uses: a read-only session and a single `C_Sign`, with RSA-PSS SHA-256. It opens each signature with the certificate's public key and says what it is: "valid RSA-PSS", "PKCS#1 v1.5 … (NOT PSS)" or something else. It runs inside Tessera's sandbox, directly on the driver or through `p11-kit-client.so`:

```sh
cd ~/Downloads   # where sign-test.py is
flatpak run --command=env io.github.pedrohqb.Tessera \
  LD_PRELOAD=libgcc_s.so.1 XDG_CONFIG_HOME=$HOME/.config \
  SSL_CERT_FILE=/etc/ssl/certs/ca-certificates.crt SSL_CERT_DIR=/etc/ssl/certs \
  LD_LIBRARY_PATH=/app/lib/tessera/extensions/SerproID/extra/usr/lib \
  python3 - /app/lib/tessera/extensions/SerproID/extra/libserproidp11.so random < sign-test.py
```

| Option | What it changes |
|---|---|
| `fixed` / `random` | each word is one signature: the same digest every time, or random bytes (with none, one `random`) |
| `rw` | read-write session |
| `len` | asks for the size first (`C_Sign` with no buffer), like `cryptoki` |
| `oslock` | initializes with `CKF_OS_LOCKING_OK`, like p11-kit |
| `iface` | uses the table from `C_GetInterface` (PKCS#11 3.0) |
| `reinit` | `C_Initialize`, `C_Finalize`, `C_Initialize` first |
| `mimic` | first makes the calls that `p11-kit-server` makes on a connection |
| `clobber` | wipes the PSS parameters right after `C_SignInit` (shows the SerproID defect) |
| environment variable `PIN=…` | logs in first (SerproID does not need it); as an argument, it would become the label prefix |
| a text | picks the token whose label starts with it (default: `SerproID`) |

## External tools used in the investigation

- **`strace`:** immutable systems do not ship `strace`. A copy from Ubuntu 24.04, with the libraries it needs, run through the system loader (`/lib64/ld-linux-x86-64.so.2 --library-path lib strace.bin`), works without installing anything. Comparing the direct path with the path through p11-kit (`-e trace=%file,%network,%process`) showed that the driver does the same things in both. The size of the TLS writes (`-e trace=write`) showed that the request through p11-kit was 16 bytes smaller (451 instead of 467), and that was the clue that led to the cause.
- **`tools/spy.c`:** an `LD_PRELOAD` library that logs `getenv`, `getpid`, `readlink`, `uname`, `getpwuid` and the like, only when called from inside SerproID. It showed that the driver only queries `HOME`, the PID and `/proc/self/exe`. Build: `gcc -O2 -shared -fPIC -o libtessera-spy.so tools/spy.c -ldl`; use with `LD_PRELOAD="libgcc_s.so.1 /path/libtessera-spy.so" SPY_LOG=/path/spy.log`.
- **`tests/fake-driver/keeps-mechanism-pointer.c`:** a fake PKCS#11 driver that, like SerproID, keeps the mechanism pointer and only reads the parameters in `C_Sign`. It reproduces the defect through p11-kit and confirms the fix (see [Development](development.md)).
