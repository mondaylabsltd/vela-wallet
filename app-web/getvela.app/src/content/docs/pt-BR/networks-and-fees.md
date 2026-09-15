---
title: Redes e taxas
description: As 12 redes que a Vela suporta, como funcionam as taxas de gas com abstração de contas, quem opera o relay e fica com as taxas, quando você paga a ativação da conta de gas, e como a Vela escolhe endpoints RPC.
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Redes e taxas

## Redes suportadas

A Vela vem com **12 redes EVM**:

| Rede | Token nativo das taxas |
| ----------- | ---------------- |
| Ethereum | ETH |
| BNB Chain | BNB |
| Polygon | POL |
| Arbitrum | ETH |
| Optimism | ETH |
| Base | ETH |
| Avalanche | AVAX |
| Gnosis | xDAI |
| Unichain | ETH |
| Tempo | USD |
| Monad | MON |
| World Chain | ETH |

Sua carteira tem **o mesmo endereço em todas elas**, então é um endereço só para
compartilhar em qualquer lugar.

Você também pode **adicionar redes personalizadas** (Configurações → Redes). Como a
Vela é uma carteira de conta inteligente, a rede precisa oferecer os contratos de
que a Vela depende: o EntryPoint ERC-4337, os contratos Safe e o precompilado de
assinatura **P-256 (RIP-7212)** que verifica sua passkey on-chain. A Vela confere
isso automaticamente antes de deixar você adicionar uma rede.

<Callout type="info" title="Por que a Gnosis aparece tanto">
Além de ser uma das 12 redes, a Gnosis Chain hospeda o **índice de passkeys** da
Vela — o contrato que guarda sua chave pública e o nome da conta para a recuperação
entre aparelhos. Isso é separado de em qual rede você transaciona.
</Callout>

## Como funcionam as taxas (abstração de contas)

A Vela usa **abstração de contas ERC-4337**, então a transação não é transmitida por
você diretamente: ela é uma **UserOperation** entregue a um **relay**, que a submete
on-chain e é reembolsado pelo gas. (A especificação ERC-4337 chama esse papel de
*bundler*. O da Vela se chama relay porque faz mais do que agrupar: cota taxas em
banda e opera o protocolo de conta de gas descrito abaixo, e nenhuma das duas coisas
faz parte do padrão.) Disso decorrem algumas coisas:

- **O gas sai do saldo da sua própria carteira** — no token nativo da rede (ETH,
  BNB, xDAI…) por padrão, ou numa stablecoin suportada onde o relay oferecer uma;
  você escolhe o ativo da taxa na tela de confirmação. A Tempo não tem moeda nativa,
  então lá o gas é sempre liquidado em stablecoins em dólar. Não há **paymaster**
  ERC-4337 patrocinando — nem barrando — cada transação. (A Vela pode bancar a
  *ativação da conta de gas*, que é uma vez só, para usuários novos; isso é outra
  coisa e está logo abaixo.)
- **É o relay que cota o preço do gas** — ele é a única fonte de verdade, e a
  carteira mostra essa cotação e assina exatamente o que mostra. Não há seletor de
  velocidade: toda transação é enviada com prioridade alta.
- O total é o **custo de rede mais a taxa de serviço do relay**, com uma taxa mínima
  pequena em transações muito baratas. A cotação do relay é o preço — não existe
  outra tabela para consultar. Uma parte vai para os validadores da chain; o resto
  paga o relay que adianta o gas e mantém a infraestrutura.
- A tela de confirmação mostra a **taxa estimada** no ativo da taxa e na sua moeda
  de exibição antes de você assinar. O valor cotado e o destinatário dele fazem
  parte do que você assina, então o relay recebe exatamente o que foi mostrado — um
  número alterado invalidaria sua assinatura.

## Quem opera o relay — e quem fica com as taxas

Cada rede aponta para um relay. Por padrão é **o relay da própria Vela**, e você
pode trocar o endpoint em _Configurações → Avançado → Endpoints de serviço_. Um
endpoint vale para todas as redes integradas; uma rede personalizada mantém a URL de
relay que você deu ao adicioná-la.

Uma ressalva honesta sobre compatibilidade: o app cota taxas por um método RPC
específico da Vela (`vela_getInBandGasQuote`), e sem ele o fluxo de envio falha.
Então o endpoint para onde você apontar precisa rodar
[vela-relay](https://github.com/mondaylabsltd/vela-relay) — a instância da Vela ou
uma que você hospede. Um bundler ERC-4337 genérico como **Pimlico** ou **Alchemy**
não implementa esse método, então não funciona de ponta a ponta na versão atual.

Quem operar o relay de uma rede **fica com as taxas daquela rede**: a margem do
relay em cada transação e o depósito de ativação da conta de gas. Rode o seu próprio
vela-relay e essas taxas financiam a sua infraestrutura em vez da da Vela; do tráfego
que você mandar para outro lugar, a Vela não tira nada.

<Callout type="warning" title="A conta de gas faz parte do protocolo vela-relay">
A etapa de **ativação da conta de gas** financia uma conta de relay dedicada à sua
carteira em cada rede. Se você apontar o endpoint para um vela-relay auto-hospedado,
o depósito financia a conta do seu próprio relay, não a da Vela.
</Callout>

### Ativando a conta de gas (Vela Relay)

No relay da Vela, sua primeira transação em cada rede **ativa uma conta de gas
dedicada**. O app primeiro pede ao caixa do relay que financie isso por você — o que
acontece em silêncio dentro do fluxo de envio, e uma carteira patrocinada nunca vê
tela de financiamento. Só quando o patrocínio é recusado é que o app mostra um
pedido de recarga: você manda uma quantia pequena do token nativo para o endereço da
conta de gas que aparece, e ele explica por que não houve patrocínio.

**Você paga a taxa de ativação** sempre que não houver patrocínio gratuito, ou seja,
quando:

- **O caixa da Vela para aquela rede está vazio ou baixo** — o fundo gratuito
  naquela chain está temporariamente esgotado.
- **Você usou toda a cota gratuita** — o patrocínio tem teto por carteira, e depois
  das primeiras vezes o custo é seu.
- **O relay da Vela não financia aquela rede** — por exemplo **redes personalizadas
  ou de teste que você mesmo adicionou**, para as quais a Vela não mantém caixa.
  (Mande-as para o seu próprio relay se preferir pular a ativação de vez.)

O depósito de ativação é **não reembolsável**: é o saldo inicial da conta de relay e
se recompõe com o tempo a partir dos reembolsos de gas, mas ainda assim pode se
esgotar e precisar de **nova ativação** depois. O endereço do relay também pode
mudar numa atualização do serviço, o que exige ativar de novo.

A taxa sai do seu saldo no **ativo da taxa** que você escolheu — o token nativo, por
padrão. Se um envio trava por gas, é porque o seu saldo naquele ativo não cobre a
taxa; onde o relay oferece gas em stablecoin, trocar o ativo na tela de confirmação
pode destravar.

Quando você envia o valor **máximo** de um token nativo, a Vela reserva
automaticamente o suficiente para o gas, para a transação não falhar.

## Como a Vela conversa com cada rede

A Vela lê saldos e envia transações por um **conjunto de endpoints RPC**, não por um
provedor só. Ela reúne endpoints de várias fontes, pontua por latência e
confiabilidade e **troca automaticamente** quando um está lento ou fora do ar —
deixando os ruins temporariamente no banco — para que um nó instável nunca derrube o
app.

A seguir: [como as passkeys funcionam](/pt-BR/docs/passkeys).
