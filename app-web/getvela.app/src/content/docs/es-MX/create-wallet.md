---
title: Crea tu wallet
description: Crea una wallet Vela de autocustodia en cosa de un minuto con una passkey, sin frase semilla. Tu wallet es una cuenta inteligente Safe con la misma dirección en cada red.
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Crea tu wallet

Crear una wallet toma cosa de un minuto y una sola verificación biométrica. Abre la
wallet web en [wallet.getvela.app](https://wallet.getvela.app/) y elige **Crear una
wallet**.

## Los pasos

1. **Ponle nombre a tu wallet.** Elige un nombre para reconocer la cuenta después,
   también al iniciar sesión en otro dispositivo. Se guarda junto a tu llave
   pública, así que trátalo como público: no pongas nada privado ahí.
2. **Confirma lo básico.** Una lista corta confirma que entiendes que Vela es de
   autocustodia y que todavía es software en alfa, con enlaces al
   [aviso de privacidad](/privacy) y a los [términos](/terms).
3. **Crea tu passkey.** Cuando aparezca el aviso, autentícate con **Face ID, Touch
   ID o tu huella**. Eso crea una passkey WebAuthn (P-256) que tu dispositivo
   guarda y que Vela nunca ve. No hay paso de «anota la frase semilla», porque no
   hay frase semilla.
4. **Listo.** Vela muestra la dirección de tu wallet y ya estás dentro. Puedes
   verificarla y luego iniciar sesión para entrar a tu wallet.

## Qué es realmente tu wallet

Esta es la parte que casi ninguna wallet explica — y define cómo funciona Vela.

Tu wallet Vela es una **cuenta inteligente Safe** (un contrato), no una simple
«cuenta de propiedad externa». Tu passkey es la dueña de esa cuenta; un montaje
ERC-4337 te deja operarla solo con tu cara o tu huella.

<Callout type="info" title="Tu dirección es la misma en cada red">
Vela deriva tu dirección de la llave pública de tu passkey, así que es idéntica en
Ethereum, Base, Arbitrum, Gnosis y en cualquier otra red soportada. Das una sola
dirección en todos lados.
</Callout>

Una consecuencia útil: la dirección es **contrafactual**. Se calcula antes de que
se despliegue nada on-chain, así que **puedes recibir fondos antes de que exista el
contrato de tu wallet**. El contrato se despliega solo —pagando de su propio
saldo— en tu primera transacción en cada red.

## Qué acaba de pasar con tus llaves

- Tu dispositivo generó un **par de llaves de passkey**.
- La **llave privada** la guarda el servicio de passkeys de tu sistema (Llavero de
  iCloud o Gestor de contraseñas de Google), cifrada de extremo a extremo y
  sincronizada entre tus dispositivos: ninguna app, ni siquiera Vela, la ve nunca.
- La **llave pública y el nombre que elegiste** se publican en el índice de
  passkeys de Vela, que además escribe la llave en un registro públicamente
  legible en Gnosis Chain, para que tu cuenta se pueda encontrar desde un
  dispositivo nuevo. Ve
  [recuperación e inicio de sesión](/es-MX/docs/recovery).

## Siguientes pasos

- [Recibir tus primeros tokens](/es-MX/docs/send-and-receive)
- [Entender redes y comisiones](/es-MX/docs/networks-and-fees)
- [Leer por qué las passkeys hacen esto seguro](/es-MX/docs/passkeys)
