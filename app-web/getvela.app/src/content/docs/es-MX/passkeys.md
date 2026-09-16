---
title: Cómo funcionan las passkeys
description: El modelo de seguridad detrás de Vela — qué es una passkey, dónde vive tu llave y por qué no hay nada que robar con phishing.
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Cómo funcionan las passkeys

Todo el modelo de seguridad de Vela se apoya en una idea: la llave que controla tu
wallet es una **passkey**, creada por tu dispositivo y guardada por tu sistema
operativo — ninguna app, ni siquiera Vela, puede leerla — y usada solo con tu cara
o tu huella.

## Qué es realmente una passkey

Una passkey es un par de llaves pública/privada que crea tu dispositivo. La **llave
privada** la guarda el servicio de passkeys de tu sistema — normalmente el Llavero
de iCloud en Apple, el Gestor de contraseñas de Google en Android — cifrada de
extremo a extremo, así que ninguna app puede leerla ni copiarla. Las apps no
reciben la llave: reciben el permiso de *pedirle a tu dispositivo que firme algo*
después de que te autenticas.

Es la misma tecnología que protege Apple Pay y tu desbloqueo biométrico.

<Callout type="info" title="El punto clave">
Una app —incluida Vela— puede pedir una firma, pero nunca ve tu llave privada. Tu
cara o tu huella autorizan a tu dispositivo a firmar; la llave se queda en tu
sistema operativo, cifrada de extremo a extremo.
</Callout>

## Por qué no hay nada que robar con phishing

El phishing funciona haciéndote entregar un secreto. Con una frase semilla, ese
secreto son doce palabras que puedes teclear en una página falsa. Con una passkey
**no hay ningún secreto que se pueda teclear**. Un sitio fraudulento no puede
pedirte que «escribas tu passkey», porque una passkey no se escribe: es una
operación de hardware con tu biometría de portero.

Eso elimina la forma más común en que la gente pierde fondos en autocustodia.

## Cómo se siente firmar una transacción

1. Confirmas una transacción en Vela.
2. Tu dispositivo pide Face ID / Touch ID.
3. Tu dispositivo firma la transacción con tu passkey.
4. Vela transmite la transacción firmada a la red.

El mismo gesto que desbloquear tu teléfono, porque es el mismo mecanismo de passkey
que tu dispositivo ya usa en todos lados.

<Callout type="warning" title="La seguridad del dispositivo sigue importando">
Una passkey protege buenísimo contra ataques remotos y phishing. No protege contra
alguien que tiene tu dispositivo desbloqueado y pasa tu verificación biométrica.
Ten siempre un código de acceso y no le pases un teléfono desbloqueado a alguien en
quien no confías.
</Callout>

## Dónde vive el resto

La llave **pública** de tu passkey se publica en un pequeño índice on-chain, para
que tu wallet se pueda recuperar en un dispositivo nuevo. Ese es el tema de la
siguiente página: [recuperación e inicio de sesión](/es-MX/docs/recovery).
