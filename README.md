# Tessera

**[Português](#português)** · **[English](#english)**

---

## Português

O Tessera é uma central de tokens para Flatpak. Ele faz o token A3 ou o certificado em nuvem funcionar nos aplicativos em Flatpak, como o PJeOffice Pro e os navegadores, **sem instalar nenhum pacote de fornecedor no sistema**. Foi feito pensando nos sistemas imutáveis (Bluefin, Aurora, Bazzite, Fedora Silverblue e Kinoite), em que instalar o `.deb` ou o `.rpm` do fabricante do token é difícil ou impossível. Funciona também nas distribuições comuns.

*Tessera* era, em latim, a pequena peça que identificava quem a portava.

Estão incluídos:

1. o driver **SafeSign**, necessário para o token GD Burti, atualmente o mais usado pela advocacia;
2. o driver **SafeNet**, necessário para o token SafeNet 5100, o segundo mais usado pela advocacia;
3. o driver **SerproID**, necessário para o certificado digital em nuvem do Serpro, com o aplicativo SerproID;
4. o **Token Administration** do SafeSign, para trocar e desbloquear o PIN.

Os tokens ficam disponíveis para estes aplicativos:

| Aplicativo | Para quê |
|---|---|
| PJeOffice Pro ([FlatPark](https://flatpark.org)) | assinar no PJe |
| Firefox, Chromium, Google Chrome, Microsoft Edge e Brave | entrar com certificado em sites como Projudi e eproc |
| Papers e Okular | assinar PDF e conferir assinaturas |

Para o **Lacuna Web PKI** e o **Softplan WebSigner**, exigidos por alguns tribunais, como o e-SAJ:

- **nos navegadores em Flatpak**, use o [**Sigillum**](https://github.com/pedrohqb/sigillum), que os instala nos navegadores e os liga aos tokens do Tessera;
- **nos navegadores nativos**, use os pacotes oficiais dos fornecedores ([Web PKI](https://get.webpkiplugin.com/) e [WebSigner](https://websigner.softplan.com.br/)). Para que eles usem os tokens do Tessera, abra o plugin no navegador, vá em "Cripto Dispositivos" e, em "Opções personalizadas", acrescente o caminho do `p11-kit-proxy.so` da sua distribuição: `/usr/lib64/p11-kit-proxy.so` (Fedora e openSUSE), `/usr/lib/x86_64-linux-gnu/p11-kit-proxy.so` (Debian e Ubuntu) ou `/usr/lib/p11-kit-proxy.so` (Arch e derivados, onde `/lib/libp11-kit.so` também serve: é a mesma biblioteca).

![O Tessera com o SafeSign e o SerproID](data/screenshots/main.png)

---

### Instalação

#### 1. Preparar o sistema (uma vez)

O sistema precisa de três coisas: o Flatpak com o Flathub, o serviço de cartões (`pcscd`) com o driver CCID, e o servidor p11-kit do usuário. O **Fedora** (tradicional e atômico), o **Bluefin**, o **Aurora** e o **Bazzite** já vêm com os pacotes; basta ligar os serviços, mais abaixo.

Debian, Ubuntu, Linux Mint e derivados:

```bash
sudo apt install flatpak pcscd libccid p11-kit
```

**Atenção:** em sistemas com p11-kit 0.25.5 ou anterior, o Tessera mostra os tokens e o PIN é pedido, mas **a assinatura falha** (no navegador, `SSL_ERROR_HANDSHAKE_FAILED`). Isso foi confirmado no Debian 13 (MX Linux). Pela versão do pacote, o mesmo vale para o Debian 12, o Ubuntu 24.04 e o Linux Mint 22, ainda sem teste. A versão instalada aparece em `dpkg -l libp11-kit0`. O p11-kit mudou, na 0.25.6, a forma de transmitir os parâmetros da assinatura, e as versões anteriores não se entendem com o Tessera nesse ponto. Nesses sistemas, use o [distrobox-adv-br](https://github.com/pedrohqb/distrobox-adv-br).

Arch Linux, Manjaro, EndeavourOS e CachyOS:

```bash
sudo pacman -S flatpak pcsclite ccid
```

openSUSE Tumbleweed:

```bash
sudo zypper in flatpak pcsc-ccid p11-kit-server
```

Depois, ligue o serviço de cartões e o servidor p11-kit do usuário. No Fedora, no Arch, no openSUSE e nos seus derivados:

```bash
sudo systemctl enable --now pcscd.socket
systemctl --user enable --now p11-kit-server.socket
```

No Debian, no Ubuntu, no Linux Mint e nos seus derivados, o pacote do p11-kit traz o servidor, mas não o `p11-kit-server.socket`, e o servidor não aceita ser ativado por socket. Ele entra como um serviço comum do usuário, que sobe no login:

```bash
sudo systemctl enable --now pcscd.socket
mkdir -p ~/.config/systemd/user
cat > ~/.config/systemd/user/p11-kit-server.service <<'EOF'
[Unit]
Description=p11-kit server
Documentation=man:p11-kit(8)

[Service]
ExecStartPre=/bin/mkdir -p -m 700 %t/p11-kit
ExecStart=/usr/bin/p11-kit server -f -n %t/p11-kit/pkcs11 pkcs11:
Restart=on-failure

[Install]
WantedBy=default.target
EOF
systemctl --user daemon-reload
systemctl --user enable --now p11-kit-server.service
```

O comando do servidor p11-kit é o que deixa os aplicativos verem os tokens do Tessera. A janela do Tessera também o oferece, com um botão de copiar (no Debian e no Ubuntu, numa linha só, que faz o mesmo).

**Observação:** num Fedora em que algum desses pacotes tenha sido removido, reinstale-os com `sudo dnf install pcsc-lite pcsc-lite-ccid p11-kit-server`.

#### 2. Instalar o Tessera

[![Get it on FlatPark](https://flatpark.org/badges/get-it-on-flatpark.svg)](https://flatpark.org/apps/io.github.pedrohqb.Tessera)

O Tessera é distribuído pelo [FlatPark](https://flatpark.org), um repositório de Flatpaks. A base em que ele roda (o runtime do GNOME) vem do Flathub, e o Flatpak só a baixa de um repositório da **mesma instalação** do Tessera: de usuário (`--user`) ou de sistema (`--system`). Por isso os comandos abaixo adicionam os dois repositórios; se algum já estiver configurado, o `--if-not-exists` não muda nada.

Para instalar só para você:

```bash
flatpak --user remote-add --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo
flatpak --user remote-add --if-not-exists flatpark https://dl.flatpark.org/flatpark.flatpakrepo
flatpak --user install flatpark io.github.pedrohqb.Tessera
```

Na instalação, o Flatpak baixa os drivers dos sites oficiais dos fornecedores e confere cada um pelo SHA256. As atualizações chegam com o `flatpak update`, como as dos outros aplicativos.

Para instalar para todos os usuários do computador (instalação de sistema, que pede a senha de administrador):

```bash
flatpak --system remote-add --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo
flatpak --system remote-add --if-not-exists flatpark https://dl.flatpark.org/flatpark.flatpakrepo
flatpak --system install flatpark io.github.pedrohqb.Tessera
```

Funciona do mesmo jeito: cada usuário que abrir o Tessera tem a sua própria instância, com os seus tokens, e o passo 1 de cada aplicativo continua sendo por usuário.

**Observação:** quem instalou antes uma versão de teste (o repositório `tessera-test`) deve tirá-la primeiro, sem apagar as configurações: `flatpak --user uninstall io.github.pedrohqb.Tessera && flatpak --user remote-delete tessera-test`.

---

### Uso

Abra o **Tessera** pelo menu de aplicativos. Com o token conectado, ele aparece em **Tokens**, com os seus certificados. O certificado SerproID aparece depois de você entrar pelo aplicativo SerproID, no botão da linha do driver.

Em **Aplicativos**, cada aplicativo mostra o que falta. São no máximo dois passos, feitos uma vez:

- **Passo 1 · acesso ao servidor p11-kit do sistema:** um comando para rodar no terminal, com botão de copiar. Por exemplo, para o Firefox:
  ```bash
  flatpak override --user --filesystem=xdg-run/p11-kit/pkcs11 org.mozilla.firefox
  ```
  Depois, feche o aplicativo e abra de novo.
- **Passo 2 · banco de certificados:** o botão **Configurar**. Só o Firefox, o Papers e o Okular têm esse passo. No Firefox, feche o navegador antes de apertar o botão.

| Aplicativo | Passos |
|---|---|
| PJeOffice Pro | nenhum: basta o servidor p11-kit do passo de instalação |
| Firefox | passo 1 e passo 2 |
| Chromium, Chrome, Edge e Brave | só o passo 1 |
| Papers e Okular | passo 1 e passo 2 |

Quando tudo estiver certo, o aplicativo aparece como **Pronto**, e a linha **Conexão com o sistema** diz quantos tokens do Tessera os aplicativos estão vendo.

A seção **Aplicativos** tem duas abas: **Flatpak**, com os passos acima, e **Nativos**, para os aplicativos instalados pela própria distribuição (o Firefox em `.rpm` ou `.deb`, por exemplo). Os nativos não precisam da conexão com o sistema: eles chegam ao Tessera pelo `p11-kit-proxy.so`, e o único passo é o botão **Configurar**, que o acrescenta ao banco de certificados do aplicativo. Feche o aplicativo antes.

| Aplicativo nativo | Onde o Tessera configura |
|---|---|
| Firefox | todos os perfis |
| Chromium, Chrome, Edge e Brave | o banco que eles compartilham (`~/.pki/nssdb` ou `~/.local/share/pki/nssdb`) |
| Okular e Papers | nenhum botão próprio: eles usam o banco do perfil padrão do Firefox, então ficam prontos quando o Firefox estiver configurado |
| PJeOffice Pro | a lista de bibliotecas A3 (`list.a3`) em `~/.pjeoffice-pro/pjeoffice-pro.config`, com o caminho do p11-kit da sua distribuição |

Antes de oferecer o botão, o Tessera confere se o aplicativo já recebe o p11-kit sem ele. No **Fedora e derivados** (Bluefin, Aurora, Bazzite), o próprio sistema já carrega o p11-kit em todo aplicativo que usa NSS: Firefox, Chrome, Chromium, Okular e Papers nativos aparecem como **Pronto**, sem botão. Se o módulo do Tessera tiver sido acrescentado antes, cada token aparece duas vezes, e o botão vira **Retirar**, que tira só o do Tessera. Nas outras distribuições, um banco que já tenha o p11-kit, posto por quem for, também aparece como **Pronto**. O PJeOffice Pro não usa NSS e sempre precisa do seu **Configurar**.

#### Em segundo plano

- **Fechar a janela não desliga o Tessera:** os tokens continuam disponíveis, e um ícone na bandeja mostra o resumo. No GNOME, o ícone depende da extensão AppIndicator, que o Bluefin e o Aurora já trazem. Para tirar o ícone, use o menu ☰ → **Mostrar ícone na bandeja**.
- **Sair**, pelo menu ou pela bandeja, desliga o Tessera e os tokens deixam de aparecer nos aplicativos.
- **Para o Tessera abrir sozinho ao entrar na sessão,** use o menu ☰ → **Iniciar automaticamente no login**.

#### Se algo não funcionar

- **A linha "Conexão com o sistema" está vermelha:** rode `systemctl --user enable --now p11-kit-server.socket`. Ela volta ao normal sozinha em alguns segundos.
- **O aplicativo não vê o token:** confira se ele está como **Pronto** na lista e se foi aberto de novo depois do passo 1.
- **O token não aparece no Tessera:** tire e ponha o token. Se um driver aparecer como "Parado", aperte **Reiniciar**.
- **Um navegador pede o PIN duas vezes:** veja [Aplicativos](docs/apps.md#chromium-google-chrome-microsoft-edge-and-brave).

Mais casos em [Troubleshooting](docs/troubleshooting.md) (em inglês).

---

### Desinstalação

Saia do Tessera (menu ☰ → **Sair**) e rode:

```bash
flatpak uninstall --user --delete-data io.github.pedrohqb.Tessera
```

Numa instalação de sistema, troque `--user` por `--system`.

A permissão do passo 1 de cada aplicativo é desfeita com `flatpak override --user --reset <aplicativo>`. Esse comando também apaga outras permissões que você tenha dado ao aplicativo.

---

### Documentação

A documentação técnica, em inglês, está em [docs/](docs/README.md):

- **[Architecture](docs/architecture.md):** como o token sai do sandbox do Tessera e chega aos aplicativos, e por que não há caminho mais simples.
- **[Drivers](docs/drivers.md):** origem e verificação de cada driver, e o que foi preciso ajustar, sobretudo no SerproID.
- **[Apps](docs/apps.md):** o que cada aplicativo precisa e por quê.
- **[Installation and usage](docs/usage.md):** a janela, a bandeja e a linha de comando.
- **[Compatibility](docs/compatibility.md):** o que é conferido automaticamente a cada mudança e o que já foi testado à mão com tokens reais. Para acrescentar o seu teste, abra uma issue com o modelo "Compatibility report".
- **[Troubleshooting](docs/troubleshooting.md):** logs, ferramentas de teste e como investigar um problema.
- **[History](docs/history.md):** os problemas encontrados nos testes com tokens reais e como cada um foi resolvido.
- **[Development](docs/development.md):** compilação, testes, versões e como acrescentar um driver.

---

### Suporte

Havendo qualquer dificuldade, abra uma [issue](https://github.com/pedrohqb/tessera/issues) ou peça ajuda no grupo oficial no [Telegram](https://t.me/advogados_linux).

---

### Licença

GPL-3.0-or-later. Os drivers dos fornecedores não fazem parte do Tessera: são baixados dos fornecedores na instalação e seguem as licenças deles.

---

## English

Tessera is a token hub for Flatpak. It makes A3 tokens and cloud certificates work in Flatpak apps, such as PJeOffice Pro and web browsers, **without installing any vendor package on the system**. It was made with immutable systems in mind (Bluefin, Aurora, Bazzite, Fedora Silverblue and Kinoite), where installing a token vendor's `.deb` or `.rpm` is hard or impossible. It also works on regular distributions.

*Tessera* was the Latin word for a small token that identified its bearer.

Included:

1. the **SafeSign** driver, for GD Burti tokens, currently the most used by Brazilian lawyers;
2. the **SafeNet** driver, for SafeNet 5100 tokens, the second most used;
3. the **SerproID** driver, for Serpro's cloud certificate, with the SerproID app;
4. SafeSign's **Token Administration**, to change and unlock the PIN.

The tokens are available to these apps:

| App | For |
|---|---|
| PJeOffice Pro ([FlatPark](https://flatpark.org)) | signing in PJe |
| Firefox, Chromium, Google Chrome, Microsoft Edge and Brave | certificate login on sites such as Projudi and eproc |
| Papers and Okular | signing PDFs and checking signatures |

For **Lacuna Web PKI** and **Softplan WebSigner**, required by some courts (e-SAJ, for example):

- **in Flatpak browsers**, use [**Sigillum**](https://github.com/pedrohqb/sigillum), which installs them in the browsers and connects them to Tessera's tokens;
- **in native browsers**, use the vendors' official packages ([Web PKI](https://get.webpkiplugin.com/) and [WebSigner](https://websigner.softplan.com.br/)). For them to use Tessera's tokens, open the plugin in the browser, go to "Cripto Dispositivos" and, under "Opções personalizadas", add the path of your distribution's `p11-kit-proxy.so`: `/usr/lib64/p11-kit-proxy.so` (Fedora and openSUSE), `/usr/lib/x86_64-linux-gnu/p11-kit-proxy.so` (Debian and Ubuntu) or `/usr/lib/p11-kit-proxy.so` (Arch and derivatives, where `/lib/libp11-kit.so` works too: it is the same library).

### Installation

#### 1. Prepare the system (once)

The system needs three things: Flatpak with Flathub, the smart card service (`pcscd`) with the CCID driver, and the user's p11-kit server. **Fedora** (traditional and atomic), **Bluefin**, **Aurora** and **Bazzite** already ship the packages; just turn on the services, below.

Debian, Ubuntu, Linux Mint and derivatives:

```bash
sudo apt install flatpak pcscd libccid p11-kit
```

**Warning:** on systems with p11-kit 0.25.5 or older, Tessera shows the tokens and the PIN is asked, but **signing fails** (in the browser, `SSL_ERROR_HANDSHAKE_FAILED`). This was confirmed on Debian 13 (MX Linux). By package version, the same applies to Debian 12, Ubuntu 24.04 and Linux Mint 22, not yet tested. `dpkg -l libp11-kit0` shows the installed version. In 0.25.6, p11-kit changed how it sends the signing parameters, and older versions do not understand Tessera on that point. On those systems, use [distrobox-adv-br](https://github.com/pedrohqb/distrobox-adv-br).

Arch Linux, Manjaro, EndeavourOS and CachyOS:

```bash
sudo pacman -S flatpak pcsclite ccid
```

openSUSE Tumbleweed:

```bash
sudo zypper in flatpak pcsc-ccid p11-kit-server
```

Then turn on the smart card service and the user's p11-kit server. On Fedora, Arch, openSUSE and their derivatives:

```bash
sudo systemctl enable --now pcscd.socket
systemctl --user enable --now p11-kit-server.socket
```

On Debian, Ubuntu, Linux Mint and their derivatives, the p11-kit package ships the server but not `p11-kit-server.socket`, and the server cannot be socket-activated. It goes in as a plain user service, started at login:

```bash
sudo systemctl enable --now pcscd.socket
mkdir -p ~/.config/systemd/user
cat > ~/.config/systemd/user/p11-kit-server.service <<'EOF'
[Unit]
Description=p11-kit server
Documentation=man:p11-kit(8)

[Service]
ExecStartPre=/bin/mkdir -p -m 700 %t/p11-kit
ExecStart=/usr/bin/p11-kit server -f -n %t/p11-kit/pkcs11 pkcs11:
Restart=on-failure

[Install]
WantedBy=default.target
EOF
systemctl --user daemon-reload
systemctl --user enable --now p11-kit-server.service
```

The p11-kit server command is what lets apps see Tessera's tokens. Tessera's window offers it too, with a copy button (on Debian and Ubuntu, as a single line that does the same).

**Note:** on a Fedora system where one of these packages was removed, reinstall them with `sudo dnf install pcsc-lite pcsc-lite-ccid p11-kit-server`.

#### 2. Install Tessera

[![Get it on FlatPark](https://flatpark.org/badges/get-it-on-flatpark.svg)](https://flatpark.org/apps/io.github.pedrohqb.Tessera)

Tessera is distributed through [FlatPark](https://flatpark.org), a Flatpak remote. The base it runs on (the GNOME runtime) comes from Flathub, and Flatpak only downloads it from a remote in the **same installation** as Tessera: the user's (`--user`) or the system's (`--system`). So the commands below add both remotes; if one is already set up, `--if-not-exists` leaves it as it is.

To install for yourself only:

```bash
flatpak --user remote-add --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo
flatpak --user remote-add --if-not-exists flatpark https://dl.flatpark.org/flatpark.flatpakrepo
flatpak --user install flatpark io.github.pedrohqb.Tessera
```

During installation, Flatpak downloads the drivers from the vendors' official sites and checks each one against its SHA256. Updates come with `flatpak update`, like any other app.

To install for every user of the computer (a system-wide install, which asks for the administrator password):

```bash
flatpak --system remote-add --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo
flatpak --system remote-add --if-not-exists flatpark https://dl.flatpark.org/flatpark.flatpakrepo
flatpak --system install flatpark io.github.pedrohqb.Tessera
```

It works the same way: each user who opens Tessera gets their own instance, with their own tokens, and each app's step 1 is still per user.

**Note:** if you installed a test build before (the `tessera-test` remote), remove it first, keeping your settings: `flatpak --user uninstall io.github.pedrohqb.Tessera && flatpak --user remote-delete tessera-test`.

### Usage

Open **Tessera** from the app menu. With the token plugged in, it appears under **Tokens**, with its certificates. The SerproID certificate appears after you sign in through the SerproID app, with the button on the driver's row.

Under **Apps**, each app shows what is missing. There are at most two steps, done once:

- **Step 1 · access to the system's p11-kit server:** a command to run in a terminal, with a copy button. For Firefox, for example:
  ```bash
  flatpak override --user --filesystem=xdg-run/p11-kit/pkcs11 org.mozilla.firefox
  ```
  Then close the app and open it again.
- **Step 2 · certificate database:** the **Set up** button. Only Firefox, Papers and Okular have this step. For Firefox, close the browser before pressing the button.

| App | Steps |
|---|---|
| PJeOffice Pro | none: the p11-kit server from the installation is enough |
| Firefox | step 1 and step 2 |
| Chromium, Chrome, Edge and Brave | step 1 only |
| Papers and Okular | step 1 and step 2 |

When everything is right, the app shows as **Ready**, and the **System connection** row tells how many of Tessera's tokens the apps see.

The **Apps** section has two tabs: **Flatpak**, with the steps above, and **Native**, for the apps installed from the distribution itself (Firefox as an `.rpm` or `.deb`, say). Native apps need no system connection: they reach Tessera through `p11-kit-proxy.so`, and the only step is the **Set up** button, which adds it to the app's certificate database. Close the app first.

| Native app | Where Tessera sets it up |
|---|---|
| Firefox | every profile |
| Chromium, Chrome, Edge and Brave | the database they share (`~/.pki/nssdb` or `~/.local/share/pki/nssdb`) |
| Okular and Papers | no button of their own: they use the default Firefox profile's database, so they are ready once Firefox is set up |
| PJeOffice Pro | the list of A3 libraries (`list.a3`) in `~/.pjeoffice-pro/pjeoffice-pro.config`, with p11-kit's path on your distribution |

Before offering the button, Tessera checks whether the app already gets p11-kit without it. On **Fedora and its derivatives** (Bluefin, Aurora, Bazzite), the system itself loads p11-kit in every app that uses NSS: the native Firefox, Chrome, Chromium, Okular and Papers show as **Ready**, with no button. If Tessera's module was added earlier, each token shows twice, and the button becomes **Remove**, which takes out only Tessera's. On other distributions, a database that already has p11-kit, put there by anyone, also shows as **Ready**. PJeOffice Pro does not use NSS and always needs its own **Set up**.

#### In the background

- **Closing the window does not stop Tessera:** the tokens stay available, and a tray icon shows a summary. On GNOME, the icon needs the AppIndicator extension, which Bluefin and Aurora ship. To remove the icon, use the ☰ menu → **Show an icon in the tray**.
- **Quit**, from the menu or the tray, stops Tessera, and the tokens disappear from the apps.
- **To have Tessera start when you log in,** use the ☰ menu → **Start automatically at login**.

#### If something does not work

- **The "System connection" row is red:** run `systemctl --user enable --now p11-kit-server.socket`. It goes back to normal by itself in a few seconds.
- **An app does not see the token:** check that it shows as **Ready** in the list, and that it was opened again after step 1.
- **The token does not appear in Tessera:** unplug it and plug it in again. If a driver shows as "Stopped", press **Restart**.
- **A browser asks for the PIN twice:** see [Apps](docs/apps.md#chromium-google-chrome-microsoft-edge-and-brave).

More cases in [Troubleshooting](docs/troubleshooting.md).

### Uninstalling

Quit Tessera (☰ menu → **Quit**) and run:

```bash
flatpak uninstall --user --delete-data io.github.pedrohqb.Tessera
```

For a system-wide install, use `--system` instead of `--user`.

Each app's step 1 permission is undone with `flatpak override --user --reset <app>`. That command also removes any other permission you gave the app.

### Documentation

The technical documentation is in [docs/](docs/README.md):

- **[Architecture](docs/architecture.md):** how a token leaves Tessera's sandbox and reaches the apps, and why there is no simpler way.
- **[Drivers](docs/drivers.md):** where each driver comes from, how it is checked, and what had to be adjusted, above all for SerproID.
- **[Apps](docs/apps.md):** what each app needs, and why.
- **[Installation and usage](docs/usage.md):** the window, the tray and the command line.
- **[Compatibility](docs/compatibility.md):** what is checked automatically on every change, and what has been tested by hand with real tokens. To add your test, open an issue with the "Compatibility report" template.
- **[Troubleshooting](docs/troubleshooting.md):** logs, test tools and how to investigate a problem.
- **[History](docs/history.md):** the problems found while testing with real tokens, and how each was solved.
- **[Development](docs/development.md):** building, tests, releases and how to add a driver.

### Support

If you have any trouble, open an [issue](https://github.com/pedrohqb/tessera/issues) or ask in the [Telegram](https://t.me/advogados_linux) group (in Portuguese).

### License

GPL-3.0-or-later. The vendor drivers are not part of Tessera: they are downloaded from their vendors at installation and are covered by the vendors' own licenses.
