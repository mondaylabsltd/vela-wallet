---
title: Enviar e receber
description: Como receber e enviar tokens na Vela — um endereço só em todas as redes, transações assinadas de forma legível, e como a abstração de contas realmente move seus fundos.
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Enviar e receber

## Receber

1. Abra sua carteira e toque em **Receber**.
2. Compartilhe seu endereço: copie ou deixe quem envia escanear o QR code.
3. Quando a transferência confirmar on-chain, o saldo aparece na sua carteira.

Duas coisas que vale saber:

- Seu endereço é **o mesmo em todas as redes suportadas**, então você compartilha um
  endereço só; apenas confirme que quem envia está usando a rede certa.
- Dá para **receber antes de a carteira estar implantada**. As contas da Vela são
  contas inteligentes contrafactuais: os fundos podem chegar ao seu endereço antes
  de o contrato existir naquela chain; ele se implanta sozinho no seu primeiro envio
  ali.

## Enviar

1. Toque em **Enviar** e escolha o **token**.
2. Digite o **valor** (dá para alternar entre o token e sua moeda de exibição) e o
   **destinatário**. Onde consegue, a Vela resolve destinatários conhecidos para um
   nome: uma conta Vela, um nome ENS, um Basename e assim por diante.
3. **Revise e confirme.** A Vela mostra a transferência e então pede sua passkey
   (Face ID / Touch ID / digital).

### O que acontece quando você confirma

A Vela não simplesmente «transmite» uma transação. Por baixo:

1. Ela monta uma **UserOperation** ERC-4337 para a sua conta Safe.
2. Seu aparelho a assina com uma asserção **WebAuthn (P-256)** depois da checagem
   biométrica.
3. A operação assinada vai para o **relay**, que a envia ao EntryPoint; seu Safe
   verifica a assinatura P-256 **on-chain** e executa.

<Callout type="info" title="O relayer não consegue mexer na sua transação">
O relay recebe uma UserOperation <strong>já assinada</strong>. Ele pode atrasar ou
se recusar a repassar, mas não pode mudar destinatário, valor ou qualquer outro
campo — qualquer alteração invalida sua assinatura. É um facilitador de entrega, não
um custodiante, e é código aberto, então você pode rodar o seu.
</Callout>

### Assinatura legível — nada de aprovar às cegas

Antes de você assinar, a Vela decodifica a transação com descritores **ERC-7730** e
mostra a **intenção** (Enviar, Aprovar, Trocar…), os **valores e endereços** e uma
indicação de risco — não hexadecimal opaco. Quando não consegue decodificar por
completo uma chamada, ela mostra um **aviso explícito de assinatura às cegas** em
vez de fingir que entendeu. Uma aprovação de token ilimitada não é só sinalizada: a
Vela a reescreve para um valor finito e se recusa a enviar uma aprovação que
continuasse ilimitada.

## Antes de apertar enviar

- **Confira os primeiros e os últimos caracteres do endereço.** Malware que troca
  endereços existe mesmo.
- **Confirme a rede.** Enviar na rede errada é o erro caro mais comum. Veja
  [redes e taxas](/pt-BR/docs/networks-and-fees).
- **Comece pequeno com destinatários novos.** Uma transferência de teste minúscula é
  um seguro barato.

Transações são irreversíveis. Não existe atendimento que traga de volta um envio
para o endereço errado — é assim a autocustódia.

## Lendo seu histórico

Saldos e histórico são lidos ao vivo de um conjunto de endpoints RPC públicos, com
failover automático. Se a rede estiver lenta, o histórico pode demorar um pouco —
um indicador girando quer dizer «ainda buscando», não «os fundos sumiram».
