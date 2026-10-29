# Aplicativos

Todo aplicativo Flatpak precisa de duas coisas para usar os tokens do Tessera:

1. **Chegar ao servidor p11-kit do sistema:** a permissão `--filesystem=xdg-run/p11-kit/pkcs11`. A janela do Tessera mostra o comando com um botão de copiar e marca ✓ quando o `override` já existe. Depois do comando, é preciso fechar e abrir o aplicativo de novo.
2. **Carregar o `p11-kit-client.so` do runtime:** `/usr/lib/x86_64-linux-gnu/pkcs11/p11-kit-client.so`. Como isso é feito muda de aplicativo para aplicativo, e é o que o botão **Configurar** resolve.

Antes de tudo, o servidor do sistema precisa estar ligado, uma vez por usuário: `systemctl --user enable --now p11-kit-server.socket`. A linha "Conexão com o sistema" da janela confere isso de ponta a ponta: pelo soquete do sistema, o Tessera lista os tokens que um aplicativo enxergaria e procura os seus (pelo nome e número de série). Fica vermelha se ninguém responde ou se os tokens do Tessera não estão lá. Só ver o soquete não basta: com o servidor do sistema desligado, o servidor de confiança do próprio Flatpak responde no mesmo caminho, e foi isso que enganou a primeira versão dessa linha. A conferência se repete a cada 30 s com a janela aberta e sempre que a lista de tokens muda; `tessera status` mostra o mesmo resultado no terminal.

## PJeOffice Pro (FlatPark)

`br.jus.cnj.PJeOfficePro`. **Nada a fazer.** O pacote do FlatPark já tem `xdg-run/p11-kit/pkcs11` e carrega o `p11-kit-client.so`. Basta o servidor do sistema ligado. Uma observação: um PJeOffice aberto antes do Tessera não vê os drivers publicados depois; é só abri-lo de novo.

## Firefox

`org.mozilla.firefox`.

1. `flatpak override --user --filesystem=xdg-run/p11-kit/pkcs11 org.mozilla.firefox`
2. Feche o Firefox e aperte **Configurar**. O Tessera lê o `profiles.ini` (em `~/.var/app/org.mozilla.firefox/config/mozilla/firefox` no Firefox 147 em diante, ou em `.mozilla/firefox` nos anteriores) e registra o módulo "Tessera (p11-kit)" no banco NSS de **todos** os perfis. Com o Firefox aberto, o perfil está trancado (uma trava `fcntl` no arquivo `.parentlock`; o link `lock` pode sobrar de um Firefox que fechou mal e não conta), e o Tessera avisa em vez de mexer.

Também dá para fazer à mão: Configurações → Privacidade e Segurança → Dispositivos de segurança → Carregar, arquivo `/usr/lib/x86_64-linux-gnu/pkcs11/p11-kit-client.so`.

No Gerenciador de dispositivos, aparece um módulo com os slots do SerproID, do SafeSign e da confiança do sistema. O "Módulo integrado de raízes" é o conjunto de raízes que vem com o próprio Firefox, não o p11-kit, então não há duplicata.

## Chromium, Google Chrome, Microsoft Edge e Brave

`org.chromium.Chromium`, `com.google.Chrome`, `com.microsoft.Edge`, `com.brave.Browser`. **Basta o passo 1.** Ao iniciar o NSS, o Chromium carrega sozinho o módulo de certificados raiz, `libnssckbi.so` (`LoadNSSModule("Root Certs", "libnssckbi.so")` em `crypto/nss_util.cc`). No runtime do Flatpak, esse arquivo é o p11-kit, que com o `override` chega ao servidor do sistema e, portanto, aos tokens do Tessera. É o mesmo caminho do Papers.

Na primeira versão, o **Configurar** registrava também o `p11-kit-client.so` no banco NSS do navegador. Cada token passou a aparecer duas vezes, e o navegador pedia o PIN do SafeSign uma vez por cópia: duas no Chrome, no Edge e no Brave, três no Chromium (cujo `~/.pki/nssdb` real pode ter ainda outro registro). Agora o **Configurar** só retira esse registro, se existir. O passo 2 aparece como pronto quando não há nada a retirar.

