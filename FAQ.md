# Tessera: perguntas frequentes / FAQ

**[Português](#português)** · **[English](#english)** · **[README](README.md)**

---

## Português

**Instalação**
- [O PIN é pedido, mas a assinatura falha. Por quê?](#o-pin-é-pedido-mas-a-assinatura-falha-por-quê)
- [Instalo só para mim ou para todos os usuários?](#instalo-só-para-mim-ou-para-todos-os-usuários)
- [A instalação diz que falta o runtime org.gnome.Platform.](#a-instalação-diz-que-falta-o-runtime-orggnomeplatform)
- [Instalei antes uma versão de teste. O que faço?](#instalei-antes-uma-versão-de-teste-o-que-faço)
- [No Fedora, falta o pcscd ou o p11-kit-server.](#no-fedora-falta-o-pcscd-ou-o-p11-kit-server)
- [Funciona com Snap? E sem systemd?](#funciona-com-snap-e-sem-systemd)

**Uso**
- [Preciso do Sigillum?](#preciso-do-sigillum)
- [A linha "Conexão com o sistema" está vermelha.](#a-linha-conexão-com-o-sistema-está-vermelha)
- [O aplicativo não vê o token.](#o-aplicativo-não-vê-o-token)
- [O token não aparece no Tessera.](#o-token-não-aparece-no-tessera)
- [O token aparece duas vezes, ou o navegador pede o PIN duas vezes.](#o-token-aparece-duas-vezes-ou-o-navegador-pede-o-pin-duas-vezes)
- [Eu usava o SafeSign, o SafeNet ou o SerproID instalado no sistema. Preciso fazer algo?](#eu-usava-o-safesign-o-safenet-ou-o-serproid-instalado-no-sistema-preciso-fazer-algo)
- [Fechei a janela. O Tessera continua rodando?](#fechei-a-janela-o-tessera-continua-rodando)
- [Aparece um aviso no topo da janela dizendo que a assinatura não funciona.](#aparece-um-aviso-no-topo-da-janela-dizendo-que-a-assinatura-não-funciona)

**Outros**
- [O Tessera é seguro?](#o-tessera-é-seguro)
- [Como desinstalar?](#como-desinstalar)
- [Existe outro "Tessera" no Flathub?](#existe-outro-tessera-no-flathub)
- [Como conto que funcionou (ou não) no meu sistema?](#como-conto-que-funcionou-ou-não-no-meu-sistema)

### O PIN é pedido, mas a assinatura falha. Por quê?

Quase sempre é a versão do p11-kit do sistema. Com a 0.25.5 ou anterior, o Tessera mostra os tokens e o PIN é pedido, mas toda assinatura falha (no navegador, `SSL_ERROR_HANDSHAKE_FAILED`). Na versão 0.25.6, o p11-kit mudou a forma de transmitir os parâmetros da assinatura, e as versões anteriores não se entendem com o Tessera nesse ponto. Listar os tokens e pedir o PIN não usam esses parâmetros; por isso essa parte funciona.

Isso afeta o Debian 12 e 13, o LMDE 6 e 7, o Ubuntu 24.04 e o Linux Mint 22. Na família Debian, a versão aparece em `dpkg -l libp11-kit0`. Nesses sistemas, use o [distrobox-adv-br](https://github.com/pedrohqb/distrobox-adv-br). No MX Linux, o p11-kit do repositório de teste (MX Package Installer → MX Test Repo) resolve.

### Instalo só para mim ou para todos os usuários?

Tanto faz: com `--user`, só você; com `--system`, todos os usuários do computador (pede a senha de administrador). Nos dois casos, cada usuário que abrir o Tessera tem a sua própria instância, com os seus tokens, e os passos de cada aplicativo são por usuário.

### A instalação diz que falta o runtime org.gnome.Platform.

A base em que o Tessera roda (o runtime do GNOME) vem do Flathub, e o Flatpak só a baixa de um repositório da **mesma instalação** do Tessera. Se o Flathub estiver configurado só para o sistema e o Tessera for instalado para o usuário (ou o contrário), o runtime não é encontrado. Rode os três comandos do README com a mesma opção (`--user` ou `--system`): o `--if-not-exists` não muda nada no que já estiver configurado.

### Instalei antes uma versão de teste. O que faço?

Tire-a primeiro, sem apagar as configurações, e instale pelo FlatPark:

```bash
flatpak --user uninstall io.github.pedrohqb.Tessera && flatpak --user remote-delete tessera-test
```

### No Fedora, falta o pcscd ou o p11-kit-server.

Eles vêm instalados no Fedora e nos derivados atômicos, mas podem ter sido removidos. Reinstale com `sudo dnf install pcsc-lite pcsc-lite-ccid p11-kit-server` (nos atômicos, com `rpm-ostree install`).

### Funciona com Snap? E sem systemd?

**Snap** (o Firefox e o Chromium do Ubuntu): não. Use a versão em Flatpak ou o pacote nativo do navegador.

**Sem systemd** (Devuan, Void, antiX, MX com sysVinit): ainda não foi testado. O servidor p11-kit do usuário pode ser iniciado à mão, deixando o terminal aberto:

```bash
mkdir -p -m 700 "$XDG_RUNTIME_DIR/p11-kit"
p11-kit server -f -n "$XDG_RUNTIME_DIR/p11-kit/pkcs11" pkcs11:
```

### Preciso do Sigillum?

Só para o **Web PKI** e o **WebSigner** em **navegadores em Flatpak**. Nos navegadores nativos, use os pacotes oficiais dos fornecedores e acrescente o caminho do p11-kit no plugin, como explica o [README](README.md#web-pki-e-websigner). No Arch e derivados, `/lib/libp11-kit.so` também funciona: é a mesma biblioteca que `/usr/lib/p11-kit-proxy.so`.

### A linha "Conexão com o sistema" está vermelha.

O servidor p11-kit do sistema está desligado, e os aplicativos em Flatpak não veem os tokens. Rode o comando do servidor da sua distribuição (passo 1 da instalação), ou copie-o pelo botão da própria linha. Ela volta ao normal sozinha em alguns segundos.

### O aplicativo não vê o token.

Confira se ele aparece como **Pronto** na lista do Tessera e se foi aberto de novo depois do passo 1. O aplicativo precisa estar fechado quando você aperta **Configurar**. Se o PIN é pedido mas a assinatura falha, veja a [primeira pergunta](#o-pin-é-pedido-mas-a-assinatura-falha-por-quê).

### O token não aparece no Tessera.

Tire e ponha o token. Se um driver aparecer como "Parado", aperte **Reiniciar**. Confira também se o serviço de cartões está ligado (`sudo systemctl enable --now pcscd.socket`).

### O token aparece duas vezes, ou o navegador pede o PIN duas vezes.

No **Fedora e derivados**, o próprio sistema já carrega o p11-kit nos aplicativos nativos. Se o módulo do Tessera também foi acrescentado, cada token aparece duas vezes, e o botão do aplicativo vira **Retirar**, que tira só o do Tessera. Nos navegadores Chromium em Flatpak, veja [Apps](docs/apps.md#chromium-google-chrome-microsoft-edge-and-brave) (em inglês).

### Eu usava o SafeSign, o SafeNet ou o SerproID instalado no sistema. Preciso fazer algo?

Sim: retire a indicação da biblioteca do fornecedor dos bancos de certificados dos navegadores. Se o mesmo token chega ao navegador por dois caminhos, o do pacote do fornecedor e o do Tessera, os dois disputam o token, e o resultado é token em dobro, PIN pedido mais de uma vez ou falha na assinatura. O Tessera não retira módulos de outros programas por conta própria, porque eles podem ser usados por outra coisa; por isso é preciso fazer à mão, com os navegadores fechados.

- **Firefox:** Configurações → Privacidade e Segurança → **Dispositivos de segurança**. Selecione o módulo do SafeSign, do SafeNet ou do SerproID (o que não é do Tessera nem do sistema) e clique em **Descarregar**. Faça isso em cada perfil.
- **Chromium, Chrome, Edge e Brave:** liste os módulos do banco que eles compartilham e retire o do fornecedor pelo nome que aparecer:

  ```bash
  modutil -dbdir sql:$HOME/.pki/nssdb -list
  modutil -dbdir sql:$HOME/.pki/nssdb -delete "NOME DO MÓDULO"
  ```

  Se a pasta `~/.pki/nssdb` não existir, o banco está em `~/.local/share/pki/nssdb`: use esse caminho nos dois comandos.

Se não for mais usar os pacotes do fornecedor, pode também desinstalá-los do sistema.

### Fechei a janela. O Tessera continua rodando?

Sim: os tokens continuam disponíveis, e um ícone na bandeja mostra o resumo. No GNOME, o ícone depende da extensão AppIndicator, que o Bluefin e o Aurora já trazem. **Sair**, pelo menu ☰ ou pela bandeja, desliga o Tessera, e os tokens deixam de aparecer nos aplicativos. Para o Tessera abrir sozinho ao entrar na sessão, use o menu ☰ → **Iniciar automaticamente no login**; para tirar o ícone, ☰ → **Mostrar ícone na bandeja**.

### Aparece um aviso no topo da janela dizendo que a assinatura não funciona.

O Tessera identificou um p11-kit antigo demais no sistema. Veja a [primeira pergunta](#o-pin-é-pedido-mas-a-assinatura-falha-por-quê).

### O Tessera é seguro?

Os drivers são baixados dos sites oficiais dos fornecedores e conferidos pelo SHA256; um arquivo alterado faz a instalação falhar. Tudo entre os aplicativos e o Tessera passa por sockets locais, numa pasta que só o seu usuário abre: nada fica exposto na rede. Os drivers dos fornecedores rodam presos no sandbox do Tessera, sem acesso à sua pasta pessoal e sem nada instalado no sistema. O que o Tessera não protege é o mesmo que nenhum programa protege: um programa malicioso já rodando com o seu usuário. Detalhes em [Security](docs/architecture.md#security) (em inglês).

### Como desinstalar?

Saia do Tessera (menu ☰ → **Sair**) e rode:

```bash
flatpak uninstall --user --delete-data io.github.pedrohqb.Tessera
```

Numa instalação de sistema, troque `--user` por `--system`. A permissão do passo 1 de cada aplicativo é desfeita com `flatpak override --user --reset <aplicativo>`, que também apaga outras permissões que você tenha dado ao aplicativo.

### Existe outro "Tessera" no Flathub?

Sim, um gerador de QR Code (`fr.ethal.tessera`), sem relação com este projeto. O Flatpak identifica os aplicativos pelo ID, e o deste é `io.github.pedrohqb.Tessera`: os dois não se misturam.

### Como conto que funcionou (ou não) no meu sistema?

Abra uma [issue](https://github.com/pedrohqb/tessera/issues/new/choose) com o modelo **Compatibility report**, ou conte no grupo do [Telegram](https://t.me/advogados_linux): distribuição, token ou certificado, aplicativo (Flatpak ou nativo) e o que aconteceu. O que já foi testado está em [Compatibility](docs/compatibility.md).

---

## English

**Installation**
- [The PIN is asked, but signing fails. Why?](#the-pin-is-asked-but-signing-fails-why)
- [Should I install for myself or for every user?](#should-i-install-for-myself-or-for-every-user)
- [The installation says the org.gnome.Platform runtime is missing.](#the-installation-says-the-orggnomeplatform-runtime-is-missing)
- [I installed a test build before. What now?](#i-installed-a-test-build-before-what-now)
- [On Fedora, pcscd or p11-kit-server is missing.](#on-fedora-pcscd-or-p11-kit-server-is-missing)
- [Does it work with Snap? Without systemd?](#does-it-work-with-snap-without-systemd)

**Usage**
- [Do I need Sigillum?](#do-i-need-sigillum)
- [The "System connection" row is red.](#the-system-connection-row-is-red)
- [An app does not see the token.](#an-app-does-not-see-the-token)
- [The token does not appear in Tessera.](#the-token-does-not-appear-in-tessera)
- [The token shows twice, or the browser asks for the PIN twice.](#the-token-shows-twice-or-the-browser-asks-for-the-pin-twice)
- [I used SafeSign, SafeNet or SerproID installed on the system. Do I need to do anything?](#i-used-safesign-safenet-or-serproid-installed-on-the-system-do-i-need-to-do-anything)
- [I closed the window. Is Tessera still running?](#i-closed-the-window-is-tessera-still-running)
- [A warning at the top of the window says signing does not work.](#a-warning-at-the-top-of-the-window-says-signing-does-not-work)

**Other**
- [Is Tessera safe?](#is-tessera-safe)
- [How do I uninstall it?](#how-do-i-uninstall-it)
- [Is there another "Tessera" on Flathub?](#is-there-another-tessera-on-flathub)
- [How do I report that it works (or not) on my system?](#how-do-i-report-that-it-works-or-not-on-my-system)

### The PIN is asked, but signing fails. Why?

Almost always, it is the system's p11-kit version. With 0.25.5 or older, Tessera shows the tokens and the PIN is asked, but every signature fails (in the browser, `SSL_ERROR_HANDSHAKE_FAILED`). In 0.25.6, p11-kit changed how it sends the signing parameters, and older versions do not understand Tessera on that point. Listing tokens and asking for the PIN do not use those parameters, which is why that part works.

This affects Debian 12 and 13, LMDE 6 and 7, Ubuntu 24.04 and Linux Mint 22. On the Debian family, `dpkg -l libp11-kit0` shows the version. On those systems, use [distrobox-adv-br](https://github.com/pedrohqb/distrobox-adv-br). On MX Linux, the p11-kit from the test repository (MX Package Installer → MX Test Repo) solves it.

### Should I install for myself or for every user?

Either: with `--user`, only you; with `--system`, every user of the computer (it asks for the administrator password). Either way, each user who opens Tessera gets their own instance, with their own tokens, and each app's steps are per user.

### The installation says the org.gnome.Platform runtime is missing.

The base Tessera runs on (the GNOME runtime) comes from Flathub, and Flatpak only downloads it from a remote in the **same installation** as Tessera. If Flathub is set up only system-wide and Tessera is installed for the user (or the other way round), the runtime is not found. Run the README's three commands with the same option (`--user` or `--system`): `--if-not-exists` leaves what is already set up as it is.

### I installed a test build before. What now?

Remove it first, keeping your settings, and install from FlatPark:

```bash
flatpak --user uninstall io.github.pedrohqb.Tessera && flatpak --user remote-delete tessera-test
```

### On Fedora, pcscd or p11-kit-server is missing.

They come installed on Fedora and its atomic derivatives, but may have been removed. Reinstall them with `sudo dnf install pcsc-lite pcsc-lite-ccid p11-kit-server` (on atomic systems, with `rpm-ostree install`).

### Does it work with Snap? Without systemd?

**Snap** (Ubuntu's Firefox and Chromium): no. Use the browser's Flatpak or native package.

**Without systemd** (Devuan, Void, antiX, MX with sysVinit): not tested yet. The user's p11-kit server can be started by hand, keeping the terminal open:

```bash
mkdir -p -m 700 "$XDG_RUNTIME_DIR/p11-kit"
p11-kit server -f -n "$XDG_RUNTIME_DIR/p11-kit/pkcs11" pkcs11:
```

### Do I need Sigillum?

Only for **Web PKI** and **WebSigner** in **Flatpak browsers**. In native browsers, use the vendors' official packages and add the p11-kit path in the plugin, as the [README](README.md#web-pki-and-websigner) explains. On Arch and its derivatives, `/lib/libp11-kit.so` also works: it is the same library as `/usr/lib/p11-kit-proxy.so`.

### The "System connection" row is red.

The system's p11-kit server is off, and Flatpak apps do not see the tokens. Run your distribution's server command (installation step 1), or copy it with the row's own button. The row goes back to normal by itself in a few seconds.

### An app does not see the token.

Check that it shows as **Ready** in Tessera's list, and that it was opened again after step 1. The app must be closed when you press **Set up**. If the PIN is asked but signing fails, see the [first question](#the-pin-is-asked-but-signing-fails-why).

### The token does not appear in Tessera.

Unplug it and plug it in again. If a driver shows as "Stopped", press **Restart**. Also check that the smart card service is on (`sudo systemctl enable --now pcscd.socket`).

### The token shows twice, or the browser asks for the PIN twice.

On **Fedora and its derivatives**, the system itself already loads p11-kit in native apps. If Tessera's module was added too, each token shows twice, and the app's button becomes **Remove**, which takes out only Tessera's. For Chromium-based browsers as Flatpaks, see [Apps](docs/apps.md#chromium-google-chrome-microsoft-edge-and-brave).

### I used SafeSign, SafeNet or SerproID installed on the system. Do I need to do anything?

Yes: remove the vendor library's entry from the browsers' certificate databases. If the same token reaches the browser by two paths, the vendor package's and Tessera's, the two compete for the token, and the result is a token shown twice, the PIN asked more than once, or failed signatures. Tessera does not remove other programs' modules on its own, since something else may use them, so this is done by hand, with the browsers closed.

- **Firefox:** Settings → Privacy & Security → **Security Devices**. Select the SafeSign, SafeNet or SerproID module (the one that is neither Tessera's nor the system's) and click **Unload**. Do it in each profile.
- **Chromium, Chrome, Edge and Brave:** list the modules of the database they share and remove the vendor's by the name shown:

  ```bash
  modutil -dbdir sql:$HOME/.pki/nssdb -list
  modutil -dbdir sql:$HOME/.pki/nssdb -delete "MODULE NAME"
  ```

  If `~/.pki/nssdb` does not exist, the database is in `~/.local/share/pki/nssdb`: use that path in both commands.

If you will not use the vendor packages anymore, you can also uninstall them from the system.

### I closed the window. Is Tessera still running?

Yes: the tokens stay available, and a tray icon shows a summary. On GNOME, the icon needs the AppIndicator extension, which Bluefin and Aurora ship. **Quit**, from the ☰ menu or the tray, stops Tessera, and the tokens disappear from the apps. To have Tessera start when you log in, use the ☰ menu → **Start automatically at login**; to remove the icon, ☰ → **Show an icon in the tray**.

### A warning at the top of the window says signing does not work.

Tessera found a p11-kit too old on the system. See the [first question](#the-pin-is-asked-but-signing-fails-why).

### Is Tessera safe?

The drivers are downloaded from the vendors' official sites and checked against their SHA256; a changed file makes the installation fail. Everything between the apps and Tessera goes through local sockets, in a folder only your user can open: nothing is exposed on the network. The vendors' drivers run confined to Tessera's sandbox, without access to your home folder and with nothing installed on the system. What Tessera does not protect against is what no program does: a malicious program already running as your user. Details in [Security](docs/architecture.md#security).

### How do I uninstall it?

Quit Tessera (☰ menu → **Quit**) and run:

```bash
flatpak uninstall --user --delete-data io.github.pedrohqb.Tessera
```

For a system-wide install, use `--system` instead of `--user`. Each app's step 1 permission is undone with `flatpak override --user --reset <app>`, which also removes any other permission you gave the app.

### Is there another "Tessera" on Flathub?

Yes, a QR code generator (`fr.ethal.tessera`), unrelated to this project. Flatpak tells apps apart by their ID, and this one's is `io.github.pedrohqb.Tessera`: the two do not mix.

### How do I report that it works (or not) on my system?

Open an [issue](https://github.com/pedrohqb/tessera/issues/new/choose) with the **Compatibility report** template, or tell the [Telegram](https://t.me/advogados_linux) group: distribution, token or certificate, app (Flatpak or native) and what happened. What has been tested is in [Compatibility](docs/compatibility.md).
