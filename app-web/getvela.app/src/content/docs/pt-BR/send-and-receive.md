---
title: Enviar e receber
description: "Como receber e enviar com a Vela — um endereço em todas as redes, envio para uma ou várias pessoas, de onde vêm os nomes dos destinatários, o que você confirma e como o relay movimenta seus fundos."
source: c23b205bcd8b
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Enviar e receber

## Receber

1. Abra a sua carteira e toque em **Receber**.
2. Compartilhe o seu endereço — copie ou mostre o QR code. A carteira web também
   consegue gerar uma solicitação de pagamento com o valor incluído.
3. Quando a transferência é confirmada on-chain, ela aparece no seu saldo.

- O seu endereço é **o mesmo em todas as redes**, então você passa um endereço só —
  mas quem envia ainda precisa usar uma rede que a Vela suporta, ou uma que você
  adicionou.
- Você pode **receber antes de a carteira estar implantada** numa rede. Ela se
  implanta sozinha no seu primeiro envio nessa rede.

## Enviar

1. Toque em **Enviar** e escolha o **token**.
2. Informe o **valor** (no token ou na sua moeda de exibição) e o **endereço do
   destinatário**, colando, escaneando um QR code ou escolhendo um contato.
3. **Revise.** A Vela mostra o que vai acontecer, a taxa e o nome que encontrou para
   o destinatário, se houver.
4. **Confirme** com uma das suas chaves — Face ID, digital, PIN, ou um toque e o PIN
   na sua chave de segurança.

### Enviar para várias pessoas, ou consolidar

- **Dividir** — envie um token para várias pessoas numa única transação. Você pode
  colar uma lista ou importar uma planilha, e informar os valores na sua moeda.
- **Consolidar** — envie vários tokens para um único endereço numa única transação.

Nos dois casos você assina uma vez só, e a transação paga uma única taxa.

### Nomes para endereços

Quando você informa um endereço, a Vela procura um nome para ele: primeiro no seu
próprio registro (o nome de outra carteira Vela), depois nos registros reversos de
`.bnb`, `.arb`, `.g`, Basename e ENS, lidos direto de cada rede. Isso funciona num
sentido só — dá nome a um endereço que você informou. Digitar um nome como
`alice.eth` não busca um endereço. Os seus **contatos** salvos também mostram seus
nomes. Trate um nome como uma pista, não como prova: um registro reverso ou o nome
de uma carteira Vela é escolhido por quem controla aquele endereço.

### Moeda da taxa e velocidade

A tela de confirmação mostra a taxa na moeda da taxa e na sua moeda. Você pode pagar
na moeda da rede ou, onde o relay aceitar, numa stablecoin em dólar, e escolher a
velocidade (padrão: *Rápido*). Quando você envia o **máximo** de uma moeda nativa, a
Vela reserva o suficiente para a taxa. [Como a taxa é calculada](/pt-BR/docs/networks-and-fees).

### O que acontece quando você confirma

1. A Vela monta uma **UserOperation** ERC-4337 para o seu Safe, incluindo o
   pagamento da taxa ao relay.
2. A sua chave verifica você e assina a operação com uma asserção **WebAuthn
   (P-256)**.
3. A operação assinada vai para o **relay**, que a envia ao EntryPoint; o seu Safe
   confere a assinatura P-256 on-chain e executa.

<Callout type="info" title="O relay não consegue alterar sua transação">
O relay recebe uma operação que já está assinada. Ele não consegue mudar o
destinatário, o valor nem a taxa — qualquer mudança invalida a sua assinatura. Ele
pode atrasá-la ou recusá-la, e decide quando ela entra na rede. O relay é de código
aberto, e você pode [rodar o seu](/pt-BR/docs/self-hosting#relay).
</Callout>

Antes de você assinar, a Vela decodifica o que a transação faz e avisa sobre o que
não consegue decodificar; veja [assinatura legível](/pt-BR/docs/clear-signing).

## Antes de apertar enviar

- **Confira o começo e o fim do endereço.** Malware que troca endereços existe de
  verdade, assim como endereços parecidos plantados no seu histórico.
- **Confirme a rede.** Enviar pela rede errada é um erro comum, e caro.
- **Comece com pouco para um destinatário novo.** Uma transferência de teste bem
  pequena é um seguro barato.

As transações são irreversíveis. Ninguém consegue desfazer um envio para o endereço
errado — faz parte da autocustódia.

## Sua atividade

A sua atividade junta o que você enviou deste aparelho com as transferências de
tokens lidas dos logs de cada rede. Uma transferência simples de moeda nativa para
você que chega por outro contrato (por exemplo, alguns saques de exchange) pode não
gerar log em algumas redes, então ela pode aparecer no saldo sem aparecer na
atividade. Os saldos são lidos em tempo real por um conjunto de endpoints RPC com
troca automática em caso de falha; um indicador de carregamento significa “ainda
buscando”, não “o dinheiro sumiu”.

A seguir: [redes e taxas](/pt-BR/docs/networks-and-fees).
