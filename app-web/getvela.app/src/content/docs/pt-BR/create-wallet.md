---
title: Criar sua carteira
description: "Crie uma carteira Vela com uma a sete chaves — o que cada passo faz, por que as chaves ficam definidas na criação, o que se torna público e o que a sua carteira é de fato."
source: a2edda21a075
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Criar sua carteira

Criar uma carteira leva um ou dois minutos. Abra a carteira web em
[wallet.getvela.app](https://wallet.getvela.app/) — ou a extensão, o app de
desktop ou o de celular — e escolha **Criar carteira**.

## Passo a passo

1. **Dê um nome à carteira.** O nome ajuda você a reconhecê-la, e ele é gravado num
   registro público junto com as suas chaves — trate-o como público e não coloque
   nada pessoal nele.
2. **Confirme o que vai acontecer.** Você marca que as suas chaves públicas e o
   nome da carteira são gravados on-chain, que as suas chaves privadas ficam nos
   seus aparelhos ou chaves de segurança e que concorda com os [termos](/terms) e a
   [política de privacidade](/privacy).
3. **Crie a primeira chave.** Escolha como: **este aparelho** (Face ID, Touch ID,
   digital, Windows Hello), **um celular ou tablet** (escaneie um QR code e crie a
   chave lá, onde o app oferecer essa opção) ou uma **chave de segurança USB**. O
   aparelho cria a passkey e assina uma vez com ela, para que o app saiba que a
   chave funciona de verdade antes de continuar.
4. **Adicione mais chaves, se quiser.** Até sete no total, de qualquer tipo.
   Qualquer uma delas vai poder assinar sozinha. Se a sua única chave não está
   sincronizada em lugar nenhum — uma chave de segurança, ou o Windows Hello —, o
   app pede uma segunda, porque com uma única chave não sincronizada basta perder um
   aparelho para perder a carteira.
5. **Crie.** O app calcula o endereço da carteira a partir do conjunto completo de
   chaves e publica esse conjunto no registro público na Gnosis Chain. Quando esse
   registro estiver on-chain, a carteira abre.

<Callout type="warning" title="Escolha suas chaves agora">
Seu endereço é calculado a partir das chaves com que você termina a criação, então
não dá para adicionar, remover nem trocar chaves depois.
[Signatários e chaves de segurança](/pt-BR/docs/signers) explica por quê, e como
escolher.
</Callout>

## O que é a sua carteira

Sua carteira é uma **conta inteligente Safe** — um contrato, não uma conta comum
com uma única chave privada. As suas chaves são as proprietárias dela, e qualquer
uma pode autorizar uma transação. [O contrato da conta](/pt-BR/docs/account-contract)
lista todos os contratos envolvidos.

O endereço é **o mesmo em todas as redes** e é **contrafactual**: ele é calculado
antes de qualquer coisa ser implantada, então você pode receber fundos em qualquer
rede na hora. O contrato se implanta sozinho na primeira vez que você envia algo
numa rede, e a taxa dessa primeira transação inclui a implantação. Criar a carteira
não custa nada.

## O que é público

<span id="what-is-public"></span>

Criar uma carteira grava um registro permanente num contrato de registro público
na Gnosis Chain, que qualquer pessoa pode ler e que não pode ser editado nem
apagado:

- a **chave pública** de cada chave (nunca a chave privada) e o seu **ID de
  credencial**;
- o **modelo do autenticador** de cada chave (qual gerenciador de senhas ou chave
  de segurança a criou) e indicadores que dizem se você foi verificado e se a chave
  é sincronizada;
- o **nome da carteira** e um **nome para cada chave**;
- o **endereço da carteira** e quando ela foi criada;
- os próprios **dados de registro assinados**.

O índice de chaves públicas da Vela envia o registro e paga o gas dele, então é o
primeiro a ver esse registro. Nada ali permite mover os seus fundos; é o que deixa
qualquer uma das suas chaves encontrar a carteira de novo num aparelho novo
([recuperação](/pt-BR/docs/recovery)). A [política de privacidade](/privacy) traz a
lista completa, e a [página do registro](/registry) mostra todos os registros.

## Próximos passos

- [Receba seus primeiros tokens](/pt-BR/docs/send-and-receive)
- [Entenda redes e taxas](/pt-BR/docs/networks-and-fees)
- [O que fazer se você perder um aparelho](/pt-BR/docs/recovery)
