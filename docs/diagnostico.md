# Diagnóstico

## Onde olhar primeiro

| O quê | Onde |
|---|---|
| Estado geral | `flatpak run --command=tessera io.github.pedrohqb.Tessera status` |
| Saída de cada driver (e do `tessera remote`) | `~/.var/app/io.github.pedrohqb.Tessera/data/tessera/logs/<driver>.log` (recriado a cada início do driver) |
| Log do SerproID | `~/.var/app/io.github.pedrohqb.Tessera/.config/serproid/serproid.log`: uma linha por assinatura, com o programa que pediu; `CKR_FUNCTION_CANCELED depois de receber 20 vezes AGUARDANDO` quando a aprovação não chega |
| O que foi publicado | `ls ~/.config/pkcs11/modules/` (arquivos `tessera-*.module`) |
| O que o sistema enxerga | `p11-kit list-modules` (no sistema) |
| O que um aplicativo enxerga | `flatpak run --command=sh <app> -c 'p11-kit list-modules'` |
| O servidor do sistema | `systemctl --user status p11-kit-server.socket p11-kit-server.service`; o log em `journalctl --user -u p11-kit-server.service` |
| Permissões de um aplicativo | `flatpak info --show-permissions <app>` e `~/.local/share/flatpak/overrides/<app>` |

## Problemas comuns

| Sintoma | Causa provável | O que fazer |
|---|---|---|
| "Conexão com o sistema" vermelha | `p11-kit-server.socket` desligado, ou ligado depois de o Tessera abrir | `systemctl --user enable --now p11-kit-server.socket`, depois sair do Tessera e abrir de novo; `tessera status` mostra o mesmo resultado |
| O aplicativo não vê token nenhum | falta o passo 1, ou o aplicativo foi aberto antes do passo 1 ou antes de o driver ser publicado | Passo 1, depois fechar e abrir o aplicativo |
| O Tessera vê o token, o aplicativo não | o driver ainda não respondeu (não publicado) ou o aplicativo é anterior à publicação | Ver o estado do driver na janela; reabrir o aplicativo |
| Driver "Parado: sem resposta 3 vezes seguidas" | o driver trava (por exemplo, sem token) | **Reiniciar**, ou desligar o driver se não for usado |
| Certificados em dobro no Papers | `p11-kit-client.so` registrado no banco do Papers | **Configurar** de novo (a versão atual retira o registro) |
| SerproID: celular pede, aprovar dá "Ops!", e o pedido se repete | parâmetros do mecanismo corrompidos (versões anteriores à correção) | Atualizar o Tessera |
| `SSL_ERROR_DECRYPT_ERROR_ALERT` no Firefox com SerproID | assinatura PKCS#1 v1.5 no lugar de PSS | Atualizar o Tessera (a versão de teste que fazia a consulta de tamanho tinha esse efeito) |
| "Não foi possível adicionar módulo" no Firefox | um driver travado publicado no sistema | Desligar o driver na janela; a supervisão atual já evita isso |
| Configurar do Firefox não faz nada | o Firefox estava aberto | Fechar o Firefox e tentar de novo |

## Ferramentas embutidas

Todas rodam dentro do sandbox do Tessera: `flatpak run --command=tessera io.github.pedrohqb.Tessera <comando>`.

- **`status`:** drivers (servindo, parado, arquivos ausentes), leitores, tokens e certificados.
- **`probe-module MÓDULO`:** carrega um driver **direto**, sem o p11-kit, e relata cada passo: inicialização (com `CKF_OS_LOCKING_OK` e, se falhar, sem), informações da biblioteca, slots, tokens e mecanismos. Foi o que mostrou que o SerproID funcionava direto, mas não pelo p11-kit.
- **`probe-sign MÓDULO`:** acha uma chave privada e assina um resumo SHA-256 fixo com `CKM_RSA_PKCS` e com `CKM_RSA_PKCS_PSS`. `TESSERA_PIN=…` faz o login; `TESSERA_MECH=PSS` limita aos mecanismos com esse texto no nome. Para usá-lo pelo p11-kit, passe o `p11-kit-client.so` como módulo e `P11_KIT_SERVER_ADDRESS=unix:path=$XDG_RUNTIME_DIR/tessera/<driver>`. Ele usa a biblioteca `cryptoki`, que pede o tamanho antes de assinar. Com o SerproID, isso leva ao caminho que devolve PKCS#1 v1.5 (veja [Drivers](drivers.md#serproid)). Para imitar um navegador, use o `sign-test.py`.
- **`list-tokens`:** o que a janela usa para listar os tokens de um driver; lê `P11_KIT_SERVER_ADDRESS`.

### Registrador de chamadas

Com `TESSERA_REMOTE_TRACE=1` no ambiente do servidor de um driver, o `tessera remote` anota no log do driver cada chamada PKCS#11 que chega a ele (menos `C_GenerateKeyPair` e `C_UnwrapKey`, que têm oito argumentos): argumentos (mecanismos, parâmetros, modelos de atributos, dados), resultado e duração. Exemplo de um servidor iniciado à mão, dentro do sandbox:

