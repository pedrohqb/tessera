# Aplicativos

Todo aplicativo Flatpak precisa de duas coisas para usar os tokens do Tessera:

1. **Chegar ao servidor p11-kit do sistema:** a permissão `--filesystem=xdg-run/p11-kit/pkcs11`. A janela do Tessera mostra o comando com um botão de copiar e marca ✓ quando o `override` já existe. Depois do comando, é preciso fechar e abrir o aplicativo de novo.
2. **Carregar o `p11-kit-client.so` do runtime:** `/usr/lib/x86_64-linux-gnu/pkcs11/p11-kit-client.so`. Como isso é feito muda de aplicativo para aplicativo, e é o que o botão **Configurar** resolve.

Antes de tudo, o servidor do sistema precisa estar ligado, uma vez por usuário: `systemctl --user enable --now p11-kit-server.socket`. A linha "Conexão com o sistema" da janela fica vermelha enquanto ele não responde.

## PJeOffice Pro (FlatPark)

`br.jus.cnj.PJeOfficePro`. **Nada a fazer.** O pacote do FlatPark já tem `xdg-run/p11-kit/pkcs11` e carrega o `p11-kit-client.so`. Basta o servidor do sistema ligado. Uma observação: um PJeOffice aberto antes do Tessera não vê os drivers publicados depois; é só abri-lo de novo.

## Firefox

`org.mozilla.firefox`.

1. `flatpak override --user --filesystem=xdg-run/p11-kit/pkcs11 org.mozilla.firefox`
2. Feche o Firefox e aperte **Configurar**. O Tessera lê o `profiles.ini` (em `~/.var/app/org.mozilla.firefox/config/mozilla/firefox` no Firefox 147 em diante, ou em `.mozilla/firefox` nos anteriores) e registra o módulo "Tessera (p11-kit)" no banco NSS de **todos** os perfis. Com o Firefox aberto, o perfil está trancado (o arquivo `lock`), e o Tessera avisa em vez de mexer.

Também dá para fazer à mão: Configurações → Privacidade e Segurança → Dispositivos de segurança → Carregar, arquivo `/usr/lib/x86_64-linux-gnu/pkcs11/p11-kit-client.so`.

No Gerenciador de dispositivos, aparece um módulo com os slots do SerproID, do SafeSign e da confiança do sistema. O "Módulo integrado de raízes" é o conjunto de raízes que vem com o próprio Firefox, não o p11-kit, então não há duplicata.

## Chromium, Google Chrome, Microsoft Edge e Brave

`org.chromium.Chromium`, `com.google.Chrome`, `com.microsoft.Edge`, `com.brave.Browser`. Os navegadores baseados no Chromium leem os certificados de cliente e os módulos PKCS#11 do banco NSS do usuário, `~/.pki/nssdb`, e não têm tela para carregar módulos. O **Configurar** registra o `p11-kit-client.so` nesse banco. Onde o banco fica depende do pacote ([manifestos do Flathub](https://github.com/flathub)):

| Navegador | Banco NSS | Passo 1 |
|---|---|---|
| Chromium | o `~/.pki/nssdb` real (o pacote pode usar a pasta pessoal) | override do socket |
| Microsoft Edge | `~/.var/app/com.microsoft.Edge/.pki/nssdb` (o pacote tem `--persist=.pki`) | override do socket |
| Brave | `~/.var/app/com.brave.Browser/.pki/nssdb` (o pacote tem `--persist=.pki`) | override do socket |
| Google Chrome | `~/.var/app/com.google.Chrome/.pki/nssdb`, **só com `--persist=.pki`** | override do socket **e** `--persist=.pki` |

O pacote do Chrome não guarda o `~/.pki`: sem `--persist=.pki`, o registro se perderia sempre que o Chrome fechasse. Por isso o comando do passo 1 do Chrome inclui essa opção.

O navegador cria o banco na primeira vez que roda. Se o banco ainda não existir, o **Configurar** explica: faça o passo 1, abra o navegador uma vez, reinicie o Tessera (para ele enxergar a pasta nova) e tente de novo. Depois de configurar, reinicie o navegador.

> Ainda não testado com token real. Se o navegador também carregar sozinho o módulo de raízes do runtime, como acontece no Papers, cada certificado vai aparecer duas vezes, e a solução será a mesma do Papers.

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
