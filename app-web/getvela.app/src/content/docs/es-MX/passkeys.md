---
title: Cómo funcionan las passkeys
description: "Qué es una passkey, dónde vive la llave privada en cada tipo de llave, por qué no hay ningún secreto que te puedan robar con phishing y de qué no te protege una passkey."
source: b23999b2ed69
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Cómo funcionan las passkeys

Las llaves que controlan una wallet de Vela son **passkeys**: credenciales WebAuthn
sobre la curva P-256. Tu dispositivo o tu llave de seguridad crea cada una, guarda
la llave privada y solo la usa después de que confirmas con Face ID, una huella, el
PIN de tu dispositivo, o un toque y un PIN en una llave de seguridad. Vela nunca
recibe la llave privada; si un gestor de contraseñas la sincroniza, es el gestor
quien la guarda, cifrada, en tu nombre.

## Qué es una passkey

Una passkey es un par de llaves pública y privada creado para un solo sitio web; en
el caso de Vela, `getvela.app`. Ninguna app recibe nunca la llave privada: solo
puede pedirle al autenticador que firme algo, y el autenticador te pregunta a ti
primero.

Dónde vive la llave privada depende del tipo de llave:

| Tipo de llave | Dónde vive la llave privada | ¿Se sincroniza con otros dispositivos? |
| --- | --- | --- |
| **Este dispositivo**: Face ID, Touch ID, huella, Windows Hello | El gestor de contraseñas de tu plataforma (Llavero de iCloud, Administrador de contraseñas de Google) o un gestor de contraseñas como 1Password | Por lo general sí, con cifrado de extremo a extremo, si la sincronización está activada. Las llaves de Windows Hello se quedan en la PC |
| **Otro celular**, al que llegas escaneando un código QR | El gestor de contraseñas de ese celular | Igual que arriba |
| **Una llave de seguridad física** (YubiKey y otras llaves FIDO2, por USB o NFC) | Dentro de la llave de seguridad | Nunca |

Una wallet de Vela puede usar hasta siete llaves, combinadas como quieras, que se
eligen al crearla; [firmantes y llaves de seguridad](/es-MX/docs/signers) trata esa
elección.

## Ningún secreto que robar con phishing

El phishing funciona haciendo que entregues un secreto. Una frase semilla son doce
palabras que alguien puede convencerte de escribir en algún lado. Una passkey **no
tiene ningún secreto que puedas escribir**: no hay nada que revelar ni nada que
pegar, y un sitio falso no puede pedírtela. Y como una passkey se crea para un solo
sitio web, tu navegador solo ofrece una passkey de `getvela.app` a páginas de
getvela.app y sus subdominios.

Eso elimina toda una categoría de pérdidas, la de la frase de recuperación robada,
que es común en la autocustodia.

## De qué no te protege una passkey

<Callout type="warning" title="Una passkey firma lo que apruebes">
El aviso de tu celular o de tu navegador dice <em>qué</em> llave se está usando, no
<em>qué</em> se está firmando. Una passkey firma una transacción dañina con la misma
facilidad que una buena si la apruebas. Por eso Vela decodifica cada transacción
antes de que firmes (<a href="/es-MX/docs/clear-signing">firma legible</a>), y por
eso importa la página que te la muestra (<a href="/es-MX/docs/bybit-attack">el
ataque a Bybit</a>).
</Callout>

Tampoco te protege de alguien que tenga tu celular desbloqueado y pueda pasar su
verificación, ni de quien controle la cuenta a través de la cual se sincroniza tu
passkey. Mantén un código de desbloqueo en tu dispositivo, protege tu cuenta de
Apple o de Google, y considera una llave de seguridad física que no se sincronice en
ningún lado.

## Cómo se siente firmar

1. Confirmas una transacción en Vela, después de leer lo que hace.
2. Tu dispositivo o tu llave de seguridad te pide Face ID, una huella, tu PIN, o un
   toque más un PIN.
3. Firma, y a la app solo le regresa la firma.
4. La app le pasa la operación firmada al relay, que la envía; el contrato de tu
   wallet verifica la firma de la passkey on-chain antes de hacer cualquier cosa.

## A dónde va la llave pública

Las mitades **públicas** de tus llaves quedan anotadas en un registro público en
Gnosis Chain, para que un dispositivo nuevo pueda encontrar tu wallet. De eso trata
[recuperación e inicio de sesión](/es-MX/docs/recovery).

Sigue: [firmantes y llaves de seguridad](/es-MX/docs/signers).
