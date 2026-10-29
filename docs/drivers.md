# Drivers

## Como os drivers chegam ao Tessera

Os drivers não fazem parte do Tessera, porque as licenças dos fornecedores não permitem redistribuí-los. Eles usam o *extra-data* do Flatpak: na instalação, o Flatpak baixa cada arquivo do site do fornecedor, confere o sha256 fixado no manifesto e só então roda `flatpak/apply_extra.sh`, sem rede, dentro do sandbox.

| Driver | Arquivo do fornecedor | sha256 |
|---|---|---|
| SafeSign IC 4.5.0.0 (A.E.T. Europe) | `SafeSignICStandardLinux4.5.0.0-AET.000ub2404x86_64.deb` | `7742e21e…1b91` |
| SafeSign, Token Administration | `SafeSignICStandardLinux4.5.0.0-AET.000redhat10x86_64.rpm` | `2f5ea462…3378` |
| GDBM, para o SafeSign | Ubuntu 24.04 `libgdbm6t64_1.23-5.1build1_amd64.deb` e `libgdbm-compat4t64_1.23-5.1build1_amd64.deb` | `18d6d74a…645e`, `fb8564af…ee65` |
| SafeNet Authentication Client 10.9 (Thales) | `Linux_SAC_10.9_GA.zip` (DigiCert) | `46759cfe…013e` |
| SerproID 2.1.6 (Serpro) | `SerproID-2.1.6-amd64.deb` | `0ffa9ffe…e588` |

