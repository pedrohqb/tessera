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

Para o **Lacuna Web PKI** e o **Softplan WebSigner**, exigidos por alguns tribunais, como o e-SAJ, use também o [**Sigillum**](https://github.com/pedrohqb/sigillum), que os instala nos navegadores e os liga aos tokens do Tessera.

![O Tessera com o SafeSign e o SerproID](data/screenshots/main.png)

---

### Instalação

#### 1. Preparar o sistema (uma vez)

O sistema precisa de três coisas: o Flatpak com o Flathub, o serviço de cartões (`pcscd`) com o driver CCID, e o servidor p11-kit do usuário. O **Fedora** (tradicional e atômico), o **Bluefin**, o **Aurora** e o **Bazzite** já vêm com os pacotes; basta ligar os serviços, mais abaixo.

Debian, Ubuntu, Linux Mint e derivados:

```bash
sudo apt install flatpak pcscd libccid
```

Arch Linux, Manjaro, EndeavourOS e CachyOS:

```bash
sudo pacman -S flatpak pcsclite ccid
```

openSUSE Tumbleweed:

```bash
sudo zypper in flatpak pcsc-ccid
```

Depois, em qualquer distribuição, ligue o serviço de cartões e o servidor p11-kit do usuário:

```bash
sudo systemctl enable --now pcscd.socket
systemctl --user enable --now p11-kit-server.socket
```

O segundo comando é o que deixa os aplicativos verem os tokens do Tessera. A janela do Tessera também o mostra, com um botão de copiar.

**Observação:** num Fedora em que algum desses pacotes tenha sido removido, reinstale-os com `sudo dnf install pcsc-lite pcsc-lite-ccid p11-kit-server`.

**Observação:** se o Flathub ainda não estiver configurado, rode:

```bash
flatpak remote-add --user --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo
```

#### 2. Instalar o Tessera

[![Get it on FlatPark](https://flatpark.org/badges/get-it-on-flatpark.svg)](https://flatpark.org/apps/io.github.pedrohqb.Tessera)

O Tessera é distribuído pelo [FlatPark](https://flatpark.org), um repositório de Flatpaks. Adicione o repositório (uma vez) e instale:

```bash
flatpak --user remote-add --if-not-exists flatpark https://dl.flatpark.org/flatpark.flatpakrepo
flatpak --user install flatpark io.github.pedrohqb.Tessera
```

Na instalação, o Flatpak baixa os drivers dos sites oficiais dos fornecedores e confere cada um pelo SHA256. As atualizações chegam com o `flatpak update`, como as dos outros aplicativos.

Para instalar para todos os usuários do computador (instalação de sistema, que pede a senha de administrador), use `--system` no lugar de `--user`:

```bash
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

### A fazer

- Separar o `src/ui.rs` (a janela, a supervisão dos drivers e a bandeja estão no mesmo arquivo, com cerca de 1.200 linhas) em três módulos, sem mudar o comportamento. Os testes automáticos (`cargo test`) servem de rede de proteção.

### Documentação

A documentação técnica, em inglês, está em [docs/](docs/README.md):

- **[Architecture](docs/architecture.md):** como o token sai do sandbox do Tessera e chega aos aplicativos, e por que não há caminho mais simples.
- **[Drivers](docs/drivers.md):** origem e verificação de cada driver, e o que foi preciso ajustar, sobretudo no SerproID.
- **[Apps](docs/apps.md):** o que cada aplicativo precisa e por quê.
- **[Installation and usage](docs/usage.md):** a janela, a bandeja e a linha de comando.
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

For **Lacuna Web PKI** and **Softplan WebSigner**, required by some courts (e-SAJ, for example), also use [**Sigillum**](https://github.com/pedrohqb/sigillum), which installs them in the browsers and connects them to Tessera's tokens.

### Installation

#### 1. Prepare the system (once)

The system needs three things: Flatpak with Flathub, the smart card service (`pcscd`) with the CCID driver, and the user's p11-kit server. **Fedora** (traditional and atomic), **Bluefin**, **Aurora** and **Bazzite** already ship the packages; just turn on the services, below.

Debian, Ubuntu, Linux Mint and derivatives:

```bash
sudo apt install flatpak pcscd libccid
```

Arch Linux, Manjaro, EndeavourOS and CachyOS:

```bash
sudo pacman -S flatpak pcsclite ccid
```

openSUSE Tumbleweed:

```bash
sudo zypper in flatpak pcsc-ccid
```

Then, on any distribution, turn on the smart card service and the user's p11-kit server:

```bash
sudo systemctl enable --now pcscd.socket
systemctl --user enable --now p11-kit-server.socket
```

The second command is what lets apps see Tessera's tokens. Tessera's window shows it too, with a copy button.

**Note:** on a Fedora system where one of these packages was removed, reinstall them with `sudo dnf install pcsc-lite pcsc-lite-ccid p11-kit-server`.

**Note:** if Flathub is not set up yet, run:

```bash
flatpak remote-add --user --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo
```

#### 2. Install Tessera

[![Get it on FlatPark](https://flatpark.org/badges/get-it-on-flatpark.svg)](https://flatpark.org/apps/io.github.pedrohqb.Tessera)

Tessera is distributed through [FlatPark](https://flatpark.org), a Flatpak remote. Add the remote (once) and install:

```bash
flatpak --user remote-add --if-not-exists flatpark https://dl.flatpark.org/flatpark.flatpakrepo
flatpak --user install flatpark io.github.pedrohqb.Tessera
```

During installation, Flatpak downloads the drivers from the vendors' official sites and checks each one against its SHA256. Updates come with `flatpak update`, like any other app.

To install for every user of the computer (a system-wide install, which asks for the administrator password), use `--system` instead of `--user`:

```bash
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

### To do

- Split `src/ui.rs` (the window, the driver supervision and the tray share one file of about 1,200 lines) into three modules, without changing behaviour. The automated tests (`cargo test`) are the safety net.

### Documentation

The technical documentation is in [docs/](docs/README.md):

- **[Architecture](docs/architecture.md):** how a token leaves Tessera's sandbox and reaches the apps, and why there is no simpler way.
- **[Drivers](docs/drivers.md):** where each driver comes from, how it is checked, and what had to be adjusted, above all for SerproID.
- **[Apps](docs/apps.md):** what each app needs, and why.
- **[Installation and usage](docs/usage.md):** the window, the tray and the command line.
- **[Troubleshooting](docs/troubleshooting.md):** logs, test tools and how to investigate a problem.
- **[History](docs/history.md):** the problems found while testing with real tokens, and how each was solved.
- **[Development](docs/development.md):** building, tests, releases and how to add a driver.

### Support

If you have any trouble, open an [issue](https://github.com/pedrohqb/tessera/issues) or ask in the [Telegram](https://t.me/advogados_linux) group (in Portuguese).

### License

GPL-3.0-or-later. The vendor drivers are not part of Tessera: they are downloaded from their vendors at installation and are covered by the vendors' own licenses.
