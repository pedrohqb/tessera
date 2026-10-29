# Arquitetura

## O caminho de um token até o aplicativo

```
Token USB ──► pcscd (sistema)
                 │  --socket=pcsc
                 ▼
┌──────────────── sandbox do Tessera ────────────────┐
│ um servidor p11-kit por driver:                    │
│   $XDG_RUNTIME_DIR/tessera/safesign  ◄─ libaetpkss │
│   $XDG_RUNTIME_DIR/tessera/safenet   ◄─ libeToken  │
│   $XDG_RUNTIME_DIR/tessera/serproid  ◄─ libserproidp11 ──► nuvem do Serpro
│ cada conexão é atendida por `tessera remote`       │
└────────────────────────────────────────────────────┘
                 ▲  "remote: unix:path=…" em
                 │  ~/.config/pkcs11/modules/tessera-<driver>.module
┌──────────────── sistema ───────────────────────────┐
│ p11-kit-server.socket ($XDG_RUNTIME_DIR/p11-kit/pkcs11)
│   serve: confiança do sistema + módulos do sistema │
│          (OpenSC, se houver) + drivers do Tessera  │
└────────────────────────────────────────────────────┘
                 ▲  --filesystem=xdg-run/p11-kit/pkcs11
                 │  + p11-kit-client.so do runtime
   PJeOffice Pro · Firefox · Chromium · Chrome · Edge · Brave · Papers · Okular
```

1. O Tessera inicia um servidor p11-kit para cada driver ligado, cada um com o seu soquete em `$XDG_RUNTIME_DIR/tessera/<driver>`.
2. Para cada driver que responde, ele grava no sistema o arquivo `~/.config/pkcs11/modules/tessera-<driver>.module`, com uma única linha útil: `remote: unix:path=/run/user/<uid>/tessera/<driver>`. Assim o p11-kit do sistema passa a incluir esses tokens entre os seus módulos.
3. O servidor p11-kit do próprio sistema (`p11-kit-server.socket`, ativado uma vez pelo usuário) serve tudo isso em `$XDG_RUNTIME_DIR/p11-kit/pkcs11`: a confiança do sistema, os módulos do sistema e os drivers do Tessera.
4. Um aplicativo Flatpak que recebe `--filesystem=xdg-run/p11-kit/pkcs11` encontra esse soquete no mesmo lugar e o usa por meio do `p11-kit-client.so` do runtime. Aplicativos nativos o veem pelo `p11-kit-proxy.so`.

Nenhum pacote de fornecedor é instalado no sistema. O que o Tessera grava fora do próprio sandbox são só os arquivos `tessera-*.module`, e, nos aplicativos, o registro do módulo no banco NSS (veja [Aplicativos](aplicativos.md)).

## Por que não um atalho mais simples

O caminho tem um salto a mais (o servidor do sistema) porque duas regras do Flatpak tornam impossível o caminho direto:

- **O Flatpak desliga a configuração de usuário do p11-kit em todo sandbox.** Ele grava `/etc/pkcs11/pkcs11.conf` com `user-config: none`. Então nenhum processo dentro de um sandbox pode carregar uma configuração p11-kit própria (por exemplo, uma lista de módulos do Tessera). Por isso cada driver tem um servidor próprio, que não precisa de configuração nenhuma (`--provider <driver>`), e a junção dos drivers fica a cargo do p11-kit do sistema, que lê `~/.config/pkcs11/modules`.
- **O Flatpak ocupa `$XDG_RUNTIME_DIR/p11-kit` com o seu próprio servidor de confiança** (do `flatpak-session-helper`) e apaga `P11_KIT_SERVER_ADDRESS` do ambiente. Por isso o Tessera não pode servir no endereço padrão. A tentativa de montar `xdg-run/p11-kit` dentro do sandbox do Tessera falhou com `bwrap: Can't mount on symlink destination /run/user/1000/p11-kit`. Os soquetes ficam em `xdg-run/tessera`.

A permissão `xdg-run/p11-kit/pkcs11` dada a um aplicativo substitui, só para ele, o servidor de confiança do Flatpak pelo servidor do sistema. Como o servidor do sistema também serve a confiança do sistema, o aplicativo não perde os certificados raiz.

## O processo que carrega o driver

O `p11-kit-server` cria um processo por conexão e, nesse processo, roda o executável `p11-kit-remote` do diretório `P11_KIT_PRIVATEDIR`. O Tessera aponta essa variável para `$XDG_DATA_HOME/tessera/libexec`, onde grava um `p11-kit-remote` que só executa `tessera remote "$@"`. Assim, quem carrega o driver é o próprio Tessera (`src/remote.rs`). Antes de carregar o driver, `tessera remote`:

