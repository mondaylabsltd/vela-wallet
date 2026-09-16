---
title: Assinatura legível
description: A Vela decodifica as transações em linguagem clara antes de você aprovar — intenção, valores, endereços e risco — em vez de hexadecimal opaco. Quando não consegue decodificar uma chamada, ela avisa em vez de fingir.
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Assinatura legível

A maioria das carteiras pede que você aprove um paredão de hexadecimal e torça pelo
melhor. A «assinatura às cegas» — aprovar chamadas que você não consegue ler — está
por trás de boa parte das carteiras esvaziadas. A resposta da Vela é a **assinatura
legível**: antes de assinar, a transação é traduzida em algo que você entende.

## O que você vê

No lugar da calldata crua, a Vela mostra:

- **Intenção** — o que a transação faz: *Enviar*, *Aprovar*, *Trocar* e por aí vai.
- **A substância** — os valores e endereços envolvidos, com valores de token em
  unidades reais e destinatários resolvidos para um nome quando existe.
- **Os detalhes** — nonce, prazos e a calldata crua, disponíveis quando você pedir
  em vez de jogados na sua cara.
- **Uma indicação de risco**, com cores, para que o assustador pareça assustador.

## Como funciona (ERC-7730)

A Vela decodifica tanto **chamadas de contrato** quanto **dados tipados EIP-712**
usando descritores
[ERC-7730](https://github.com/LedgerHQ/clear-signing-erc7730-registry): definições
pequenas e compartilháveis do que as funções de um contrato significam.

- Quando existe um **descritor específico do contrato**, a transação é marcada como
  **verificada** e recebe o nome do contrato.
- Quando não existe, a Vela cai em **descritores padrão** para formatos comuns —
  tokens ERC-20, NFTs ERC-721, cofres ERC-4626 e permits ERC-2612 — de modo que a
  maioria das ações do dia a dia ainda é decodificada.

Valores de token são formatados com as **casas decimais reais on-chain** do token. A
Vela nunca simplesmente assume 18; se não consegue confirmar as casas, mostra o
valor mas **sinaliza como não verificado**, em vez de chutar.

## Níveis de risco

Toda transação decodificada ganha um nível de risco, para que os padrões perigosos
saltem aos olhos:

- **Atenção** para aprovações e permits: você está concedendo poder de gasto.
- **Perigo** para o que é realmente arriscado, como uma **aprovação de token
  ilimitada**.
- Risco menor para ações rotineiras como staking ou depósito.

<Callout type="warning" title="Aprovações ilimitadas são bloqueadas">
Um «approve» que concede permissão ilimitada é uma das formas mais comuns de os
fundos serem drenados mais tarde. A Vela faz mais do que sinalizar: ela reescreve o
pedido para um valor finito escolhido por você, e uma checagem final antes do envio
se recusa a mandar qualquer aprovação que continuasse ilimitada. Essa trava lê a
calldata crua diretamente, então funciona mesmo quando não existe descritor para o
contrato.
</Callout>

## Quando a Vela não consegue decodificar uma chamada

Honestidade importa mais do que uma tela limpa. Quando não há descritor ERC-7730 mas
a função aparece num banco público de seletores, a Vela decodifica a chamada de
forma genérica e a rotula como **melhor esforço** — decodificada, mas não
verificada — sob um aviso de atenção. Se nem isso der certo, ou se a Vela só
conseguir decodificar parte da transação, ela **não** finge ter entendido.

<Callout type="danger" title="Aviso explícito de assinatura às cegas">
Se uma chamada não pode ser decodificada, a Vela mostra um aviso claro de assinatura
às cegas em vez de um resumo falsamente simpático. Se ela só consegue resolver
alguns campos, avisa que a visão é parcial e mantém o nível de risco alto. Você
sempre sabe quanto do que está assinando a Vela realmente conseguiu ler.
</Callout>

## Por que isso importa

Autocustódia significa que ninguém pode reverter uma transação ruim por você. A
defesa não é uma central de atendimento — é entender o que você aprova **antes** de
aprovar. A assinatura legível transforma «confie neste bloco opaco» em «é isto que
ele faz, exatamente». Onde isso se encaixa no modelo de segurança geral está no
[whitepaper](/pt-BR/docs/whitepaper).
