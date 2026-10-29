# Histórico

Como o Tessera chegou à forma atual, nos dias 28 e 29 de outubro de 2026, com os testes feitos num Bluefin com tokens reais. Os problemas estão na ordem em que apareceram. Cada um tem o sintoma, a causa e a solução.

## A ideia (28 de outubro)

1. **A proposta ao FlatPark.** Uma sugestão de melhoria (não um relatório de defeito) para o FlatPark, que já empacota o PJeOffice Pro: oferecer em Flatpak o que o distrobox-adv-br oferece em contêiner, ou seja, SafeSign, SafeNet e SerproID, mais webpki e websigner, utilizáveis no Papers e no Firefox, para sistemas imutáveis.
2. **A resposta do mantenedor** propôs um aplicativo "central", no estilo do `org.flatpark.Cellar`, com duas camadas: o acesso aos tokens e, por cima, o *native messaging* dos assinadores web. Decisão: começar pelo acesso aos tokens, porque webpki e websigner estão em processo de obsolescência e não compensam o esforço.
3. **O desenho.** Um aplicativo próprio, com ícone, genérico (não só ICP-Brasil) e com os drivers como extensões. O OpenSC fica no sistema, que já o tem. O nome escolhido foi **Tessera**, a ficha romana que identificava o portador. A interface é em Rust com GTK 4 e libadwaita, e o repositório é `pedrohqb/tessera`, mantido pelo autor. As "limpezas" do distrobox-adv-br (sem scripts de instalação, sem `/etc`, `libneoidp11.so`, a pasta de certificados do SerproID) são a regra para desempacotar os drivers.
4. **Primeiras compilações.** O OpenSC precisava de `winscard.h`, então o pcsc-lite foi incluído no pacote (o runtime não traz a biblioteca cliente). Vieram depois ajustes de pastas (`--libdir=lib`, módulo e *completions* do OpenSC em `/app`) e uma opção do meson que não existe. Mais tarde o OpenSC saiu do pacote: o do sistema é servido pelo mesmo caminho.

## Primeiros testes no sistema real (29 de outubro, manhã)

5. **A instalação falhou** com "Dados extras faltando nos metadados destacados". Um `.flatpak` avulso não leva o extra-data. A versão de teste passou a ser um repositório Flatpak empacotado (`tessera-repo.tar.gz`), com um script que o adiciona como repositório local.
6. **`bwrap: Can't mount on symlink destination /run/user/1000/p11-kit`.** O primeiro desenho servia os tokens no endereço padrão do p11-kit, mas o Flatpak ocupa esse endereço com o próprio servidor de confiança. Os soquetes foram para `xdg-run/tessera`.
7. **O Firefox via o p11-kit, mas não o token.** A configuração de usuário do p11-kit é ignorada em todo sandbox (`user-config: none`). Daí a arquitetura atual: um servidor por driver e arquivos `remote:` em `~/.config/pkcs11/modules`, servidos pelo `p11-kit-server.socket` do sistema. Uma recomendação anterior, de desligar esse soquete, foi revertida.
8. **SafeSign sem token detectado.** O `libaetpkss.so` não carregava por falta de `libgdbm_compat.so.4`, e a mensagem "failed to fork for accept" vinha dessa falha. A solução foram os pacotes do GDBM do Ubuntu 24.04. O Token Administration pedia o wxWidgets, e passou a vir do RPM do RHEL 10, como no pacote do AUR.
9. **SerproID não abria.** O script do aplicativo fazia `cd /usr/share/…`, e os caminhos foram trocados no desempacotamento. Depois veio `Unable to open DISPLAY` (JavaFX), resolvido com `--socket=x11`. Por fim, a pasta `~/.config/serproid/certificados` tem de existir, como já fazia o distrobox-adv-br.
10. **"Não foi possível adicionar módulo" no Firefox.** O SafeNet, sem token, travava, e um driver travado publicado no sistema trava quem carrega o p11-kit. Isso criou a supervisão (só se publica o que responde) e a chave por driver.
11. **O SafeSign funcionou no Firefox:** CERTIFICADO OAB, "Logado".
12. **SerproID: `CKR_DEVICE_ERROR` pelo p11-kit, mas funcionando direto.** Num servidor iniciado à mão com `P11_KIT_DEBUG` apareceu `undefined symbol: _Unwind_Resume_or_Rethrow`. Solução: `LD_PRELOAD=libgcc_s.so.1`. Os campos do token vinham com bytes NUL, e passaram a ser limpos.
13. **Processos órfãos.** Um servidor órfão prendia o soquete, e conexões travadas sobreviviam ao servidor. Solução: `PR_SET_PDEATHSIG` e um grupo de processos por servidor. O `p11-kit server` com `P11_KIT_PRIVATEDIR` respondia "'server' is not a valid command", então o Tessera passou a executar diretamente o ajudante `p11-kit-server`.

## O login do SerproID (29 de outubro, tarde)

