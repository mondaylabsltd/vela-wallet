---
title: Recuperação e login
description: Como a Vela permite recuperar sua carteira em um aparelho novo sem frase-semente — e os limites honestos desse modelo.
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Recuperação e login

A parte mais difícil de uma carteira sem frase-semente é a recuperação: se não há
doze palavras, como você volta em um celular novo? Aqui está exatamente como a Vela
resolve isso.

## Como funciona

Quando você cria uma carteira, duas coisas são publicadas no **índice de passkeys**
da Vela:

- A **chave pública** da sua passkey (nunca a privada).
- O **nome** que você escolheu para a carteira.

A chave pública fica armazenada na blockchain Gnosis por meio de um contrato, então
é publicamente legível e não depende de os servidores da Vela continuarem no ar.

Sua chave **privada**, por sua vez, é uma passkey sincronizada pelo chaveiro da sua
plataforma: **Chaveiro do iCloud** em aparelhos Apple, **Gerenciador de senhas do
Google** no Android.

Para entrar em um aparelho novo:

1. Entre na mesma conta iCloud ou Google, com a sincronização do chaveiro ativada.
2. Abra a Vela e escolha entrar.
3. Autentique-se com sua passkey. A plataforma fornece a passkey sincronizada; o
   índice fornece a conta correspondente. Sua carteira está de volta.

O índice é um cache, não um ponto único de falha. Se um dia ele ficar inacessível e
sua conta não estiver no armazenamento local, a Vela consegue reconstruir sua chave
pública no próprio aparelho a partir de duas assinaturas de passkey e derivar de
novo o endereço da carteira — sem servidor nenhum envolvido.

<Callout type="info" title="Por que separar assim">
A chave pública no índice on-chain deixa qualquer pessoa (inclusive uma instalação
recém-feita) encontrar sua conta. A chave privada, sincronizada pelo chaveiro da
plataforma em que você confia, é o que de fato autoriza transações. Tudo o que está
no índice é dado público, e nada ali dentro pode mover seus fundos — só assinaturas
da sua passkey podem.
</Callout>

## Os limites, sem enfeite

Autocustódia significa que a responsabilidade é sua de verdade. É isto que você
precisa entender.

<Callout type="warning" title="Sua recuperação depende do chaveiro da plataforma">
O login entre aparelhos da Vela depende de a sua passkey ser sincronizada pelo
Chaveiro do iCloud ou pelo Gerenciador de senhas do Google. Mantenha essa conta
segura e as opções de recuperação dela em dia. Se você perder <strong>tanto</strong>
seus aparelhos <strong>quanto</strong> o chaveiro da conta da plataforma, a Vela não
tem como regerar sua chave privada — por projeto, nunca a tivemos.
</Callout>

Orientação prática:

- **Deixe a sincronização do chaveiro ligada.** É ela que leva sua passkey de um
  aparelho para outro.
- **Proteja sua conta Apple / Google** com uma senha forte e métodos de recuperação
  próprios. Essa conta agora faz parte da segurança da sua carteira.
- **Mantenha mais de um aparelho logado** onde der, para que perder um celular seja
  um transtorno e não uma crise.

## O que a Vela pode e o que não pode

- **Pode:** ajudar você a achar sua conta de novo pelo índice público.
- **Não pode:** mover seus fundos, congelar sua carteira ou recuperar uma chave
  privada. A Vela nunca a teve. É esse o sentido inteiro da autocustódia — e a troca
  que você aceita por ela.
