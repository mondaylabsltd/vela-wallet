---
title: Como as passkeys funcionam
description: O modelo de segurança por trás da Vela — o que é uma passkey, onde a sua chave mora e por que não há nada para roubar por phishing.
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Como as passkeys funcionam

Todo o modelo de segurança da Vela se apoia em uma ideia: a chave que controla sua
carteira é uma **passkey**, criada pelo seu aparelho e guardada pelo seu sistema
operacional — nenhum app, nem a Vela, consegue lê-la — e usada só com o seu rosto ou
a sua digital.

## O que é uma passkey, de fato

Uma passkey é um par de chaves pública/privada criado pelo seu aparelho. A **chave
privada** fica com o serviço de passkeys do seu sistema — normalmente o Chaveiro do
iCloud na Apple, o Gerenciador de senhas do Google no Android — criptografada de
ponta a ponta, de modo que nenhum app consegue lê-la ou copiá-la. Os apps não
recebem a chave: recebem a permissão de *pedir ao seu aparelho que assine algo*
depois que você se autentica.

É a mesma tecnologia que protege o Apple Pay e o seu desbloqueio biométrico.

<Callout type="info" title="O ponto central">
Um app — inclusive a Vela — pode pedir uma assinatura, mas nunca vê sua chave
privada. Seu rosto ou sua digital autorizam o aparelho a assinar; a chave em si
continua no sistema operacional, criptografada de ponta a ponta.
</Callout>

## Por que não há nada para roubar por phishing

O phishing funciona fazendo você entregar um segredo. Com uma frase-semente, esse
segredo são doze palavras que dá para digitar numa página falsa. Com uma passkey
**não existe segredo digitável**. Um site golpista não consegue pedir que você
«digite sua passkey», porque passkey não é coisa que se digite: é uma operação de
hardware liberada pela sua biometria.

Isso elimina a forma mais comum de as pessoas perderem fundos em autocustódia.

## Como é assinar uma transação

1. Você confirma uma transação na Vela.
2. Seu aparelho pede Face ID / Touch ID.
3. Seu aparelho assina a transação com a sua passkey.
4. A Vela transmite a transação assinada para a rede.

O mesmo gesto de desbloquear o celular — porque é o mesmo mecanismo de passkey que
seu aparelho já usa em todo o resto.

<Callout type="warning" title="A segurança do aparelho continua importando">
Uma passkey protege muito bem contra ataques remotos e phishing. Ela não protege
contra alguém que está com o seu aparelho desbloqueado e passa pela sua checagem
biométrica. Mantenha um código de acesso e não entregue um celular desbloqueado a
quem você não confia.
</Callout>

## Onde mora o resto

A chave **pública** da sua passkey é publicada num pequeno índice on-chain, para
que sua carteira possa ser recuperada em um aparelho novo. Esse é o assunto da
próxima página: [recuperação e login](/pt-BR/docs/recovery).
