---
title: Redes e taxas
description: "As 24 redes integradas à Vela, como adicionar outra, exatamente como a taxa de uma transação é calculada e quem a recebe, e o que acontece quando um relay fica sem gas."
source: 84328d162a3a
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Redes e taxas

## Redes integradas

A Vela tem **24 redes** integradas, todas mainnets:

| Rede | Gas pago em | Rede | Gas pago em |
| --- | --- | --- | --- |
| Ethereum | ETH | Arc | USDC (a moeda nativa) |
| BNB Chain | BNB | X Layer | OKB |
| Polygon | POL | Stable | USDT0 (a moeda nativa) |
| Arbitrum | ETH | Soneium | ETH |
| Optimism | ETH | MegaETH | ETH |
| Base | ETH | Robinhood Chain | ETH |
| Avalanche | AVAX | Mantle | MNT |
| Gnosis | xDAI | Kaia | KAIA |
| Unichain | ETH | Celo | CELO |
| Tempo | pathUSD (sem moeda nativa) | Ink | ETH |
| Monad | MON | Plume | PLUME |
| World Chain | ETH | XRPL EVM | XRP |

Na maioria delas, você também pode pagar a taxa numa stablecoin em dólar que o
relay aceite naquela rede (veja abaixo).

A sua carteira tem o **mesmo endereço em todas as redes**, porque o endereço é
calculado a partir das suas chaves, não da rede.

## Adicionar outra rede

Você pode adicionar qualquer rede EVM em **Configurações → Redes**, desde que ela
tenha o que uma carteira Vela precisa: onze contratos padrão (o EntryPoint v0.7 do
ERC-4337, os contratos do Safe v1.4.1, os módulos 4337 e de passkey da Safe,
MultiSend, Multicall3 e dois implantadores determinísticos) e o pré-compilado
**RIP-7212**, que verifica assinaturas de passkey no endereço `0x100`. A carteira
confere todos eles, inclusive com uma verificação de assinatura real no
pré-compilado, antes de deixar você adicionar a rede.

O pré-compilado é indispensável. O endereço dele faz parte de como todo
endereço da Vela é calculado, então não existe verificador alternativo nem como
implantar um depois. Se uma rede tem o pré-compilado, mas faltam alguns dos
contratos, a [configuração de rede](/pt-BR/chain-setup) mostra o que falta e
implanta o que qualquer pessoa pode implantar. Uma lacuna na verificação: uma
carteira com mais de uma chave também precisa da fábrica de signatários de passkey
da Safe na rede, e isso ainda não é verificado; sem ela, só a primeira chave
consegue assinar ali.

## Como uma transação é paga

A Vela é uma carteira ERC-4337: você não transmite a transação por conta própria. O
app monta uma **UserOperation**, você a assina com uma das suas chaves, e um
**relay** a envia on-chain e adianta o gas. (O ERC-4337 chama esse papel de
bundler.) O relay é reembolsado **dentro da sua operação**: o pagamento é uma
transferência da sua carteira para o relay que fica no mesmo lote da sua transação,
então está coberto pela sua assinatura. Não há paymaster, ninguém patrocina o seu
gas e ninguém pode recusar a sua transação por causa de uma política de
patrocínio.

### Qual é a taxa

A tela de confirmação mostra um único valor, na moeda da taxa e na sua moeda de
exibição. Ele é calculado assim:

- **Gas que a carteira reserva.** A carteira simula a transação e reserva mais gas
  do que espera usar: as estimativas de verificação e de execução são aumentadas
  em metade cada uma, com mínimos (por exemplo, a verificação é de pelo menos
  300.000 unidades de gas quando a carteira já está implantada, e 2.000.000 na transação que
  a implanta).
- **Preço do gas.** O maior entre a leitura que a própria carteira faz do preço de
  gas da rede e o preço do relay para a velocidade que você escolheu. A velocidade
  padrão é *Rápido*, que o relay precifica em cerca de 1,8 × a taxa base mais o
  dobro da taxa de prioridade.
- **Taxa = 3 × gas reservado × preço do gas**, com mínimo de cerca de US$ 0,01. Na
  Tempo, o multiplicador é 2 e a taxa é paga em pathUSD.

