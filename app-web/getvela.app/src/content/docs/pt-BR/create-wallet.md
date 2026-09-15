---
title: Crie sua carteira
description: Crie uma carteira Vela autocustodiada em cerca de um minuto com uma passkey — sem frase-semente. Sua carteira é uma conta inteligente Safe com o mesmo endereço em todas as redes.
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Crie sua carteira

Criar uma carteira leva cerca de um minuto e uma única confirmação biométrica. Abra
a carteira web em [wallet.getvela.app](https://wallet.getvela.app/) e escolha
**Criar uma carteira**.

## Os passos

1. **Dê um nome à carteira.** Escolha um nome para reconhecer a conta depois,
   inclusive ao entrar de outro aparelho. Ele fica guardado ao lado da sua chave
   pública, então trate como informação pública — não coloque nada privado ali.
2. **Confirme o básico.** Uma lista curta confirma que você entendeu que a Vela é
   autocustodiada e ainda está em alfa, com links para a
   [política de privacidade](/privacy) e os [termos](/terms).
3. **Crie sua passkey.** Quando aparecer o pedido, autentique-se com **Face ID,
   Touch ID ou sua digital**. Isso cria uma passkey WebAuthn (P-256) que seu
   aparelho guarda e que a Vela nunca vê. Não existe a etapa «anote a
   frase-semente», porque não existe frase-semente.
4. **Pronto.** A Vela mostra o endereço da sua carteira e você já está dentro. Dá
   para conferir o endereço e depois entrar para cair na sua carteira.

## O que a sua carteira é de fato

Esta é a parte que a maioria das carteiras não explica — e ela define como a Vela
funciona.

Sua carteira Vela é uma **conta inteligente Safe** (um contrato), e não uma simples
«conta de propriedade externa». Sua passkey é a dona dessa conta; um arranjo
ERC-4337 deixa você operá-la só com o rosto ou a digital.

<Callout type="info" title="Seu endereço é o mesmo em todas as redes">
A Vela deriva seu endereço da chave pública da sua passkey, então ele é idêntico em
Ethereum, Base, Arbitrum, Gnosis e em todas as outras redes suportadas. Você passa
um endereço só, em qualquer lugar.
</Callout>

Uma consequência útil: o endereço é **contrafactual**. Ele é calculado antes de
qualquer coisa ser implantada on-chain, então **você pode receber fundos antes
mesmo de o contrato da sua carteira existir**. O contrato se implanta sozinho —
pagando do próprio saldo — na sua primeira transação em cada rede.

## O que acabou de acontecer com suas chaves

- Seu aparelho gerou um **par de chaves de passkey**.
- A **chave privada** fica com o serviço de passkeys do seu sistema (Chaveiro do
  iCloud ou Gerenciador de senhas do Google), criptografada de ponta a ponta e
  sincronizada entre seus aparelhos — nenhum app, nem a Vela, chega a vê-la.
- A **chave pública e o nome que você escolheu** são publicados no índice de
  passkeys da Vela, que também grava a chave em um registro publicamente legível na
  Gnosis Chain, para que sua conta possa ser encontrada de novo em um aparelho
  novo. Veja [recuperação e login](/pt-BR/docs/recovery).

## Próximos passos

- [Receber seus primeiros tokens](/pt-BR/docs/send-and-receive)
- [Entender redes e taxas](/pt-BR/docs/networks-and-fees)
- [Ler por que as passkeys tornam isso seguro](/pt-BR/docs/passkeys)
