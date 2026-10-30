# Instalação e uso

## Preparar o sistema (uma vez)

- **`pcscd` com o driver CCID.** A maioria das distribuições traz os dois, e as imagens Fedora atômicas e Universal Blue (Bluefin, Aurora, Bazzite) já vêm com eles. Para conferir: `systemctl status pcscd.socket`.
- **O servidor p11-kit do sistema:**
  ```sh
  systemctl --user enable --now p11-kit-server.socket
  ```
  Nos sistemas baseados no Fedora, a unidade vem no pacote `p11-kit-server`, que as imagens atômicas já trazem.

Nada mais é instalado no sistema.

## Instalar a versão publicada

Cada versão (tag `vX.Y.Z`) publica em [Releases](https://github.com/pedrohqb/tessera/releases) o `tessera-repo.tar.gz`, um pequeno repositório Flatpak com o script `install-test-repo.sh`:

```sh
mkdir -p ~/Downloads/tessera && cd ~/Downloads/tessera
curl -fLO https://github.com/pedrohqb/tessera/releases/latest/download/tessera-repo.tar.gz
tar -xzf tessera-repo.tar.gz
./install-test-repo.sh
```

O runtime do GNOME vem do Flathub, que precisa estar configurado.

## Instalar a versão de teste

Cada envio para a `main` gera, na integração contínua, o artefato `tessera-x86_64`. Ele contém `tessera-repo.tar.gz`, um pequeno repositório Flatpak. Um pacote `.flatpak` avulso não serve, porque não carrega o extra-data dos drivers: a instalação falha com "Dados extras faltando nos metadados destacados".

```sh
mkdir -p ~/Downloads/tessera-teste && cd ~/Downloads/tessera-teste
unzip ~/Downloads/tessera-x86_64.zip
tar -xzf tessera-repo.tar.gz
./install-test-repo.sh
```

O script adiciona a pasta como o repositório local `tessera-test` e instala ou atualiza o Tessera. Na instalação, o Flatpak baixa os drivers dos fornecedores e confere cada sha256.

Depois de atualizar, saia do Tessera (menu → **Sair**) e abra de novo.

## A janela

- **Serviço:** o estado dos servidores dos drivers, com **Iniciar** ou **Reiniciar**. Logo abaixo, **Conexão com o sistema**, que confere de ponta a ponta se os aplicativos veem os tokens do Tessera pelo `p11-kit-server.socket`, com o comando de ativação para copiar. Por último, **Serviço de cartões (pcscd)**.
- **Leitoras:** os leitores que o `pcscd` vê e se há cartão.
- **Tokens:** cada token com o driver e os certificados (titular, emissor, validade). Um certificado vencido aparece em destaque.
- **Drivers:** cada driver com a sua chave liga/desliga e o seu estado: "Publicado: os aplicativos podem usar", "Iniciando…", "Sem resposta; tentando de novo (1 de 3)", "Parado: …" ou "Desligado". Os botões das ferramentas dos fornecedores (Token Administration, SerproID) ficam aqui.
- **Aplicativos:** os dois passos de cada aplicativo (veja [Aplicativos](aplicativos.md)), com o estado "Pronto", "Falta o passo 1", "Falta o passo 2" ou "Faltam os passos 1 e 2". Um passo feito aparece com ✓ e sem botão, e os estados se atualizam sozinhos, sem reiniciar o Tessera.
- O botão de atualizar, no canto superior esquerdo, faz uma conferência na hora. O menu tem **Iniciar automaticamente no login**, **Mostrar ícone na bandeja**, **Sobre** e **Sair** (Ctrl+Q).

Os erros de configuração aparecem numa caixa de diálogo, com o texto inteiro e o que fazer.

## Fechar a janela, a bandeja e o segundo plano

Fechar a janela **não** para o serviço: os tokens continuam disponíveis. Enquanto o serviço roda, com a janela aberta ou fechada, há um ícone na bandeja (dá para tirá-lo no menu, em **Mostrar ícone na bandeja**) com o resumo, **Abrir o Tessera** e **Sair (parar de servir os tokens)**. Um clique no ícone abre a janela. No GNOME, a bandeja depende da extensão AppIndicator, que o Bluefin já traz.

**Sair**, pelo menu ou pela bandeja, para os servidores e retira os drivers do sistema.

**Iniciar automaticamente no login** pede ao sistema, pelo portal Background, que o Tessera seja aberto em segundo plano (`tessera --background`) no login: sem janela, só o serviço e a bandeja.

## Linha de comando

```
flatpak run io.github.pedrohqb.Tessera                 a janela (e o serviço)
flatpak run io.github.pedrohqb.Tessera --background    só o serviço
flatpak run --command=tessera io.github.pedrohqb.Tessera serve
                                                       só o serviço, no terminal, com as mensagens
flatpak run --command=tessera io.github.pedrohqb.Tessera status
                                                       drivers, leitores, tokens e certificados
flatpak run --command=tessera io.github.pedrohqb.Tessera nss-setup DIR
                                                       registra o p11-kit-client.so no banco NSS DIR
```

O `serve` não pode rodar junto com a janela, porque os dois usariam os mesmos soquetes. Saia do Tessera antes. As ferramentas de diagnóstico estão em [Diagnóstico](diagnostico.md).
