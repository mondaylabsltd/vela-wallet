---
title: Trusted Signer
description: "Uma página de arquivo único em sign.getvela.app que decodifica uma solicitação e a assina com a sua passkey por conta própria — o que ela confere, quais apps a usam e como recompilá-la ou rodar a sua própria cópia."
source: 43a1af6ffdcd
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Trusted Signer

A Vela decodifica cada transação antes de você aprová-la, e essa decodificação é um
trabalho honesto — mas é um trabalho feito pelo mesmo app que montou a transação. Se
o app, ou o caminho pelo qual ele chega até você, for adulterado, ele pode mostrar
uma coisa e assinar outra. Foi exatamente o que aconteceu com a
[Bybit](/pt-BR/docs/bybit-attack).

O Trusted Signer existe para dividir isso em dois: o app só entrega a solicitação, e
a conferência e a assinatura acontecem numa página separada — uma que você pode ler
do começo ao fim, recompilar byte a byte ou rodar você mesmo.

## Onde roda

A página oficial é servida em **sign.getvela.app**. Os apps de desktop (macOS,
Windows, Linux), iPhone e Android podem enviar uma solicitação a ela: o app abre a
página numa aba do navegador com a solicitação no link, você confere e assina com a
sua passkey ali mesmo, e a página devolve a assinatura ao app por um link
`velawallet://`. A carteira web não consegue usá-lo.

É opcional. Você o escolhe como a sua forma de assinar ao criar uma carteira ou
fazer login, e a partir daí toda assinatura dessa carteira nesse aparelho passa por
ele. Ele também pode criar as chaves da carteira. Em `sign.getvela.app`, ele usa as
mesmas passkeys de `getvela.app` que os apps.

<Callout type="info" title="O que já foi testado">
Execuções de ponta a ponta registradas com a página publicada: Android e Windows 11
(criar uma carteira e fazer login). Os apps de macOS, Linux e iPhone usam a mesma
integração; nenhum deles tem ainda uma execução completa registrada.
</Callout>

## O que ele faz antes de assinar

- **Decodifica a solicitação por conta própria.** O que a chamada faz, para quem e
  de quanto, a partir da calldata — inclusive chamadas aninhadas dentro de um lote.
- **Só assina um digest que ele mesmo calculou.** Os digests EIP-191, EIP-712,
  SafeOp e SafeMessage são calculados na página, nunca recebidos de quem faz a
  solicitação; testes conferem os digests SafeOp e SafeMessage com o `vela-core`, o
  código que a carteira usa, e o app recusa uma assinatura sobre qualquer digest
  diferente do que ele mesmo calculou.
- **Confere se a transação é a que foi solicitada.** A chamada que o site pediu
  precisa estar de fato dentro da operação que está sendo assinada, ou a página
  recusa.
- **Diz quando uma aprovação é ilimitada.** Ele não consegue mudar um valor — ou
  assina os bytes que chegaram, ou nada —, então uma aprovação ou um permit ilimitado
  (2^128 ou mais nesta página) aparece em vermelho com esse motivo e pode ser
  assinado como está; um limite on-chain é escolhido na própria tela de aprovação da
  carteira, antes de a solicitação chegar aqui. Uma aprovação para uma coleção
  inteira de NFTs é recusada.
- **Recusa o que não pode sustentar:** `eth_sign`, um método que ele não conhece, um
  token enviado ao contrato do próprio token, uma operação que ele não consegue ler e
  um login cujo desafio (challenge) foi fornecido por quem fez a solicitação.
- **Recusa o que entregaria a sua conta,** pela mesma regra que os apps aplicam:
  uma chamada da sua conta a uma das próprias funções de proprietários, módulos,
  guard ou fallback, inclusive dentro de um lote; um `delegatecall`, exceto para o
  contrato MultiSend da Safe, que agrupa as chamadas de uma operação; e uma
  assinatura `SafeTx`. Ele verifica cada chamada da operação montada pelo app, não
  só as que o site pediu.
- **Mostra o endereço da conta e um identicon calculado na página.** Destinatários e
  contratos nunca recebem nome a partir da solicitação — só a tabela revisada da
  própria página pode dar nome a um contrato. O nome da própria conta, que o app
  envia para você escolher a passkey certa, aparece ao lado do endereço dela.
- **Pede verificação do usuário** (sua digital, seu rosto ou seu PIN) em toda
  assinatura.

## O que ele deliberadamente não tem

- **Nenhum editor.** A solicitação fica fixa quando chega: você assina ou não
  assina. Um seletor de taxa ou um editor de limite reescreveriam a calldata, que é
  justamente a doença que esta página existe para evitar.
- **Nenhum acesso à rede.** A página é um único arquivo cuja política de segurança de
  conteúdo (`default-src 'none'`) está dentro dos próprios bytes, então ela não
  consegue buscar nada, abrir uma conexão nem carregar uma imagem. A única coisa que
  sai dela é a resposta, quando ela segue o link de callback da solicitação
  (`velawallet://` quando quem pediu foi um app da Vela). Os logos dos tokens são
  desenhados como letras.

## O que o app confere em troca

O app também não confia na página. Ele só aceita uma assinatura quando o desafio
assinado é o digest **que o app calculou**, a verificação do usuário foi feita, a
chave é uma das chaves da sua carteira e a assinatura P-256 é válida para essa chave.

## Cada versão publicada, verificável

Cada versão é compilada a partir de `app-web/trusted-signer/src/` num único arquivo,
de forma reproduzível — Bun e Node produzem os mesmos bytes — e publicada no seu
próprio endereço, `sign.getvela.app/b/<sha256>/sign.html`, ao lado de todas as
versões anteriores. A lista fica em `sign.getvela.app/index.json`.

```sh
cd app-web/trusted-signer
node samples/build-single.mjs --check   # rebuilds a version listed in dist/
curl -sL https://sign.getvela.app/b/<sha256>/sign.html | shasum -a 256
```

Ao iniciar, o app de desktop baixa a versão publicada que vai abrir, calcula o hash
dela e o compara com as versões embutidas nele. O resultado só vai para o log, e uma
página que não corresponde é aberta mesmo assim. Os apps de celular ainda não fazem
essa conferência.

## Rode a sua própria cópia

As Configurações guardam o endereço da página que os seus apps abrem, então você pode
apontá-lo para a sua própria implantação: qualquer endereço HTTPS, ou `localhost`
para testes. Compile com `bun samples/build-single.mjs` (ou `node`) e copie `dist/`
para o seu host.

Uma cópia no seu próprio domínio assina com passkeys criadas para **aquele** domínio,
não com as passkeys de `getvela.app` — então é uma forma de criar e usar uma carteira
cujas chaves ficam sob o seu domínio, não uma forma de assinar por uma carteira
existente do `getvela.app`. Todas as chaves de uma carteira compartilham um mesmo
domínio.

O código, e os scripts que o compilam e o conferem, estão em
`app-web/trusted-signer/`.
