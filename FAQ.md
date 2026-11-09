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
- [O plugin Web PKI ou WebSigner não mostra o meu token.](#o-plugin-web-pki-ou-websigner-não-mostra-o-meu-token)
- [A linha "Conexão com o sistema" está vermelha.](#a-linha-conexão-com-o-sistema-está-vermelha)
- [O aplicativo não vê o token.](#o-aplicativo-não-vê-o-token)
- [O token não aparece no Tessera.](#o-token-não-aparece-no-tessera)
- [O token aparece duas vezes, ou o navegador pede o PIN duas vezes.](#o-token-aparece-duas-vezes-ou-o-navegador-pede-o-pin-duas-vezes)
- [Eu usava o SafeSign, o SafeNet ou o SerproID instalado no sistema. Preciso fazer algo?](#eu-usava-o-safesign-o-safenet-ou-o-serproid-instalado-no-sistema-preciso-fazer-algo)
- [O JSignPdf não acha mais o certificado.](#o-jsignpdf-não-acha-mais-o-certificado)
- [Como troco ou desbloqueio o PIN de um token do OpenSC (ePass2003)?](#como-troco-ou-desbloqueio-o-pin-de-um-token-do-opensc-epass2003)
- [Meu token GD Burti (ou cartão SafeSign) trava ou some.](#meu-token-gd-burti-ou-cartão-safesign-trava-ou-some)
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

Não mais, quando as extensões de assinador web do Tessera chegarem ao FlatPark: o **Web PKI**, o **WebSigner** da Softplan e o da Certisign passam a ser extensões do Tessera, e a janela mostra o comando de cada navegador em Flatpak (veja o [README](README.md#web-pki-e-websigner)). Até lá, o Sigillum serve para o Web PKI e o WebSigner da Softplan. Nos navegadores nativos, nenhum dos dois é preciso: use os pacotes oficiais dos fornecedores e acrescente o caminho do p11-kit no plugin. No Arch e derivados, `/lib/libp11-kit.so` também funciona: é a mesma biblioteca que `/usr/lib/p11-kit-proxy.so`.

### O plugin Web PKI ou WebSigner não mostra o meu token.

Nos navegadores em Flatpak, confira, nesta ordem: o passo 1 do navegador (seção Aplicativos) está feito; a linha do navegador em **Assinadores web** mostra o plugin como feito; o navegador foi reiniciado depois do comando; e, nas configurações do plugin, **"Dispositivos SafeSign AET"** (aba "Cripto Dispositivos") está marcado; no WebSigner da Certisign, a opção é **"SafeSign"**. Essa opção vale para qualquer token do Tessera, não só para o SafeSign: é o nome que o plugin usa para carregar o p11-kit. Não marque também "Tokens SafeNet", senão cada token aparece duas vezes.

### A linha "Conexão com o sistema" está vermelha.

O servidor p11-kit do sistema está desligado, e os aplicativos em Flatpak não veem os tokens. Rode o comando do servidor da sua distribuição (passo 1 da instalação), ou copie-o pelo botão da própria linha. Ela volta ao normal sozinha em alguns segundos.

### O aplicativo não vê o token.

Confira se ele aparece como **Pronto** na lista do Tessera e se foi aberto de novo depois do passo 1. O aplicativo precisa estar fechado quando você aperta **Configurar**. Se o passo 2 pedir para abrir o aplicativo e reiniciar o Tessera, é porque o aplicativo foi aberto pela primeira vez depois que o Tessera abriu, e o Tessera ainda não enxerga a pasta dele: use o botão **Reiniciar o Tessera** da linha. Se o PIN é pedido mas a assinatura falha, veja a [primeira pergunta](#o-pin-é-pedido-mas-a-assinatura-falha-por-quê).

### O token não aparece no Tessera.

Veja primeiro se o driver dele está instalado: os drivers dos fabricantes são extensões à parte (tabela no [README](README.md)). Com o token conectado, se faltar o driver, a linha do leitor diz qual instalar, com o comando pronto para copiar; depois de instalar, use **Reiniciar o Tessera** no menu. Se o driver já estiver lá, tire e ponha o token. Se um driver aparecer como "Parado", aperte **Reiniciar**. Confira também se o serviço de cartões está ligado (`sudo systemctl enable --now pcscd.socket`).

### O token aparece duas vezes, ou o navegador pede o PIN duas vezes.

No **Fedora e derivados**, o próprio sistema já carrega o p11-kit nos aplicativos nativos. Se o módulo do Tessera também foi acrescentado, cada token aparece duas vezes, e o botão do aplicativo vira **Retirar**, que tira só o do Tessera. Nos navegadores Chromium em Flatpak, veja [Apps](docs/apps.md#chromium-google-chrome-microsoft-edge-and-brave) (em inglês).

O token também aparece duas vezes quando **dois drivers atendem o mesmo token**, e o Tessera avisa na janela:

- **Dois drivers do Tessera** (o OpenSC e o do fornecedor): desligue um deles na lista de drivers. Com o SafeNet ligado, o OpenSC já deixa para ele os cartões IDPrime e o eToken 5110.
- **O sistema também tem o OpenSC** (o Fedora e o Bluefin o trazem instalado): o token do OpenSC chega aos aplicativos pelo do sistema e pelo do Tessera. Desligue o OpenSC no Tessera; o do sistema basta.

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

### O JSignPdf diz "Nenhuma chave privada foi encontrada".

No painel **Certificado**, à esquerda, o **Tipo de keystore** deve ser **PKCS11** (o **Configurar** do Tessera já o escolhe; feche o JSignPdf antes). Digite o PIN do token em **Palavra-passe do keystore**, aperte **Carregar chaves**, escolha o certificado em **Alias da chave** e assine; deixe vazios **Ficheiro de keystore** e **Palavra-passe da chave**. Sem o PIN, o token não mostra a chave. O JSignPdf abre um token por vez: com mais de um conectado, escolha qual na lista da linha do JSignPdf, no Tessera, e aperte **Configurar** (com o JSignPdf fechado).

### O JSignPdf não acha mais o certificado.

O JSignPdf abre um único slot do servidor p11-kit, e o Tessera grava a posição do slot do token quando você aperta **Configurar**. Se o token mudar de leitora, ou se o sistema ganhar ou perder algum módulo, essa posição muda. Feche o JSignPdf, conecte o token e aperte **Configurar** de novo na linha dele.

### Como troco ou desbloqueio o PIN de um token do OpenSC (ePass2003)?

O OpenSC não tem programa com janela para isso; as ferramentas de linha de comando dele vêm no Tessera. Num terminal, com o token conectado:

```bash
flatpak run io.github.pedrohqb.Tessera opensc pkcs15-tool --change-pin
flatpak run io.github.pedrohqb.Tessera opensc pkcs15-tool --unblock-pin
```

O primeiro pede o PIN atual e o novo; o segundo, o PUK e o novo PIN. `opensc pkcs15-tool --list-certificates` lista os certificados, e `opensc opensc-tool --list-readers`, as leitoras. Erre o PIN com cuidado: depois de algumas tentativas o token bloqueia, e sem o PUK só a autoridade certificadora resolve.

### Meu token GD Burti (ou cartão SafeSign) trava ou some.

O OpenSC não sabe ler o token GD Burti e, quando tenta, o token trava, também para o SafeSign. O OpenSC do Tessera já deixa de lado os tokens GD Burti (os leitores "StarSign"). Se você usa um **cartão** SafeSign numa leitora comum, ou o problema continuar, desligue o OpenSC na lista de drivers da janela e tire e ponha o token. No sistema, o mesmo vale para o OpenSC instalado pela distribuição (o Fedora e o Bluefin o trazem).

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

Numa instalação de sistema, troque `--user` por `--system`. As extensões de driver saem junto com o Tessera. O Flatpak lista cada uma duas vezes e, na segunda, avisa que ela não está instalada (um **[✗]** na lista): é um defeito do próprio Flatpak, inofensivo, porque a primeira linha já a removeu. Se `flatpak list | grep Tessera.Driver` ainda mostrar alguma, remova com `flatpak uninstall --user io.github.pedrohqb.Tessera.Driver.SerproID` (o nome que aparecer). A permissão do passo 1 de cada aplicativo é desfeita com `flatpak override --user --reset <aplicativo>`, que também apaga outras permissões que você tenha dado ao aplicativo.

O Tessera desliga o OpenSC do sistema com dois arquivos que continuam lá depois de desinstalar. Para religá-lo, desligue antes a chave **Desligar o OpenSC do sistema**, na seção Serviço, ou apague os arquivos depois:

```bash
rm -f ~/.config/pkcs11/modules/opensc.module ~/.config/pkcs11/modules/opensc-pkcs11.module
```

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
- [The Web PKI or WebSigner plugin does not show my token.](#the-web-pki-or-websigner-plugin-does-not-show-my-token)
- [The "System connection" row is red.](#the-system-connection-row-is-red)
- [An app does not see the token.](#an-app-does-not-see-the-token)
- [The token does not appear in Tessera.](#the-token-does-not-appear-in-tessera)
- [The token shows twice, or the browser asks for the PIN twice.](#the-token-shows-twice-or-the-browser-asks-for-the-pin-twice)
- [I used SafeSign, SafeNet or SerproID installed on the system. Do I need to do anything?](#i-used-safesign-safenet-or-serproid-installed-on-the-system-do-i-need-to-do-anything)
- [JSignPdf no longer finds the certificate.](#jsignpdf-no-longer-finds-the-certificate)
- [How do I change or unblock the PIN of an OpenSC token (ePass2003)?](#how-do-i-change-or-unblock-the-pin-of-an-opensc-token-epass2003)
- [My GD Burti token (or SafeSign card) hangs or disappears.](#my-gd-burti-token-or-safesign-card-hangs-or-disappears)
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

Not anymore, once Tessera's web signer extensions reach FlatPark: **Web PKI**, Softplan's **WebSigner** and Certisign's become extensions of Tessera, and the window shows the command for each Flatpak browser (see the [README](README.md#web-pki-and-websigner)). Until then, Sigillum covers Web PKI and Softplan's WebSigner. In native browsers neither is needed: use the vendors' official packages and add the p11-kit path in the plugin. On Arch and its derivatives, `/lib/libp11-kit.so` also works: it is the same library as `/usr/lib/p11-kit-proxy.so`.

### The Web PKI or WebSigner plugin does not show my token.

In Flatpak browsers, check, in this order: the browser's step 1 (Apps section) is done; the browser's row under **Web signers** shows the plugin as done; the browser was restarted after the command; and, in the plugin's settings, **"Dispositivos SafeSign AET"** ("Cripto Dispositivos" tab) is ticked; in Certisign's WebSigner the option is **"SafeSign"**. That option works for any of Tessera's tokens, not only SafeSign: it is the name the plugin uses to load p11-kit. Do not tick "Tokens SafeNet" as well, or each token shows twice.

### The "System connection" row is red.

The system's p11-kit server is off, and Flatpak apps do not see the tokens. Run your distribution's server command (installation step 1), or copy it with the row's own button. The row goes back to normal by itself in a few seconds.

### An app does not see the token.

Check that it shows as **Ready** in Tessera's list, and that it was opened again after step 1. The app must be closed when you press **Set up**. If step 2 asks you to open the app and restart Tessera, the app was first opened after Tessera started, and Tessera does not see its folder yet: use the row's **Restart Tessera** button. If the PIN is asked but signing fails, see the [first question](#the-pin-is-asked-but-signing-fails-why).

### The token does not appear in Tessera.

First check that its driver is installed: the vendors' drivers are separate extensions (table in the [README](README.md)). With the token plugged in, if its driver is missing, the reader's row says which one to install, with the command ready to copy; after installing, use **Restart Tessera** in the menu. If the driver is there, unplug the token and plug it in again. If a driver shows as "Stopped", press **Restart**. Also check that the smart card service is on (`sudo systemctl enable --now pcscd.socket`).

### The token shows twice, or the browser asks for the PIN twice.

On **Fedora and its derivatives**, the system itself already loads p11-kit in native apps. If Tessera's module was added too, each token shows twice, and the app's button becomes **Remove**, which takes out only Tessera's. For Chromium-based browsers as Flatpaks, see [Apps](docs/apps.md#chromium-google-chrome-microsoft-edge-and-brave).

A token also shows twice when **two drivers serve it**, and Tessera says so in its window:

- **Two of Tessera's drivers** (OpenSC and the vendor's): switch one off in the list of drivers. While SafeNet is on, OpenSC already leaves IDPrime cards and the eToken 5110 to it.
- **The system has OpenSC too** (Fedora and Bluefin ship it installed): OpenSC's token reaches the apps through the system's and through Tessera's. Switch OpenSC off in Tessera; the system's is enough.

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

### JSignPdf says "No private key was found".

In the **Certificate** panel, on the left, **Keystore type** must be **PKCS11** (Tessera's **Set up** chooses it; close JSignPdf first). Type the token's PIN in **Keystore password**, press **Load keys**, pick the certificate under **Key alias** and sign; leave **Keystore file** and **Key password** empty. Without the PIN, the token does not show its key. JSignPdf opens one token at a time: with more than one plugged in, pick which in the list on JSignPdf's row in Tessera, and press **Set up** (with JSignPdf closed).

### JSignPdf no longer finds the certificate.

JSignPdf opens a single slot of the p11-kit server, and Tessera writes the position of the token's slot when you press **Set up**. If the token moves to another reader, or the system gains or loses a module, that position changes. Close JSignPdf, plug in the token and press **Set up** again on its row.

### How do I change or unblock the PIN of an OpenSC token (ePass2003)?

OpenSC has no windowed program for that; its command-line tools come with Tessera. In a terminal, with the token plugged in:

```bash
flatpak run io.github.pedrohqb.Tessera opensc pkcs15-tool --change-pin
flatpak run io.github.pedrohqb.Tessera opensc pkcs15-tool --unblock-pin
```

The first asks for the current PIN and the new one; the second, for the PUK and the new PIN. `opensc pkcs15-tool --list-certificates` lists the certificates, and `opensc opensc-tool --list-readers` the readers. Be careful with wrong PINs: after a few tries the token locks, and without the PUK only the certificate authority can help.

### My GD Burti token (or SafeSign card) hangs or disappears.

OpenSC cannot read GD Burti tokens, and when it tries, the token hangs, for SafeSign too. Tessera's OpenSC already leaves GD Burti tokens alone (the "StarSign" readers). If you use a SafeSign **card** in an ordinary reader, or the problem persists, switch OpenSC off in the window's list of drivers and unplug the token and plug it in again. The same goes for an OpenSC installed by the distribution (Fedora and Bluefin ship it).

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

For a system-wide install, use `--system` instead of `--user`. The driver extensions go with it. Flatpak lists each one twice and, the second time, warns that it is not installed (a **[✗]** in the list): a harmless defect of Flatpak itself, since the first line already removed it. If `flatpak list | grep Tessera.Driver` still shows one, remove it with `flatpak uninstall --user io.github.pedrohqb.Tessera.Driver.SerproID` (the name shown). Each app's step 1 permission is undone with `flatpak override --user --reset <app>`, which also removes any other permission you gave the app.

Tessera switches the system's OpenSC off with two files that stay after uninstalling. To turn it back on, first switch off **Switch off the system's OpenSC**, in the Service section, or delete the files afterwards:

```bash
rm -f ~/.config/pkcs11/modules/opensc.module ~/.config/pkcs11/modules/opensc-pkcs11.module
```

### Is there another "Tessera" on Flathub?

Yes, a QR code generator (`fr.ethal.tessera`), unrelated to this project. Flatpak tells apps apart by their ID, and this one's is `io.github.pedrohqb.Tessera`: the two do not mix.

### How do I report that it works (or not) on my system?

Open an [issue](https://github.com/pedrohqb/tessera/issues/new/choose) with the **Compatibility report** template, or tell the [Telegram](https://t.me/advogados_linux) group: distribution, token or certificate, app (Flatpak or native) and what happened. What has been tested is in [Compatibility](docs/compatibility.md).
