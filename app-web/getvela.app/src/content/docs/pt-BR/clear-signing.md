---
title: Assinatura legível
description: "A Vela decodifica as transações em linguagem clara antes de você aprovar — intenção, valores, endereços e risco — em vez de um hexadecimal opaco. Quando não consegue decodificar uma chamada, ela avisa em vez de fingir que entendeu."
source: 858d8631b7e5
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Assinatura legível

Muitas carteiras ainda mostram dados crus para qualquer contrato que não
reconhecem, e a “assinatura às cegas” — aprovar chamadas que você não consegue ler
de verdade — é uma das formas pelas quais carteiras são esvaziadas. A resposta da
Vela é a **assinatura legível**: antes de você assinar, a transação é decodificada
em algo que você consegue entender, até onde isso é possível.

## O que você vê

Em vez de calldata crua, a Vela mostra:

- **Intenção** — o que a transação faz: *Enviar*, *Aprovar*, *Trocar* e assim por
  diante.
- **O essencial** — os valores e os endereços envolvidos, com os valores de tokens
  em unidades reais e os destinatários identificados por nome, quando existe um.
- **Os detalhes** — nonce, prazos e a calldata crua, disponíveis quando você quiser,
  em vez de jogados na sua cara.
- **Uma indicação de risco**, com cores, para que as ações perigosas se destaquem.

## Como funciona (ERC-7730)

A Vela decodifica tanto **chamadas de contrato** quanto **dados tipados EIP-712**
usando descritores
[ERC-7730](https://github.com/LedgerHQ/clear-signing-erc7730-registry) —
definições pequenas e compartilháveis do que as funções de um contrato significam.

A Vela procura um descritor nesta ordem:

1. **Integrado ao app** — descritores de contratos muito usados: os roteadores do
   Uniswap, PancakeSwap e SushiSwap, WETH, o pool do Aave v3, 1inch, Lido e wstETH,
   e Seaport.
2. **Buscado no servidor de dados de chain da Vela**, que republica o registro
   público do ERC-7730.
3. **Formatos padrão** — tokens ERC-20, NFTs ERC-721 e ERC-1155, cofres ERC-4626 e
   permits ERC-2612 —, para que a maioria das ações do dia a dia continue sendo
   decodificada.

Quando um descritor escrito para aquele contrato específico corresponde, a
transação recebe o rótulo **Verificado**, com o nome do contrato. “Verificado”
significa *foi encontrado um descritor para este contrato*, não que ele tenha sido
conferido criptograficamente: os descritores buscados no servidor de dados de chain
não são assinados, então são tão confiáveis quanto esse servidor — e esse é um dos
motivos pelos quais você pode [rodar o seu](/pt-BR/docs/self-hosting#chain-data).

Os valores de tokens são formatados com as **casas decimais reais do token
on-chain**. Se a Vela não consegue confirmar as casas decimais de um token, ela
mostra o valor como se o token tivesse 18 e **o marca como não verificado**, para
que um número errado nunca pareça conferido.

## Níveis de risco

Toda transação decodificada recebe um nível de risco, para que os padrões perigosos
se destaquem:

- **Atenção** para aprovações e permits — você está concedendo poder de gasto.
- **Perigo** para o que é arriscado de verdade, como uma **aprovação ilimitada de
  token**.
- Risco menor para ações rotineiras, como staking ou depósito.

<Callout type="warning" title="Aprovações on-chain “ilimitadas” não podem ser enviadas">
Uma aprovação on-chain de valor ilimitado é uma das formas mais comuns de os fundos
serem esvaziados mais tarde. Quando um dApp pede uma (<code>approve</code>,
<code>increaseAllowance</code> ou o <code>approve</code> do Permit2) no nível
“ilimitado” — 2^200 ou mais (2^152 no Permit2), que é o que os dApps usam para
“ilimitado” —, a Vela não a envia até que você a mude para um valor específico, para
o seu saldo ou para uma revogação; uma última verificação antes do envio lê a
calldata crua, então funciona com ou sem descritor. O que ela não bloqueia: uma
<strong>aprovação finita alta</strong> (mesmo muito acima do seu saldo),
<strong>permits assinados</strong> (assinaturas EIP-2612 e Permit2) e o
<code>setApprovalForAll</code> de NFTs — cada um aparece com um alerta, e a decisão é
sua.
</Callout>

## Quando a Vela não consegue decodificar uma chamada

Quando não existe descritor ERC-7730, mas a função aparece num banco de dados
público de seletores, a Vela decodifica a chamada de forma genérica e a marca como
**melhor esforço** — decodificada, mas não verificada — sob um banner de atenção. Se
nem isso funcionar, ou se a Vela só conseguir decodificar parte de uma transação,
ela **não** finge que entende.

<Callout type="danger" title="Aviso explícito de assinatura às cegas">
Se uma chamada não pode ser decodificada, a Vela mostra um aviso claro de
assinatura às cegas em vez de um resumo falsamente amigável. Se ela só consegue
resolver alguns dos campos, avisa que a visualização é parcial e mantém o nível de
risco elevado. Você sempre sabe quanto do que está assinando a Vela conseguiu ler
de fato.
</Callout>

## Por que isso importa

Autocustódia significa que ninguém pode reverter uma transação ruim por você. A
defesa não é uma central de suporte — é entender o que você aprova **antes** de
aprovar. A assinatura legível é a forma como a Vela tenta mostrar isso, e ela tem
limites: só pode ser tão honesta quanto o app que a exibe, e é por isso que uma
[conferência independente](/pt-BR/docs/clear-signing-self-host) importa. Veja o
[whitepaper](/pt-BR/docs/whitepaper) para entender onde ela se encaixa no modelo de
segurança da Vela.
