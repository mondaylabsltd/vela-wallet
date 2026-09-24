---
title: Instalar a Vela
description: "Todas as formas de usar a Vela — web, extensão de navegador, desktop e celular —, quanto cada uma custa, o que cada uma faz e do que o seu aparelho precisa."
source: fa80f5cfdb95
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Instalar a Vela

A mesma carteira roda em vários lugares, e todos abrem o mesmo endereço com as
mesmas chaves. Escolha pelo que você precisa; dá para usar mais de um. Os downloads
estão em [Obter a Vela](/pt-BR/get-started).

| | O que é | Custo | Situação |
| --- | --- | --- | --- |
| **Web** | [wallet.getvela.app](https://wallet.getvela.app/) em qualquer navegador recente | Grátis | No ar |
| **Extensão de navegador** | A carteira na barra de ferramentas; conecta-se a dApps | Grátis | Baixar e carregar manualmente; ainda não está na Chrome Web Store |
| **Desktop** | App nativo para macOS, Windows e Linux | Grátis | Download em Obter a Vela ou no GitHub |
| **iPhone, Android** | Apps nativos | Compra única nas lojas | Ainda não estão nas lojas; você pode compilar a partir do código-fonte |

<a href="https://wallet.getvela.app/" target="_blank" rel="noopener" style="display:inline-block;margin:4px 0 8px;padding:11px 22px;border-radius:10px;background:#e8572a;color:#fff;font-weight:600;text-decoration:none;">Abrir a carteira web →</a>

## Web

Nada para instalar. Abra [wallet.getvela.app](https://wallet.getvela.app/), crie
uma carteira ou faça login, e ela está lá. A sua lista de contas fica guardada neste
navegador; em outro aparelho, basta fazer login de novo com uma das suas chaves.

## Extensão de navegador

Para navegadores Chromium: Chrome, Edge e Brave (Chrome 116 ou mais recente). Ela
coloca a carteira na barra de ferramentas e deixa os dApps se conectarem direto a
ela. Enquanto não estiver na Chrome Web Store:

1. Baixe a extensão em [Obter a Vela](/pt-BR/get-started) e descompacte numa pasta
   que você vai manter — o navegador executa a extensão a partir dela.
2. Abra `chrome://extensions` e ative o **Modo do desenvolvedor**.
3. Clique em **Carregar sem compactação** e escolha essa pasta.

É a mesma carteira: a extensão e a carteira web usam as mesmas passkeys do
`getvela.app`, então as mesmas chaves abrem o mesmo endereço.

## Desktop

Um app nativo, não uma página web dentro de uma janela: **Windows** 10 e 11 (x64 e
ARM), **macOS** 11 ou mais recente e **Linux** (.deb, .rpm ou Flatpak, x64 e ARM).

- O **Windows** vai avisar que “protegeu o computador”, porque o instalador ainda
  não tem assinatura de código. Escolha **Mais informações** e depois **Executar
  assim mesmo**.
- As versões para **macOS** são assinadas e notarizadas pela Apple numa etapa à
  parte, então podem chegar depois das outras plataformas. Quando o botão do Mac
  disser “Em breve”, a versão de Mac notarizada mais recente está na página de
  releases do GitHub.
- **Linux**: para usar uma chave de segurança USB, o sistema precisa dar ao app
  acesso a ela — os pacotes .deb e .rpm instalam essa regra para você.

No macOS e no Windows, o app de desktop tem um navegador embutido para dApps. Os
checksums de cada pacote estão na
[página de releases do GitHub](https://github.com/mondaylabsltd/vela-wallet/releases) —
e dá para conferir mais do que um checksum, veja abaixo.

## iPhone e Android

Apps nativos para iOS 17.4 ou mais recente e Android 10 ou mais recente. Eles vão
ser vendidos como compra única na App Store e no Google Play; **ainda não estão nas
lojas**. O código é aberto, então você pode compilar os apps de graça — com uma
diferença: uma versão que você mesmo assina não consegue usar as passkeys do
próprio celular para carteiras do getvela.app, mas escanear com outro celular e usar
chaves de segurança USB funcionam. Veja
[compilar os apps você mesmo](/pt-BR/docs/self-hosting#web-app).

## Confira o que você baixou

Um checksum diz que dois arquivos são idênticos. Ele não diz quem fez o arquivo — e a
lista de checksums fica na mesma página do download. Por isso, todo pacote que
anexamos a uma release também é **atestado**: a execução do workflow que o compilou
assina uma declaração que nomeia o arquivo, o commit e a própria execução, e o GitHub
a guarda. Conferir isso é um comando só, com a
[CLI do GitHub](https://cli.github.com) (faça login uma vez com `gh auth login`; a
conferência é gratuita):

```bash
gh attestation verify vela-wallet_0.9.4_amd64.deb --repo mondaylabsltd/vela-wallet
```

Ela mostra quem compilou o arquivo e a partir de qual commit, ou falha. Nada no seu
computador precisa confiar na gente para essa resposta: a assinatura é do GitHub,
feita no momento da compilação, e não pode ser produzida por quem apenas reenvia um
arquivo para algum lugar.

As imagens de Mac são assinadas com o nosso Developer ID e notarizadas pela Apple, o
que o macOS confere para você ao abrir uma delas. Para perguntar você mesmo:

```bash
xcrun stapler validate VelaWallet-0.9.4-macos-arm64.dmg
spctl -a -t open --context context:primary-signature -v VelaWallet-0.9.4-macos-arm64.dmg
```

<Callout type="warning" title="O aviso do Windows continua">
Atestado não é assinatura de código. O instalador do Windows não tem assinatura de
código, então o SmartScreen continua parando ele uma vez, com “O Windows protegeu o
computador” — escolha <strong>Mais informações</strong> e depois <strong>Executar
assim mesmo</strong>. Conferir o atestado é a verificação que diz que o arquivo é
mesmo nosso; o aviso é sobre um certificado que não compramos.
</Callout>

Os pacotes publicados antes de isso ser ativado trazem só os seus checksums.

## Usando a Vela com dApps

<span id="dapps"></span>

Os dApps se conectam à Vela do mesmo jeito que se conectam a qualquer carteira de
navegador (EIP-1193 e EIP-6963):

- num navegador de computador, pela **extensão da Vela para navegador**;
- dentro do **app de desktop** (macOS, Windows), do **app de iPhone** e do **app de
  Android**, pelo navegador embutido em cada um.

A carteira web em wallet.getvela.app não se conecta a dApps, e não há suporte a
WalletConnect. Cada solicitação que um dApp faz é decodificada e mostrada a você
antes de você assinar — veja [assinatura legível](/pt-BR/docs/clear-signing).

## Do que o seu aparelho precisa

A Vela assina com **passkeys**, que quase todo aparelho dos últimos anos suporta:

| Aparelho | Compatível |
| --- | --- |
| iPhone, iPad, Mac | iOS / iPadOS 16 ou mais recente, macOS com Safari ou Chrome recente |
| Android | Um Android recente com os serviços do Google Play, ou uma chave de segurança USB |
| Windows | Windows Hello com Chrome ou Edge, ou uma chave de segurança |
| Linux | Uma chave de segurança, ou um celular por perto (escaneie o QR code) |

Se o seu aparelho não consegue guardar uma passkey, use outro celular ou uma chave
de segurança física. [Signatários e chaves de segurança](/pt-BR/docs/signers) lista
quais tipos de chave cada app aceita.

## Os únicos endereços oficiais

- **getvela.app** — este site e os downloads
- **wallet.getvela.app** — a carteira web
- **github.com/mondaylabsltd** — o código e os pacotes de cada versão

<Callout type="warning" title="Confira antes de instalar">
Se alguma coisa mandar você para outro lugar para “instalar a Vela” ou “verificar
sua carteira”, pare. A Vela nunca pede frase de recuperação — ela nem tem uma.
</Callout>

A seguir: [crie sua carteira](/pt-BR/docs/create-wallet).