O certificado aparecia no Tessera e no Firefox, mas o login falhava: o pedido chegava ao celular (Aplicação "Tessera"), a aprovação com Face ID dava "Ops! Estamos enfrentando problemas", o pedido se repetia, e o log mostrava `Retornando CKR_FUNCTION_CANCELED depois de receber 20 vezes AGUARDANDO`. No distrobox, o mesmo certificado funcionava. A causa levou uma tarde para aparecer, porque cada hipótese tinha boa aparência:

14. **Hipótese "lista de aplicativos permitidos".** O processo que carregava o driver era o `p11-kit-remote`, e o Tessera passou a atender as conexões ele mesmo (`tessera remote`). Não resolveu. Um teste com uma cópia do Tessera chamada `firefox-esr` também falhou.
15. **Hipótese "é o p11-kit".** O `probe-sign` assinava em 4,5 s direto e pelos dois saltos do p11-kit. Isso pareceu inocentar o caminho, mas o `probe-sign` pede o tamanho antes de assinar, e esse é outro caminho do driver (item 18).
16. **Hipóteses "instâncias que se derrubam" e "conexão velha".** Havia uma só instância do SerproID, e uma conexão nova não mudou nada.
17. **O registro do p11-kit do sistema** (`log-calls: yes`) mostrou que o Firefox pedia exatamente o mesmo `CKM_RSA_PKCS_PSS` dos testes. Diferenças: sessão só de leitura e um `C_Sign` só, sem consulta de tamanho.
18. **A consulta de tamanho.** Com ela a assinatura saía, e o Tessera passou a fazê-la por conta própria. O login então chegou ao fim, mas o site respondeu `SSL_ERROR_DECRYPT_ERROR_ALERT`. O `sign-test.py`, que confere cada assinatura, mostrou "PKCS#1 v1.5 of the data as given (NOT PSS)": por esse caminho o SerproID ignora o PSS. A mudança foi desfeita.
19. **Testes diretos, um de cada vez** (todos aprovados com Face ID): direto funciona; direto com `CKF_OS_LOCKING_OK` funciona; com a linha de comando do p11-kit, com a saída num *pipe*, com nome de programa novo e com a sequência de chamadas do p11-kit (`mimic`), também funciona. Pelo p11-kit, com o `p11-kit-remote` original, com o `tessera remote`, com sinais restaurados, com a entrada e saída padrão trocadas e sem a camada gerenciada, sempre falha.
20. **`strace` e o espião de `getenv`:** nos dois casos, o driver lê os mesmos arquivos, consulta as mesmas variáveis e abre as mesmas conexões. Mas o pedido de assinatura enviado à nuvem tinha **467 bytes direto e 451 pelo p11-kit**. Então o conteúdo mudava.
21. **A causa.** Com os parâmetros do PSS apagados logo depois do `C_SignInit` (`sign-test.py clobber`), o driver direto falhou na hora. Ou seja, o SerproID guarda o ponteiro do mecanismo e só lê os parâmetros no `C_Sign`. Pelo p11-kit, a memória já foi reaproveitada nesse ponto. Um driver falso com o mesmo defeito mostrou o hash chegando como `0xd963a0444bf2382c` em vez de `0x250`. **Solução:** o `tessera remote` entrega ao driver uma cópia estável do mecanismo.
22. **Login aprovado:** "Tudo Certo! seu certificado … foi validado com sucesso", às 16:32.

## Os outros aplicativos e a janela (29 de outubro, fim da tarde)

23. **PJeOffice Pro:** assinatura com SerproID sem nenhuma configuração.
24. **Papers mostrava cada certificado duas vezes.** O NSS do Papers carrega sozinho o módulo de raízes, que no runtime é o p11-kit e, com o `override`, chega ao servidor do sistema. Somado ao `p11-kit-client.so` registrado, dava dois caminhos. Solução: um banco NSS próprio e vazio. Um *reset* do Flatseal apagou o `override` no meio do teste e mostrou que ele é necessário.
25. **O Configurar não fazia nada.** Ele exigia todos os drivers rodando, e um SafeSign sem token bastava para bloqueá-lo.
26. **O SafeSign desligado na janela e o Reiniciar sem efeito.** Uma listagem iniciada antes do reinício terminava depois dele e desligava o driver novo. Além disso, a supervisão só rodava com a janela aberta: com a janela fechada ou no início automático, nada era publicado. A supervisão passou para o serviço, com gerações, três tentativas antes de parar e o estado de cada driver na sua linha.
27. **A janela perdia a referência a si mesma,** e os resultados não chegavam à tela. Ao ser reaberta, mostrava uma janela já destruída. Os dois defeitos apareceram num teste da janela com Xvfb e xdotool.
28. **Acréscimos:** a bandeja, o status no portal Background, a remoção dos módulos ao sair, o Configurar do Firefox automático (todos os perfis), o estado do passo 1 e do servidor do sistema, Chromium, Chrome e Brave, erros em caixa de diálogo e o ícone novo (tessera hospitalis em bronze com pátina).
