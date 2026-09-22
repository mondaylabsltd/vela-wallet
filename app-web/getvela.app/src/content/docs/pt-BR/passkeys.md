---
title: Como as passkeys funcionam
description: "O que é uma passkey, onde a chave privada fica em cada tipo de chave, por que não há segredo para roubar por phishing e contra o que uma passkey não protege você."
source: b23999b2ed69
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Como as passkeys funcionam

As chaves que controlam uma carteira Vela são **passkeys**: credenciais WebAuthn na
curva P-256. O seu aparelho ou a sua chave de segurança cria cada uma delas, guarda
a chave privada e só a usa depois que você confirma com Face ID, digital, o PIN do
aparelho, ou um toque e o PIN numa chave de segurança. A Vela nunca recebe a chave
privada; se um gerenciador de senhas a sincroniza, é ele que a guarda, criptografada,
em seu nome.

## O que é uma passkey

Uma passkey é um par de chaves pública e privada criado para um único site — no
caso da Vela, `getvela.app`. Um app nunca recebe a chave privada; ele só pode pedir
ao autenticador que assine algo, e o autenticador pergunta a você antes.

Onde a chave privada fica depende do tipo de chave:

| Tipo de chave | Onde fica a chave privada | Sincronizada com outros aparelhos? |
| --- | --- | --- |
| **Este aparelho** — Face ID, Touch ID, digital, Windows Hello | O gerenciador de senhas da sua plataforma (Chaves do iCloud, Gerenciador de senhas do Google) ou um gerenciador de senhas como o 1Password | Normalmente sim, com criptografia de ponta a ponta, se a sincronização estiver ativada. Chaves do Windows Hello ficam no PC |
| **Outro celular**, conectado por QR code | O gerenciador de senhas desse celular | Igual ao item acima |
| **Uma chave de segurança física** (YubiKey e outras chaves FIDO2, por USB ou NFC) | Dentro da chave de segurança | Nunca |

Uma carteira Vela pode usar até sete chaves, em qualquer combinação, escolhidas
quando você a cria; [signatários e chaves de segurança](/pt-BR/docs/signers) trata
dessa escolha.

## Nenhum segredo para roubar por phishing

O phishing funciona fazendo você entregar um segredo. Uma frase de recuperação são
doze palavras que alguém pode convencer você a digitar em algum lugar. Uma passkey
**não tem nenhum segredo que dê para digitar**: não há nada para revelar, nada para
colar, e um site falso não tem como pedi-la. E, como uma passkey é criada para um
único site, o seu navegador só oferece uma passkey do `getvela.app` a páginas do
getvela.app e dos seus subdomínios.

Isso elimina uma categoria inteira de perdas — a frase de recuperação roubada — que
é comum na autocustódia.

## Contra o que uma passkey não protege

<Callout type="warning" title="Uma passkey assina tudo o que você aprovar">
O aviso do seu celular ou navegador diz <em>qual</em> chave está sendo usada, não
<em>o que</em> está sendo assinado. Uma passkey assina uma transação maliciosa com
a mesma facilidade que uma legítima, se você aprovar. É por isso que a Vela
decodifica cada transação antes de você assinar
(<a href="/pt-BR/docs/clear-signing">assinatura legível</a>), e é por isso que a
página que a mostra importa (<a href="/pt-BR/docs/bybit-attack">o ataque à
Bybit</a>).
</Callout>

Ela também não protege contra alguém que esteja com o seu celular desbloqueado e
consiga passar pela verificação dele, nem contra quem controla a conta pela qual a
sua passkey é sincronizada. Mantenha um código de bloqueio no aparelho, proteja a
sua conta Apple ou Google e considere uma chave de segurança física, que não é
sincronizada em lugar nenhum.

## Como é assinar

1. Você confirma uma transação na Vela, depois de ler o que ela faz.
2. O seu aparelho ou a sua chave de segurança pede Face ID, digital, o seu PIN, ou um
   toque mais o PIN.
3. Ele assina, e só a assinatura volta para o app.
4. O app entrega a operação assinada ao relay, que a envia; o contrato da sua
   carteira confere a assinatura da passkey on-chain antes de fazer qualquer coisa.

## Para onde vai a chave pública

As metades **públicas** das suas chaves ficam gravadas num registro público na
Gnosis Chain, para que um aparelho novo consiga encontrar a sua carteira. Esse é o
assunto de [recuperação e login](/pt-BR/docs/recovery).

A seguir: [signatários e chaves de segurança](/pt-BR/docs/signers).