Os sha256 são os mesmos do [distrobox-adv-br](https://github.com/pedrohqb/distrobox-adv-br). O Flatpak exige também o tamanho de cada arquivo; `scripts/pin-drivers.py` baixa cada um, confere o sha256 e preenche o tamanho no manifesto. A integração contínua roda esse script antes de cada compilação, então um arquivo trocado no site do fornecedor quebra a compilação em vez de passar despercebido.

## O desempacotamento "limpo"

`apply_extra.sh` segue as mesmas "limpezas" do distrobox-adv-br:

- só os arquivos do pacote são extraídos (o `data.tar` do `.deb`, lido com `bsdtar`, porque o runtime não tem `dpkg` nem `ar`);
- **nenhum script do fornecedor roda** (nada de `postinst`/`postrm`);
- o `/etc` do pacote é descartado;
- tudo fica em `/app/extra/drivers/<driver>`, e nada é gravado fora dessa pasta;
- um driver que falha ao desempacotar não interrompe a instalação: aparece na janela como "Arquivos do driver ausentes", e os outros continuam.

Cada driver tem um `driver.toml` em `/app/lib/tessera/drivers/<driver>/`. Um trecho do do SerproID:

```toml
name = "SerproID"
vendor = "Serpro"
description = "Cloud certificate; sign in with the SerproID app"
module = "/app/extra/drivers/serproid/libserproidp11.so"
library_path = ["/app/extra/drivers/serproid/usr/lib"]   # LD_LIBRARY_PATH do driver
create_dirs = ["~/.config/serproid/certificados"]        # criadas antes de iniciar

[env]                                                    # ambiente do servidor do driver
LD_PRELOAD = "libgcc_s.so.1"
XDG_CONFIG_HOME = "~/.config"
SSL_CERT_FILE = "/etc/ssl/certs/ca-certificates.crt"
SSL_CERT_DIR = "/etc/ssl/certs"

[[tools]]                                                # botões na janela
name = "SerproID"
exec = ["/app/extra/drivers/serproid/serproid"]
```

## SafeSign IC (GD Burti)

- **Biblioteca:** `libaetpkss.so`, ligada em `drivers/safesign/libaetpkss.so`.
- **GDBM:** a biblioteca precisa de `libgdbm_compat.so.4`, que o runtime do GNOME não tem. O sintoma era o driver não carregar. Os dois pacotes do GDBM do Ubuntu 24.04 vão para `drivers/safesign/gdbm`, que entra no `library_path`. O pacote do AUR (`safesignidentityclient`) também depende de `gdbm`.
- **Token Administration:** o `tokenadmin` do `.deb` do Ubuntu precisa do wxWidgets, que o runtime não tem. O do RPM do RHEL 10 não precisa, e é ele que o Tessera usa, como o AUR. Ele roda por um lançador (`drivers/safesign/tokenadmin`) que põe as bibliotecas do SafeSign e o GDBM no `LD_LIBRARY_PATH`. Precisa do X11 completo (`--socket=x11`).
- **Comportamento observado:** com o token conectado, a primeira busca de objetos (`C_FindObjectsInit`) leva cerca de 1,6 s. O token da OAB tem 4 certificados (as três ACs da cadeia e o do titular). O driver anuncia PSS com SHA-1/256/384/512, além de PKCS#1 v1.5.

## SafeNet Authentication Client (eToken 5100/5110, IDPrime)

- O zip traz pacotes para várias distribuições. Como no distrobox-adv-br, só o pacote "withoutUI" do Ubuntu é usado (`safenetauthenticationclient-core_*_amd64.deb`).
- **Biblioteca:** `libeToken.so`.
- **Comportamento observado:** sem token conectado, o driver demorou a responder no primeiro teste, e o p11-kit do sistema, ao carregá-lo, fez o Firefox mostrar "Não foi possível adicionar módulo". Por isso existe a supervisão: um driver que não responde não é publicado. Cada driver também pode ser desligado na janela. Ainda não foi testado com um token SafeNet real.

## SerproID

O SerproID é um certificado em nuvem. O driver (`libserproidp11.so`) não fala com nenhum leitor: ele lê a lista de certificados baixada pelo aplicativo SerproID e, a cada assinatura, pede a aprovação no celular do titular pela nuvem do Serpro. Ele foi o driver mais difícil. Todas as peculiaridades abaixo foram encontradas nos testes, e a maioria não aparece quando o driver é carregado diretamente por um aplicativo (como o Firefox no distrobox).

### Configuração que ele exige

1. **A pasta de certificados precisa existir,** mesmo vazia: `~/.config/serproid/certificados`. Sem ela, o aplicativo SerproID fecha. O distrobox-adv-br faz o mesmo. O Tessera a cria (`create_dirs`) antes de iniciar o driver e antes de abrir o aplicativo.
2. **A pasta de configuração:** o driver lê `serproid/certificados`, `serproid/ambiente` e grava `serproid/serproid.log` na pasta de configuração do usuário. O Flatpak define `XDG_CONFIG_HOME` como a pasta de configuração do próprio aplicativo, então o driver e o aplicativo procurariam em lugares diferentes. O `driver.toml` define `XDG_CONFIG_HOME=~/.config`, e o manifesto tem `--persist=.config/serproid`, que guarda essa pasta em `~/.var/app/io.github.pedrohqb.Tessera/.config/serproid`.
3. **Certificados de CA:** o OpenSSL embutido procura as raízes em `/usr/lib/ssl`, o lugar do Debian, que o runtime não tem. `SSL_CERT_FILE=/etc/ssl/certs/ca-certificates.crt` e `SSL_CERT_DIR=/etc/ssl/certs`.
4. **O aplicativo:** o script de inicialização e a configuração do aplicativo usam caminhos absolutos (`cd /usr/share/serproid-desktop; ./jre/bin/java …`). O `apply_extra.sh` troca esses caminhos, só nos arquivos de texto, pela cópia em `/app/extra/drivers/serproid`. O aplicativo é JavaFX e só desenha por X11 (o erro era `Unable to open DISPLAY`).
5. **`libneoidp11.so`:** o módulo também é oferecido com esse nome, como no distrobox-adv-br, porque alguns assinadores o procuram assim.

### As quatro peculiaridades que quebram a assinatura

1. **`undefined symbol: _Unwind_Resume_or_Rethrow`.** A biblioteca usa o desenrolador de exceções da `libgcc_s` sem declará-la como dependência. Num aplicativo em C++ ou Java a `libgcc_s` já está carregada, e o erro não aparece. No `p11-kit-remote` (em C) ou no Tessera (em Rust), o `dlopen` falha e o p11-kit só devolve `CKR_DEVICE_ERROR`. A correção é `LD_PRELOAD=libgcc_s.so.1` no ambiente do driver. O erro real só apareceu com `P11_KIT_DEBUG` num servidor iniciado à mão.
2. **Ele guarda o ponteiro do mecanismo.** No `C_SignInit`, o PKCS#11 só garante que o `CK_MECHANISM` (e os parâmetros do PSS) é válido durante a chamada. O SerproID guarda o ponteiro e só lê os parâmetros no `C_Sign`. Um aplicativo que carrega o driver diretamente mantém os parâmetros vivos, e tudo funciona. Pelo p11-kit, eles ficam na mensagem RPC, que é reaproveitada assim que o `C_SignInit` retorna. No `C_Sign`, o campo do hash do PSS estava corrompido. Num driver falso que reproduz o defeito, chegou `0xd963a0444bf2382c` em vez de `0x250` (SHA-256). O pedido ia para a nuvem com o hash errado, o celular recebia o pedido, a aprovação dava "Ops! Estamos enfrentando problemas", e o driver insistia até devolver `CKR_FUNCTION_CANCELED` (`Retornando CKR_FUNCTION_CANCELED depois de receber 20 vezes AGUARDANDO.` no log). A prova foi o `sign-test.py clobber`: com os parâmetros apagados logo depois do `C_SignInit`, o driver direto falhou na hora. A correção (`src/stable.rs`) é o `tessera remote` entregar ao driver uma cópia do mecanismo que dura até o próximo `*Init` da mesma sessão. Vale para todos os drivers.
3. **Com consulta de tamanho, ele assina em PKCS#1 v1.5.** Se o `C_Sign` é chamado primeiro sem buffer (para saber o tamanho, como faz a biblioteca `cryptoki`), o SerproID devolve uma assinatura PKCS#1 v1.5 dos dados, mesmo quando o mecanismo pedido é PSS. O TLS 1.3 exige PSS, e o site responde `SSL_ERROR_DECRYPT_ERROR_ALERT`. Uma tentativa intermediária, que acrescentava a consulta de tamanho para contornar o item 2, tinha esse efeito e foi desfeita. O NSS (Firefox, Papers) chama o `C_Sign` uma vez só, com buffer, que é o caminho certo.
4. **Rótulos com NUL.** Os campos do token (rótulo, fabricante, modelo, série) vêm preenchidos com bytes NUL em vez de espaços. O Tessera os limpa ao mostrar.

### Outros comportamentos observados

- **Mecanismos:** só `CKM_RSA_PKCS` e `CKM_RSA_PKCS_PSS`. Não há `CKM_SHA256_RSA_PKCS` (`Mecanismo invalido: 0x40` no log). Quem assina precisa calcular o hash antes.
- **Interface:** PKCS#11 2.20, sem `C_GetInterface`.
- **Token:** `CKF_PROTECTED_AUTHENTICATION_PATH` e sem login. A "senha" é a aprovação no celular.
- **Identificação do aplicativo:** a cada assinatura, o driver lê `/proc/self/exe` e grava o caminho no log. O nome aparece no celular como "Aplicação" ("Tessera", pelo p11-kit). Os testes mostraram que o nome não importa para a aprovação.
- **Janela de autorização:** logo depois de uma aprovação no celular, novas assinaturas do mesmo aplicativo saem sem perguntar, em cerca de 0,2 s, durante alguns minutos. Isso confundiu vários testes, porque uma assinatura "que funcionou" pode não ter passado pela aprovação.
- **Assinaturas guardadas:** o mesmo resumo assinado de novo devolve a mesma assinatura, em 0,2 s.
- **Notificação:** a cada pedido, ele roda `sh -c "notify-send … 'Foi enviado um pedido de PIN para o seu dispositivo.'"`.
- **Log:** `~/.var/app/io.github.pedrohqb.Tessera/.config/serproid/serproid.log`. O arquivo é acrescentado, não apagado. Linhas como `Tipo de classe nao tratado em C_FindObjectsInit: 0x9` são buscas do NSS por objetos que o driver não tem, e não indicam erro.
- **`maquinaId`:** um arquivo na mesma pasta identifica a máquina na nuvem. O do Tessera é diferente do do distrobox.

### O que não era a causa (e foi descartado nos testes)

O nome do programa, a linha de comando, o ambiente, os sinais, a entrada e saída padrão, a inicialização com `CKF_OS_LOCKING_OK`, o encerramento e reinício que o p11-kit faz antes de cada conexão, várias instâncias do driver ao mesmo tempo, e a idade da conexão. Todos foram testados um a um. O caminho está em [Histórico](historico.md).