Onde fica o banco ([documentação do Chromium](https://chromium.googlesource.com/chromium/src/+/main/docs/linux/cert_management.md)): desde o Chromium 146, em `$XDG_DATA_HOME/pki/nssdb`, que no Flatpak é `~/.var/app/<navegador>/data/pki/nssdb`. Se existir o banco antigo, `~/.pki/nssdb`, ele vence. O Tessera segue a mesma regra:

| Navegador | Banco antigo (vence, se existir) | Banco novo |
|---|---|---|
| Chromium | o `~/.pki/nssdb` real (o pacote pode usar a pasta pessoal) | `~/.var/app/org.chromium.Chromium/data/pki/nssdb` |
| Google Chrome | `~/.var/app/com.google.Chrome/.pki/nssdb` (só com `--persist=.pki`) | `~/.var/app/com.google.Chrome/data/pki/nssdb` |
| Microsoft Edge | `~/.var/app/com.microsoft.Edge/.pki/nssdb` (o pacote tem `--persist=.pki`) | `~/.var/app/com.microsoft.Edge/data/pki/nssdb` |
| Brave | `~/.var/app/com.brave.Browser/.pki/nssdb` (o pacote tem `--persist=.pki`) | `~/.var/app/com.brave.Browser/data/pki/nssdb` |

No teste de 29 de outubro de 2026, o Chrome, o Edge e o Brave usavam o banco novo. A primeira versão do Tessera procurava só o antigo e dizia que não havia banco.

O Tessera precisa dessas pastas só para retirar o registro antigo. O Flatpak só mostra a ele as pastas que já existiam quando ele abriu.

## Papers

`org.gnome.Papers`. O Papers assina PDFs pelo poppler, que procura o banco NSS nesta ordem: o perfil `*default*` mais recente com `cert9.db` em `$XDG_CONFIG_HOME/mozilla/firefox` (no Flatpak, `~/.var/app/org.gnome.Papers/config/mozilla/firefox`), depois `~/.mozilla/firefox`, depois `/etc/pkcs11/nssdb`, depois `~/.pki/nssdb`.

1. `flatpak override --user --filesystem=xdg-run/p11-kit/pkcs11 org.gnome.Papers`
2. **Configurar** cria um banco NSS **vazio** em `~/.var/app/org.gnome.Papers/config/mozilla/firefox/tessera.default`.

O banco do Papers fica vazio porque, quando o NSS abre um banco, carrega sozinho o módulo de certificados raiz (`libnssckbi.so`). No runtime do Flatpak, esse arquivo é o `load-p11-kit-trust.so`, e o módulo de confiança do p11-kit, com o `override`, aponta para o servidor do sistema. Ou seja, o Papers já chega aos tokens por esse caminho. Com o `p11-kit-client.so` registrado também, cada certificado aparecia **duas vezes** na janela de assinatura. O banco próprio serve para o poppler não cair no `~/.pki/nssdb`, que lista o módulo de raízes mais uma vez. Se uma versão anterior do Tessera registrou o módulo nesse banco, o **Configurar** o retira.

Num teste, o reset das permissões do Papers no Flatseal apagou o `override`, e nenhum certificado apareceu mais. Basta refazer o passo 1.

## Okular

`org.kde.okular`. O Okular também assina pelo poppler, então recebe o mesmo tratamento do Papers:

1. `flatpak override --user --filesystem=xdg-run/p11-kit/pkcs11 org.kde.okular`. O pacote tem `--filesystem=host`, mas isso não inclui o soquete do p11-kit.
2. **Configurar** cria o banco NSS vazio em `~/.var/app/org.kde.okular/config/mozilla/firefox/tessera.default`, o primeiro lugar em que o poppler procura (o `$XDG_CONFIG_HOME` do Okular no Flatpak). Em Configurações → Configurar o Okular → Assinaturas digitais, o banco de certificados deve ficar no padrão.

O Okular usa o runtime do KDE, que tem a mesma base freedesktop do runtime do GNOME e o mesmo `libnssckbi.so` do p11-kit. Ainda não foi testado com token real.

## Aplicativos nativos

Um aplicativo instalado no sistema (não Flatpak) que usa o `p11-kit-proxy.so` vê os tokens do Tessera sem configuração: o proxy lê `~/.config/pkcs11/modules`.

## Para quem mantém um pacote Flatpak

O suficiente para um aplicativo usar os tokens do Tessera, sem depender dele:

```yaml
finish-args:
  - --filesystem=xdg-run/p11-kit/pkcs11
```

e carregar `/usr/lib/x86_64-linux-gnu/pkcs11/p11-kit-client.so` (ou usar o NSS do runtime, que o carrega pelo módulo de raízes). É o que o PJeOffice Pro do FlatPark faz.
