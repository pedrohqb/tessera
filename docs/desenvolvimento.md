# Desenvolvimento

## Estrutura do repositório

```
src/
  main.rs       linha de comando; check_drivers (publicar, reiniciar, parar) e Health
  ui.rs         a janela, a supervisão sem janela, a bandeja e o portal Background
  server.rs     um p11-kit-server por driver (grupo de processos, PDEATHSIG, reinício)
  remote.rs     `tessera remote`: o processo que carrega o driver em cada conexão
  stable.rs     cópia estável dos parâmetros de mecanismo (o defeito do SerproID)
  trace.rs      registrador de chamadas (TESSERA_REMOTE_TRACE=1)
  status.rs     leitores (pcsc), listagem de tokens, probe-module, probe-sign
  apps.rs       os aplicativos, os arquivos tessera-*.module, overrides
  nss.rs        bancos NSS: registrar o p11-kit-client.so, o banco do Papers
  drivers.rs    descoberta e leitura dos driver.toml
  paths.rs      todos os caminhos (com as variáveis de ambiente de desenvolvimento)
  tray.rs       o ícone na bandeja (ksni)
  settings.rs   $XDG_CONFIG_HOME/tessera/settings.toml, no Flatpak
                ~/.var/app/io.github.pedrohqb.Tessera/config/tessera/settings.toml
  i18n.rs       tabela de tradução para o português do Brasil
flatpak/
  io.github.pedrohqb.Tessera.yml   o manifesto (permissões, pcsc-lite, extra-data)
  apply_extra.sh                   desempacota os drivers na instalação
  drivers/<driver>/driver.toml     a descrição de cada driver
data/          .desktop, metainfo, ícones (normal e simbólico), captura de tela
scripts/       pin-drivers.py, install-test-repo.sh, sign-test.py
tests/fake-driver/   drivers PKCS#11 falsos: um guarda o ponteiro do mecanismo,
                     outro (slow-slot-list.c) nunca responde a tempo
tools/         spy.c (espião LD_PRELOAD), gui-test/ (a janela em Xvfb)
docs/          esta documentação
```

## Compilar

Desenvolvimento, fora do Flatpak (precisa dos arquivos de desenvolvimento do GTK 4, da libadwaita e do pcsc-lite):

```sh
cargo build
cargo clippy -- -D warnings
cargo test
```

O pacote Flatpak:

```sh
python3 scripts/pin-drivers.py      # confere os sha256 e preenche os tamanhos
flatpak-builder --user --install-deps-from=flathub --force-clean --install \
  build-dir flatpak/io.github.pedrohqb.Tessera.yml
```

O manifesto usa o runtime `org.gnome.Platform` 51, o Rust da extensão `rust-stable`, e compila o pcsc-lite 2.5.3 (só a biblioteca cliente, `--libdir=lib`), porque o runtime não o tem. O protocolo do pcsc-lite é compatível com o `pcscd` do sistema.

## Variáveis de ambiente de desenvolvimento

| Variável | Padrão | Uso |
|---|---|---|
| `TESSERA_DRIVERS_PATH` | `/app/lib/tessera/drivers:/app/lib/tessera/extensions` | onde procurar `driver.toml` |
| `TESSERA_SOCKET_DIR` | `$XDG_RUNTIME_DIR/tessera` | soquetes dos servidores |
| `TESSERA_HOST_MODULES` | `~/.config/pkcs11/modules` | onde gravar `tessera-*.module` |
| `TESSERA_P11KIT_SERVER` | o primeiro que existir de `/usr/libexec/p11-kit/p11-kit-server` e afins | o ajudante do p11-kit |
| `TESSERA_CLIENT_MODULE` | `/usr/lib/x86_64-linux-gnu/pkcs11/p11-kit-client.so` | o módulo cliente que `list-tokens` e `status` carregam (o registro nos bancos NSS usa sempre o caminho padrão) |
| `TESSERA_REMOTE_TRACE` | — | `1`: registrar as chamadas ao driver |
| `TESSERA_REMOTE_LOAD` | `managed` | `raw`: servir a tabela do próprio driver |
| `TESSERA_PIN`, `TESSERA_MECH` | — | `probe-sign`: PIN e filtro de mecanismo |

