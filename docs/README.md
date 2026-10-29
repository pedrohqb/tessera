# Documentação do Tessera

O Tessera é uma central de tokens PKCS#11 para Flatpak. Ele roda os drivers dos tokens (SafeSign/GD Burti, SafeNet e SerproID) dentro do próprio sandbox e entrega os tokens aos outros aplicativos pelo p11-kit, sem instalar pacote de fornecedor no sistema. Foi feito para sistemas imutáveis (Bluefin, Aurora, Bazzite, Fedora Silverblue e Kinoite), em que instalar o `.deb` ou o `.rpm` de um fornecedor no sistema é difícil ou impossível.

Esta documentação registra como o Tessera funciona, por que ele é como é, e tudo o que foi descoberto ao fazê-lo funcionar com tokens reais.

| Documento | O que contém |
|---|---|
| [Arquitetura](arquitetura.md) | Como os tokens saem do sandbox do Tessera e chegam aos aplicativos; por que não há atalho mais simples; supervisão dos drivers |
| [Drivers](drivers.md) | Cada driver: origem, verificação, desempacotamento "limpo", dependências e as peculiaridades encontradas (sobretudo do SerproID) |
| [Aplicativos](aplicativos.md) | PJeOffice Pro, Firefox, Chromium, Chrome, Brave e Papers: o que cada um precisa e por quê |
| [Instalação e uso](uso.md) | Preparar o sistema, instalar, a janela, a bandeja, o início automático e a linha de comando |
| [Diagnóstico](diagnostico.md) | Onde ficam os logs, as ferramentas de teste e o caminho para investigar um problema |
| [Histórico](historico.md) | A sequência de problemas encontrados nos testes de 28 e 29 de outubro de 2026 e como cada um foi resolvido |
| [Desenvolvimento](desenvolvimento.md) | Estrutura do código, compilação, testes, integração contínua, versões e como acrescentar um driver |

## Em uma frase, para cada pessoa

- **Para quem usa:** instale o Tessera, rode uma vez `systemctl --user enable --now p11-kit-server.socket`, e siga os dois passos de cada aplicativo na janela. O PJeOffice Pro do FlatPark não precisa de nada.
- **Para quem mantém um pacote Flatpak que usa token:** dê ao aplicativo `--filesystem=xdg-run/p11-kit/pkcs11` e carregue o `p11-kit-client.so` do runtime. É o que o PJeOffice Pro do FlatPark já faz.
- **Para quem empacota o SerproID em outro lugar:** leia a seção do SerproID em [Drivers](drivers.md#serproid). Há quatro peculiaridades que fazem a assinatura falhar de formas difíceis de entender.

## Situação

Testado em 29 de outubro de 2026 num Bluefin, com um token GD Burti (SafeSign, certificado da OAB) e um certificado em nuvem SerproID:

- PJeOffice Pro (FlatPark): assinatura com SerproID e SafeSign.
- Firefox (Flathub): login com certificado (TLS 1.3) com SerproID e SafeSign.
- Papers: assinatura de PDF com os dois, cada certificado listado uma vez.
- Token Administration do SafeSign e aplicativo do SerproID abertos pelo Tessera.

Chromium, Chrome e Brave estão implementados e testados com um token de teste, ainda sem teste com token real.
