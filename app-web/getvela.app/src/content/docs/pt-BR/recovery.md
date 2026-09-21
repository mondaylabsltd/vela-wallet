---
title: Recuperação e login
description: "Como voltar à sua carteira num aparelho novo com qualquer uma das suas chaves, onde a carteira é procurada e os limites honestos da recuperação sem frase de recuperação."
source: 77cf3f24c7c7
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Recuperação e login

Sem frase de recuperação, a recuperação depende de duas coisas: **uma chave que você
ainda tem** e **um registro público de quais chaves pertencem à sua carteira**.

## O que fica registrado quando você cria uma carteira

O endereço da sua carteira é calculado a partir de todas as chaves com que você a
cria. Para que qualquer uma dessas chaves consiga encontrar a carteira de novo mais
tarde, criar uma carteira grava um registro num **contrato de registro** público na
Gnosis Chain: a chave pública de cada chave, o endereço da carteira, o nome dela e
os dados de registro assinados. O contrato de registro não tem dono, e o registro
não pode ser editado nem apagado. (A lista completa do que é público está em
[criar sua carteira](/pt-BR/docs/create-wallet#what-is-public).)

O serviço de índice de chaves públicas da Vela envia esse registro e paga o gas; o
registro em si fica on-chain, e qualquer app pode lê-lo diretamente.

## Fazer login num aparelho novo

1. Abra a Vela e escolha entrar.
2. Use **qualquer uma** das suas chaves: uma passkey sincronizada com este
   aparelho, um celular por perto (escaneie o QR code) ou a sua chave de segurança.
3. A Vela descobre a chave pública dessa chave a partir da assinatura e a procura —
   primeiro no índice da Vela e, se o índice não responder, no contrato de registro
   na Gnosis e depois no Ethereum — e reconstrói a carteira. Antes de mostrá-la a
   você, ela confere se as chaves encontradas realmente resultam no endereço
   registrado.

As listas de contas não são sincronizadas entre aparelhos; fazer login as
reconstrói.

<Callout type="info" title="Se nenhum índice nem registro responder">
Uma carteira com <strong>uma única chave</strong> pode ser reconstruída no
aparelho sem envolver nenhum servidor: duas assinaturas dessa chave bastam para
recuperar a chave pública dela e recalcular o endereço. Uma carteira com várias
chaves precisa do registro, porque uma chave não consegue dizer ao app quais eram as
outras.
</Callout>

## Cópias do registro

O registro na Gnosis é o primeiro que os apps leem. Em **Configurações**, você
também pode copiar o registro da sua carteira para o mesmo contrato de registro no
**Ethereum**, pagando o gas você mesmo, para que o registro exista numa segunda
rede. Qualquer pessoa pode fazer essa cópia; ela não contém nada que permita mover
fundos.

## Os limites honestos

<Callout type="warning" title="Uma chave perdida está perdida">
Se todas as chaves com que você criou a carteira sumiram — as passkeys
sincronizadas, os celulares, as chaves de segurança —, ninguém consegue recuperar a
carteira: nem a Vela, nem a Apple ou o Google, nem ninguém. Não há frase de
recuperação, nem redefinição pelo suporte, nem porta dos fundos.
</Callout>

O que torna isso improvável é ter mais de um jeito de entrar:

- **Mantenha a sincronização de passkeys ativada**, se você usa a passkey deste
  aparelho. É ela que leva a chave para um celular ou computador novo.
- **Proteja a conta por trás dela.** Quem controla a sua conta Apple ou Google
  talvez consiga usar uma passkey sincronizada; dê a essa conta uma senha forte e
  opções de recuperação próprias.
- **Crie a carteira com mais de uma chave**, por exemplo a passkey do seu celular e
  uma chave de segurança física guardada num lugar seguro. As chaves só podem ser
  adicionadas quando você cria a carteira ([por quê](/pt-BR/docs/signers)).
  Lembre-se de que qualquer chave sozinha pode assinar — e não pode ser removida;
  então, se uma delas for comprometida, transfira seus fundos para uma carteira nova
  ([o que fazer](/pt-BR/docs/signers)).

## O que a Vela pode e não pode fazer

- **Pode:** manter o índice funcionando, para que a sua carteira seja encontrada
  rapidamente num aparelho novo.
- **Não pode:** mover seus fundos, congelar sua carteira, adicionar ou remover
  chaves, nem recuperar uma chave que você perdeu. A Vela nunca guarda suas chaves.

A seguir: [assinatura legível](/pt-BR/docs/clear-signing).
