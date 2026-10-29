# Tessera

**A PKCS#11 token hub for Flatpak.** Tessera runs the drivers for hardware tokens, smart cards and cloud certificates inside its own sandbox, and shares every token with your other apps through [p11-kit](https://p11-glue.github.io/p11-glue/p11-kit.html). You don't install any driver on the system. That makes it useful on immutable systems (Fedora Silverblue and Kinoite, Bluefin, Aurora, Bazzite), where installing a vendor's driver package on the host is hard or impossible.

*Tessera* was the Latin word for a small token that identified its bearer.

![Tessera](data/screenshots/main.png)

## How it works

```
Token / card ─► pcscd (host) ─► Tessera: drivers + p11-kit server
                                        │  $XDG_RUNTIME_DIR/tessera/pkcs11
                ┌───────────────────────┴───────────────────────┐
             Firefox                                          Papers
   (p11-kit-proxy.so + "remote:" line)            (p11-kit-proxy.so + "remote:" line)
```

- Tessera starts `p11-kit server` with the installed drivers and serves all tokens on `$XDG_RUNTIME_DIR/tessera/pkcs11`.
- A client app gets two things. The first is access to that socket, through `flatpak override --user --filesystem=xdg-run/tessera <app>`. The second is a one-line p11-kit module file in its own configuration, `~/.var/app/<app>/config/pkcs11/modules/tessera.module`, containing `remote: unix:path=…/tessera/pkcs11`. The app then loads the runtime's `p11-kit-proxy.so` and sees the tokens as if the drivers were installed locally.
- Why not the p11-kit default socket (`$XDG_RUNTIME_DIR/p11-kit/pkcs11`)? Inside every sandbox, Flatpak replaces that directory with its own p11-kit server for the host's trust store, so no app can serve or reach anything else there.
- The window shows the service, the card readers, the tokens and their certificates, and the installed drivers (with buttons for the vendors' tools). It also sets up the client apps.
- The service keeps running when the window is closed. It can also start at login (menu → *Start automatically at login*).

## Drivers

| Driver | Tokens | Source |
|---|---|---|
| OpenSC | PIV, YubiKey, CAC, many national ID cards | Built from source |
| SafeSign IC (A.E.T.) | GD Burti and other SafeSign tokens | Vendor `.deb`, downloaded at install time |
| SafeNet Authentication Client (Thales) | SafeNet eToken 5100/5110, IDPrime | Vendor `.zip`, downloaded at install time |
| SerproID (Serpro) | Serpro cloud certificate | Vendor `.deb`, downloaded at install time |

Vendor drivers use Flatpak's *extra-data*: the vendor's file is downloaded from the vendor at install time and checked against the sha256 pinned in the manifest. These are the same pins as [distrobox-adv-br](https://github.com/pedrohqb/distrobox-adv-br). Each package is unpacked into its own directory, and **no maintainer script runs**. As in distrobox-adv-br, the package's `/etc` is dropped, and SerproID's module is also offered as `libneoidp11.so`. A driver that fails to unpack is listed as missing; the others keep working.

More drivers can be added without changing Tessera. A driver is a directory with a `driver.toml`:

```toml
name = "Example Token"
vendor = "Example Inc."
description = "What it is for"
module = "lib/libexample-pkcs11.so"     # absolute, or relative to this directory
create_dirs = ["~/.config/example"]     # optional

[[tools]]                               # optional: buttons in the window
name = "Token Manager"
exec = ["bin/token-manager"]
```

Bundled drivers live in `/app/lib/tessera/drivers/<id>/`. Driver extensions, named `io.github.pedrohqb.Tessera.Driver.<Name>`, are mounted in `/app/lib/tessera/extensions/<Name>/`.

## Requirements on the host

- `pcscd` running, plus the `ccid` driver. Most distributions ship both; Fedora atomic and Universal Blue images include them.

Tessera uses its own socket, so it doesn't conflict with a host `p11-kit-server.socket`.

## Client apps

In Tessera, expand the app under *Apps*. Step 1 gives the `flatpak override` command, with a copy button; run it once in a terminal. Step 2's *Set up* button writes the module file.

**Firefox** (`org.mozilla.firefox`): close Firefox before *Set up*, then choose the profile folder. Tessera registers `p11-kit-proxy.so` in that profile. Or do it by hand in Firefox: *Settings → Privacy & Security → Security Devices → Load*, module file `/usr/lib/x86_64-linux-gnu/p11-kit-proxy.so`.

**Papers** (`org.gnome.Papers`): *Set up* also creates a small NSS database at `~/.var/app/org.gnome.Papers/config/mozilla/firefox/tessera.default`. That is the first place Papers (through poppler) looks for signing certificates.

**PJeOffice Pro** ([FlatPark](https://flatpark.org), `br.jus.cnj.PJeOfficePro`) loads `p11-kit-client.so` at the default socket through its own bridge. It needs its package to grant `xdg-run/tessera` and to point the bridge at Tessera, for example with `P11_KIT_SERVER_ADDRESS=unix:path=$XDG_RUNTIME_DIR/tessera/pkcs11` in its wrapper. A user override can't do this, because Flatpak clears that variable when it sets up its trust-store server.

Any other Flatpak app that loads PKCS#11 modules through p11-kit works the same way. From a terminal, `tessera setup-app <app-id>` writes the module file for that app.

## Command line

```
tessera                     open the window (and start the token service)
tessera --background        start the token service without a window
tessera serve               run only the token service, in the foreground
tessera status              print readers, tokens and drivers
tessera setup-app ID [DIR]  point the Flatpak app ID at Tessera; with DIR,
                            also register p11-kit-proxy.so in that NSS database
```

## Building

```sh
# Flatpak test bundle (fills in the drivers' sizes after checking their sha256):
python3 scripts/pin-drivers.py
flatpak-builder --user --install-deps-from=flathub --force-clean --install build-dir flatpak/io.github.pedrohqb.Tessera.yml

# Development build (needs GTK 4, libadwaita and pcsc-lite development files):
cargo build
TESSERA_DRIVERS_PATH=/path/to/drivers cargo run
```

For development, these environment variables override the defaults: `TESSERA_DRIVERS_PATH` (driver directories, separated by `:`), `TESSERA_SOCKET`, `TESSERA_P11KIT` (the `p11-kit` command) and `TESSERA_CLIENT_MODULE`.

Every push builds a test build in GitHub Actions: `tessera-repo.tar.gz`, a small Flatpak repository. A single-file `.flatpak` bundle can't carry the drivers' extra-data, which is why it isn't used. To install the test build:

```sh
tar -xzf tessera-repo.tar.gz
./install-test-repo.sh        # adds it as the local remote "tessera-test" and installs Tessera
```

Tagging `vX.Y.Z` publishes it, together with `tessera-x86_64.tar.gz`: the whole `/app` tree built against the GNOME runtime (Tessera, its data, the bundled libpcsclite and OpenSC), for packaging as extra-data.

## Status

This is an early version. The software path (Tessera → socket → client apps → NSS) is tested with SoftHSM 2, and the OpenSC module loads. Real tokens (SafeSign, SafeNet) and SerproID's cloud sign-in still need testing on a real system. Reports are welcome.

---

## Em português

O **Tessera** é uma central de tokens para Flatpak. Ele roda os drivers dos tokens (SafeSign/GD Burti, SafeNet, SerproID e OpenSC) dentro do próprio sandbox e compartilha os tokens com os outros aplicativos — PJeOffice Pro, Firefox, Papers — por meio do p11-kit, sem instalar nenhum driver no sistema. Foi feito pensando nos sistemas imutáveis (Bluefin, Silverblue, Aurora, Bazzite), em que instalar os pacotes dos fornecedores no sistema é difícil.

Os drivers dos fornecedores são baixados dos sites oficiais na instalação e conferidos pelo SHA256 fixado. Nenhum script de instalação do fornecedor é executado, seguindo a mesma "limpeza" do [distrobox-adv-br](https://github.com/pedrohqb/distrobox-adv-br).

No sistema, basta o `pcscd` (com o driver `ccid`) em execução. Para o Firefox e o Papers, a janela do Tessera mostra o comando `flatpak override` que libera o acesso ao soquete do Tessera, e o botão de configuração grava a linha `remote:` na configuração do p11-kit do aplicativo e registra o `p11-kit-proxy.so`. O PJeOffice Pro do FlatPark ainda precisa de um ajuste no pacote deles para usar o soquete do Tessera.

## License

GPL-3.0-or-later. The vendor drivers are not part of Tessera: they are downloaded from their vendors and are covered by the vendors' own licenses.