Para testar fora do Flatpak, use um usuário comum: com root, o p11-kit ignora a configuração de usuário (`~/.config/pkcs11/modules`).

## Testes

- **Unitários:** `cargo test`.
- **O caminho completo com SoftHSM:** inicie um `p11-kit-server` com `P11_KIT_PRIVATEDIR` apontando para um `p11-kit-remote` que execute `target/debug/tessera remote "$@"`, e assine com `scripts/sign-test.py` pelo `p11-kit-client.so` e `P11_KIT_SERVER_ADDRESS`. Com a variável de ambiente `PIN=1234`, o script faz o login no SoftHSM.
- **O defeito do SerproID:** `gcc -shared -fPIC -o libfake.so tests/fake-driver/keeps-mechanism-pointer.c` e use-o como `--provider`. Pelo `p11-kit-remote` original, o log do servidor mostra `fake: C_Sign sees … hash=<lixo> -> WRONG`; pelo `tessera remote`, `hash=0x250 … -> OK`.
- **A janela:** `tools/gui-test/run.sh` abre o Tessera num Xvfb com uma sessão D-Bus própria, um driver SoftHSM de teste e uma bandeja falsa (`fake-watcher.py`, que registra o ícone e o menu). Daí, com `. /tmp/tessera-gui/env.sh`, `xdotool` clica e `import -window root` tira a captura. Foi assim que se testaram os botões, o Reiniciar, as chaves, o fechar e reabrir, a bandeja e o Sair. O driver `tests/fake-driver/slow-slot-list.c` (um `C_GetSlotList` que leva 12 s), compilado e posto numa pasta com um `driver.toml` em `TESSERA_DRIVERS_PATH`, testa as três tentativas.

## Integração contínua

`.github/workflows/ci.yml`, a cada envio para a `main`, a cada pull request, a cada tag `v*` e quando disparado à mão:

1. **Build and test:** `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test`.
2. **Flatpak bundle:** `pin-drivers.py`, `flatpak-builder` (com as últimas linhas do log numa anotação, se falhar), `flatpak build-update-repo`, e os artefatos `tessera-repo.tar.gz` (repositório de teste com `install-test-repo.sh`) e `tessera-x86_64.tar.gz` (a árvore `/app` inteira, para quem quiser empacotar como extra-data).
3. **Com uma tag `vX.Y.Z`:** cria a versão no GitHub com os dois arquivos.

## Publicar uma versão

1. Atualizar a versão em `Cargo.toml` e acrescentar a entrada `<release>` no metainfo.
2. `git tag v0.1.0 && git push origin v0.1.0`.
3. A integração contínua publica a versão em https://github.com/pedrohqb/tessera/releases.

## Acrescentar um driver

Um driver é uma pasta com um `driver.toml` (o formato está em [Drivers](drivers.md)). Os que vêm com o Tessera ficam em `flatpak/drivers/` e no `apply_extra.sh`. Um driver de terceiros pode vir como extensão Flatpak `io.github.pedrohqb.Tessera.Driver.<Nome>`, montada em `/app/lib/tessera/extensions/<Nome>/`, sem mudar o Tessera. Para um driver novo, vale conferir:

- se ele carrega sozinho (`probe-module`) e pelo p11-kit (`list-tokens` com um servidor à mão);
- se assina direto e pelo p11-kit, com o `sign-test.py` (PSS válido, e não PKCS#1 v1.5);
- se ele precisa de algum arquivo ou variável de ambiente (`[env]`, `create_dirs`, `library_path`);
- como ele se comporta sem token: se travar, a supervisão o para depois de três tentativas.

## Rumo ao Flathub

Ainda falta: publicar no Flathub, o que exige revisão das permissões (as pastas de outros aplicativos e o `talk-name` da bandeja precisam de justificativa) e uma captura de tela atual; e testar o SafeNet e os navegadores Chromium com tokens reais.
