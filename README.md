# Tessera

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

## Instalação

### 1. Preparar o sistema (uma vez)

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

### 2. Instalar o Tessera

```bash
mkdir -p ~/Downloads/tessera && cd ~/Downloads/tessera && \
curl -fLO https://github.com/pedrohqb/tessera/releases/latest/download/tessera-repo.tar.gz && \
tar -xzf tessera-repo.tar.gz && ./install-test-repo.sh
```

Na instalação, o Flatpak baixa os drivers dos sites oficiais dos fornecedores e confere cada um pelo SHA256. Para atualizar, rode os mesmos comandos com a versão nova.

---

## Uso

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

### Em segundo plano

- **Fechar a janela não desliga o Tessera:** os tokens continuam disponíveis, e um ícone na bandeja mostra o resumo. No GNOME, o ícone depende da extensão AppIndicator, que o Bluefin e o Aurora já trazem. Para tirar o ícone, use o menu ☰ → **Mostrar ícone na bandeja**.
- **Sair**, pelo menu ou pela bandeja, desliga o Tessera e os tokens deixam de aparecer nos aplicativos.
- **Para o Tessera abrir sozinho ao entrar na sessão,** use o menu ☰ → **Iniciar automaticamente no login**.

### Se algo não funcionar

- **A linha "Conexão com o sistema" está vermelha:** rode `systemctl --user enable --now p11-kit-server.socket`. Ela volta ao normal sozinha em alguns segundos.
- **O aplicativo não vê o token:** confira se ele está como **Pronto** na lista e se foi aberto de novo depois do passo 1.
- **O token não aparece no Tessera:** tire e ponha o token. Se um driver aparecer como "Parado", aperte **Reiniciar**.
- **Um navegador pede o PIN duas vezes:** veja [Aplicativos](docs/aplicativos.md#chromium-google-chrome-microsoft-edge-e-brave).

Mais casos em [Diagnóstico](docs/diagnostico.md).

---

## Desinstalação

Saia do Tessera (menu ☰ → **Sair**) e rode:

```bash
flatpak uninstall --user --delete-data io.github.pedrohqb.Tessera
flatpak remote-delete --user tessera-test
```

A permissão do passo 1 de cada aplicativo é desfeita com `flatpak override --user --reset <aplicativo>`. Esse comando também apaga outras permissões que você tenha dado ao aplicativo.

---

## Documentação

A documentação técnica está em [docs/](docs/README.md):

- **[Arquitetura](docs/arquitetura.md):** como o token sai do sandbox do Tessera e chega aos aplicativos, e por que não há caminho mais simples.
- **[Drivers](docs/drivers.md):** origem e verificação de cada driver, e o que foi preciso ajustar, sobretudo no SerproID.
- **[Aplicativos](docs/aplicativos.md):** o que cada aplicativo precisa e por quê.
- **[Instalação e uso](docs/uso.md):** a janela, a bandeja e a linha de comando.
- **[Diagnóstico](docs/diagnostico.md):** logs, ferramentas de teste e como investigar um problema.
- **[Histórico](docs/historico.md):** os problemas encontrados nos testes com tokens reais e como cada um foi resolvido.
- **[Desenvolvimento](docs/desenvolvimento.md):** compilação, testes, versões e como acrescentar um driver.

---

## Suporte

Havendo qualquer dificuldade, abra uma [issue](https://github.com/pedrohqb/tessera/issues) ou peça ajuda no grupo oficial no [Telegram](https://t.me/advogados_linux).

---

## In English

Tessera is a PKCS#11 token hub for Flatpak. It runs the SafeSign, SafeNet and SerproID drivers inside its own sandbox and shares the tokens with sandboxed apps (PJeOffice Pro, Firefox, Chromium-based browsers, Papers, Okular) through the host's p11-kit server, with no vendor packages on the host. The documentation, in Portuguese, starts at [docs/](docs/README.md); [Architecture](docs/arquitetura.md) explains the design.

## Licença

GPL-3.0-or-later. Os drivers dos fornecedores não fazem parte do Tessera: são baixados dos fornecedores na instalação e seguem as licenças deles.