Como a reserva fica bem acima do que a transação vai usar e o preço tem folga,
**a taxa muitas vezes é dez vezes ou mais o custo real da transação on-chain**, e
ainda maior na primeira transação numa rede. O relay paga o custo real e fica com
o restante; nada é devolvido. Em redes baratas, isso dá centavos; na mainnet do
Ethereum, pode ser um valor considerável. O valor exato aparece na tela de
confirmação antes de você assinar.

<Callout type="info" title="O que você vê é o que você paga">
O valor da taxa e o endereço para onde ela vai fazem parte da operação que você
assina. Um relay que mudasse qualquer um dos dois invalidaria a sua assinatura,
então você paga exatamente o valor mostrado — nada a mais, mesmo que o
gas suba antes da inclusão. Uma cotação de preço de gas do relay acima do triplo da
leitura da própria carteira é recusada.
</Callout>

### Com o que você pode pagar

- A **moeda nativa** da rede, sempre.
- Uma **stablecoin em dólar** da lista do relay para aquela rede, quando o relay
  consegue precificar a moeda nativa. Stablecoins que você não tem ficam ocultas.
- Na **Tempo**, que não tem moeda nativa, só **pathUSD**.

Você escolhe a moeda da taxa e a velocidade (*Lento*, *Padrão* ou *Rápido*) na tela
de confirmação e nas Configurações.

### A sua primeira transação numa rede

Você pode receber em qualquer rede antes de a sua carteira existir nela. Na
primeira vez que você envia algo numa rede, essa transação também implanta o
contrato da sua carteira (e um pequeno contrato signatário para cada chave extra).
O gas da implantação entra na taxa dessa transação, então o primeiro envio em cada
rede custa mais do que os seguintes.

Quando você envia o **máximo** de uma moeda nativa, a Vela reserva o suficiente
para a taxa.

## Quem opera o relay — e quem fica com a taxa

Por padrão, todas as redes usam o **relay da Vela**, e a taxa vai para a Vela. Você
pode apontar a carteira para outro relay em **Configurações → Avançado → Endpoints
de serviço**; um único endereço atende todas as redes integradas, e uma rede
personalizada mantém o endereço de relay com que foi adicionada. O relay precisa
ser o [vela-relay](https://github.com/mondaylabsltd/vela-relay) — o da Vela ou um
que você rode —, porque a carteira pede a cotação da taxa com um método específico
da Vela que bundlers genéricos, como Pimlico ou Alchemy, não implementam. Quem opera
o relay que você usa recebe a taxa; o
[guia de auto-hospedagem](/pt-BR/docs/self-hosting#relay) explica como rodar um.

O relay recebe uma operação que já está assinada. Ele não consegue mudar o
destinatário, o valor, a taxa nem nada mais. Ele pode atrasá-la ou recusá-la, e
escolhe quando ela entra na rede — então, num swap, ele poderia, em tese, negociar
na sua frente dentro da sua tolerância de slippage.

### Quando um relay fica sem gas

Um relay paga o gas com a própria **tesouraria** em cada rede. Se essa tesouraria
estiver vazia, a tela de envio avisa você antes de assinar:

- Numa rede atendida pelo relay da Vela, quem opera o relay (a Vela) precisa
  reabastecê-la; você pode relatar o problema. Se não puder esperar, você pode,
  **se quiser**, enviar você mesmo uma pequena quantia da moeda nativa para a
  tesouraria. Essa contribuição **não é reembolsável** e **não** paga a sua própria
  transação.
- Numa rede personalizada, abastecer o relay é responsabilidade de quem o opera — que
  pode ser você.

Não existe conta de gas por carteira nem depósito de ativação: uma versão anterior
da Vela tinha isso, e não existe mais.

## Como a Vela lê cada rede

A Vela lê saldos e simula transações por meio de um **conjunto de endpoints RPC**
em cada rede — os integrados, alternativas públicas e quaisquer chaves de provedor
ou endpoints que você adicionar — e passa para o próximo quando um endpoint está
lento ou fora do ar. Você pode definir seu próprio endpoint para cada rede em
**Configurações → Redes**. (O app de Android hoje usa um único endpoint por rede,
sem troca automática, e o app de iPhone ainda não deixa você mudá-lo.)

A seguir: [como as passkeys funcionam](/pt-BR/docs/passkeys).