- devolve todos os sinais ao comportamento padrão e desbloqueia a máscara (o `p11-kit-server` ignora alguns, e sinais ignorados atravessam o `exec`);
- tira o canal do p11-kit da entrada e saída padrão, que viram `/dev/null` e o log, para que um driver que escreva na saída padrão não corrompa o protocolo;
- embrulha a tabela de funções do driver para entregar a ele cópias estáveis dos parâmetros de mecanismo (veja [SerproID](drivers.md#serproid)). Essa é a correção que fez o SerproID funcionar pelo p11-kit;
- com `TESSERA_REMOTE_TRACE=1`, registra cada chamada no log (veja [Diagnóstico](diagnostico.md)).

O ambiente de cada driver vem do `[env]` do seu `driver.toml` (por exemplo, o `LD_PRELOAD` do SerproID), e `LD_LIBRARY_PATH` vem do `library_path`.

## Supervisão dos drivers

A supervisão roda no serviço, com ou sem janela (`supervise` em `src/ui.rs`, `check_drivers` em `src/main.rs`):

- A cada 4 s com a janela aberta, a cada 16 s com ela fechada (e a cada 10 s no `tessera serve`), o Tessera lista os tokens de cada driver em funcionamento. Ele faz isso num processo à parte (`tessera list-tokens`), com limite de 10 s, para um driver travado não travar a janela.
- Os drivers que respondem são publicados no sistema. Os que não respondem em 10 s deixam de ser publicados e são reiniciados; os que respondem com erro só deixam de ser publicados. Depois de 3 vezes seguidas sem resposta, o driver é parado, e o motivo aparece na sua linha da janela. Isso acontece com o SafeSign quando o token é retirado. Por isso, um driver parado assim volta sozinho quando um token é inserido (uma leitora nova com cartão aparece na listagem do `pcscd`). Um driver travado publicado no sistema travaria todos os aplicativos que carregam o p11-kit. Foi o que aconteceu com o SafeNet sem token no primeiro teste no Firefox.
- Cada (re)início do serviço (abrir, Iniciar/Reiniciar, ligar ou desligar um driver, Sair) incrementa uma "geração". Uma listagem que começou antes é descartada e refeita na hora. Antes dessa regra, o resultado vencido desligava o driver que acabara de voltar.
- **Sair** para os servidores e apaga os arquivos `tessera-*.module`, para os aplicativos não esperarem por servidores que não existem.

## Janela, bandeja e segundo plano

- O serviço é um `adw::Application` com uma "hold": continua rodando quando a janela fecha.
- Enquanto o serviço roda, com a janela aberta ou fechada, há um ícone na bandeja (dá para tirá-lo no menu, em **Mostrar ícone na bandeja**) (StatusNotifierItem, pela biblioteca `ksni`) com o resumo ("Servindo · 2/2 driver(s) · 2 publicado(s) · 2 token(s)"), **Abrir o Tessera** e **Sair (parar de servir os tokens)**. O GNOME mostra esse ícone com a extensão AppIndicator, que o Bluefin, o Aurora e o Ubuntu já trazem; o KDE mostra sem extensão. O mesmo resumo vai para o portal Background, que o GNOME mostra na lista de aplicativos em segundo plano.
- `tessera --background` inicia só o serviço. É o comando que o portal Background usa quando o início automático está ligado.

## Permissões do Flatpak

| Permissão | Para quê |
|---|---|
| `--share=ipc`, `--device=dri` | A janela (memória compartilhada do X11, aceleração gráfica) |
| `--socket=pcsc` | Falar com o `pcscd` do sistema, que é o dono dos leitores USB |
| `--share=network` | SerproID (nuvem) e verificações de revogação das ferramentas dos fornecedores |
| `--socket=x11`, `--socket=wayland` | A janela; o X11 completo porque o SerproID (JavaFX) e o Token Administration só desenham por X11 |
| `--filesystem=xdg-run/tessera:create` | Os soquetes dos servidores dos drivers |
| `--filesystem=xdg-config/pkcs11:create` | Gravar os `tessera-*.module`; nenhum outro arquivo é tocado |
| `--filesystem=xdg-run/p11-kit/pkcs11` | Conferir se os aplicativos veem os tokens do Tessera pelo servidor do sistema (veja [Aplicativos](aplicativos.md)) |
| `--filesystem=xdg-data/flatpak/overrides:ro` | Mostrar se o passo 1 de cada aplicativo já foi feito |
| `--filesystem=xdg-data/flatpak/exports/share/icons:ro`, `xdg-data/flatpak/app:ro` e os mesmos em `/var/lib/flatpak` | Mostrar cada aplicativo com o próprio ícone. Os ícones exportados são links para a pasta de cada aplicativo em `app/`, por isso ela também é necessária, só para leitura |
| `--filesystem=~/.var/app/org.gnome.Papers/config/mozilla:create`, `~/.var/app/org.kde.okular/config/mozilla:create` | O banco NSS próprio do Papers e do Okular |
| `--filesystem=~/.var/app/org.mozilla.firefox/config/mozilla/firefox` e `.../.mozilla/firefox` | Registrar o módulo nos perfis do Firefox |
| `--filesystem=~/.pki/nssdb` e, para o Chromium, o Brave, o Chrome e o Edge, `~/.var/app/<navegador>/data/pki/nssdb` e `~/.var/app/<navegador>/.pki/nssdb` | Registrar o módulo no banco NSS dos navegadores baseados no Chromium (o novo, desde o Chromium 146, e o antigo) |
| `--talk-name=org.kde.StatusNotifierWatcher` | O ícone na bandeja |
| `--persist=.config/serproid` | A pasta de certificados e o log do SerproID ficam nos dados do próprio Tessera |

As pastas dos navegadores não levam `:create`. O navegador cria o próprio banco na primeira vez que roda, e o Tessera não cria pastas para navegadores que não estão instalados.