```sh
flatpak run --command=sh io.github.pedrohqb.Tessera -c '
export LD_PRELOAD=libgcc_s.so.1 XDG_CONFIG_HOME=$HOME/.config \
  SSL_CERT_FILE=/etc/ssl/certs/ca-certificates.crt SSL_CERT_DIR=/etc/ssl/certs \
  LD_LIBRARY_PATH=/app/extra/drivers/serproid/usr/lib \
  P11_KIT_PRIVATEDIR=$XDG_DATA_HOME/tessera/libexec TESSERA_REMOTE_TRACE=1
S=$XDG_RUNTIME_DIR/tessera/teste; rm -f $S
/usr/libexec/p11-kit/p11-kit-server --foreground --name $S \
  --provider /app/extra/drivers/serproid/libserproidp11.so pkcs11: 2>$XDG_CACHE_HOME/trace.log &
P=$!; sleep 1
P11_KIT_SERVER_ADDRESS=unix:path=$S tessera list-tokens
kill $P'
cat ~/.var/app/io.github.pedrohqb.Tessera/cache/trace.log
```

`TESSERA_REMOTE_LOAD=raw` serve a tabela de funções do próprio driver, sem a camada gerenciada do p11-kit. Serve para comparar, nunca foi necessário.

### Registro do lado do sistema

O p11-kit do sistema registra as chamadas de um módulo com `log-calls: yes` no arquivo do módulo. Com a janela do Tessera fechada (senão ela reescreve o arquivo):

```sh
F=~/.config/pkcs11/modules/tessera-serproid.module
cp $F ~/tessera-serproid.module.bak && echo 'log-calls: yes' >> $F
# … usar o aplicativo …
journalctl --user -u p11-kit-server.service -o cat > ~/p11-log.txt
cp ~/tessera-serproid.module.bak $F
```

Foi assim que se viu que o Firefox pede exatamente o mesmo `CKM_RSA_PKCS_PSS` (SHA-256, MGF1-SHA256, sal de 32) que os testes.

## `scripts/sign-test.py`

Um script em Python, só com a biblioteca padrão, que assina pelo caminho que um navegador usa: sessão só de leitura e um `C_Sign` só, com RSA-PSS SHA-256. Ele abre cada assinatura com a chave pública do certificado e diz o que ela é: "valid RSA-PSS", "PKCS#1 v1.5 … (NOT PSS)" ou outra coisa. Roda dentro do sandbox do Tessera, direto no driver ou pelo `p11-kit-client.so`:

```sh
cd ~/Downloads   # onde está o sign-test.py
flatpak run --command=env io.github.pedrohqb.Tessera \
  LD_PRELOAD=libgcc_s.so.1 XDG_CONFIG_HOME=$HOME/.config \
  SSL_CERT_FILE=/etc/ssl/certs/ca-certificates.crt SSL_CERT_DIR=/etc/ssl/certs \
  LD_LIBRARY_PATH=/app/extra/drivers/serproid/usr/lib \
  python3 - /app/extra/drivers/serproid/libserproidp11.so random < sign-test.py
```

| Opção | O que muda |
|---|---|
| `fixed` / `random` | cada palavra é uma assinatura: o mesmo resumo de sempre, ou bytes aleatórios (sem nenhuma, uma `random`) |
| `rw` | sessão de leitura e escrita |
| `len` | pergunta o tamanho antes (`C_Sign` sem buffer), como a `cryptoki` |
| `oslock` | inicializa com `CKF_OS_LOCKING_OK`, como o p11-kit |
| `iface` | usa a tabela do `C_GetInterface` (PKCS#11 3.0) |
| `reinit` | `C_Initialize`, `C_Finalize`, `C_Initialize` antes |
| `mimic` | faz antes as chamadas que o `p11-kit-server` faz numa conexão |
| `clobber` | apaga os parâmetros do PSS logo depois do `C_SignInit` (mostra o defeito do SerproID) |
| variável de ambiente `PIN=…` | faz login antes (o SerproID não precisa); como argumento, viraria o prefixo do rótulo |
| um texto | escolhe o token cujo rótulo começa com ele (padrão: `SerproID`) |

## Ferramentas externas usadas na investigação

- **`strace`:** os sistemas imutáveis não trazem o `strace`. Uma cópia do Ubuntu 24.04, com as bibliotecas de que ele precisa, rodada pelo carregador do sistema (`/lib64/ld-linux-x86-64.so.2 --library-path lib strace.bin`), funciona sem instalar nada. Comparar o caminho direto com o caminho pelo p11-kit (`-e trace=%file,%network,%process`) mostrou que o driver faz as mesmas coisas nos dois. O tamanho dos envios TLS (`-e trace=write`) mostrou que o pedido pelo p11-kit era 16 bytes menor (451 em vez de 467), e essa foi a pista que levou à causa.
- **`tools/spy.c`:** uma biblioteca para `LD_PRELOAD` que registra `getenv`, `getpid`, `readlink`, `uname`, `getpwuid` e afins, só quando chamados de dentro do SerproID. Mostrou que o driver só consulta `HOME`, o PID e `/proc/self/exe`. Compilar: `gcc -O2 -shared -fPIC -o libtessera-spy.so tools/spy.c -ldl`; usar com `LD_PRELOAD="libgcc_s.so.1 /caminho/libtessera-spy.so" SPY_LOG=/caminho/spy.log`.
- **`tests/fake-driver/keeps-mechanism-pointer.c`:** um driver PKCS#11 falso que, como o SerproID, guarda o ponteiro do mecanismo e só lê os parâmetros no `C_Sign`. Ele reproduz o defeito pelo p11-kit e confirma a correção (veja [Desenvolvimento](desenvolvimento.md)).
