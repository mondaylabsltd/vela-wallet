---
title: Crea tu wallet
description: "Crea una wallet de Vela con una a siete llaves: qué hace cada paso, por qué las llaves quedan fijas al crearla, qué se vuelve público y qué es tu wallet en realidad."
source: a2edda21a075
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Crea tu wallet

Crear una wallet toma uno o dos minutos. Abre la wallet web en
[wallet.getvela.app](https://wallet.getvela.app/) (o la extensión, la app de
escritorio o la de celular) y elige **Crear billetera**.

## Pasos

1. **Ponle nombre a tu wallet.** El nombre te ayuda a reconocerla, y se escribe en
   un registro público junto con tus llaves: considéralo público y no pongas nada
   privado en él.
2. **Confirma lo que va a pasar.** Marcas que tus llaves públicas y el nombre de la
   wallet se escriben on-chain, que tus llaves privadas se quedan en tus
   dispositivos o llaves de seguridad, y que aceptas los [términos](/terms) y la
   [política de privacidad](/privacy).
3. **Crea tu primera llave.** Elige cómo: **este dispositivo** (Face ID, Touch ID,
   huella, Windows Hello), **un celular o una tablet** (escaneas un código QR y la
   creas ahí, donde la app lo ofrece) o una **llave de seguridad USB**. Tu
   dispositivo crea la passkey y luego firma una vez con ella, para que la app sepa
   que la llave de verdad funciona antes de seguir.
4. **Agrega más llaves, si quieres.** Hasta siete en total, de cualquier tipo.
   Cualquiera de ellas podrá firmar por sí sola. Si tu única llave no está
   sincronizada en ningún lado (una llave de seguridad, o Windows Hello), la app te
   pide una segunda, porque con una sola llave sin sincronizar, perder un
   dispositivo es perder la wallet.
5. **Crea.** La app calcula la dirección de tu wallet a partir del conjunto completo
   de llaves y publica ese conjunto en el registro público de Gnosis Chain. Cuando
   ese registro queda on-chain, tu wallet se abre.

<Callout type="warning" title="Elige tus llaves ahora">
Tu dirección se calcula a partir de las llaves con las que terminas, así que
después no se pueden agregar, quitar ni reemplazar llaves.
[Firmantes y llaves de seguridad](/es-MX/docs/signers) explica por qué y cómo
elegirlas.
</Callout>

## Qué es tu wallet

Tu wallet es una **cuenta inteligente Safe**: un contrato, no una cuenta simple con
una sola llave privada. Tus llaves son sus dueñas, y cualquiera de ellas puede
autorizar una transacción. [El contrato de la cuenta](/es-MX/docs/account-contract)
enumera cada contrato involucrado.

La dirección es **la misma en todas las redes** y es **contrafactual**: se calcula
antes de desplegar nada, así que puedes recibir fondos en cualquier red desde el
primer momento. El contrato se despliega solo la primera vez que envías desde una
red, y la comisión de esa primera transacción incluye el despliegue. Crear la wallet
no te cuesta nada.

## Qué es público

<span id="what-is-public"></span>

Crear una wallet escribe un registro permanente en un contrato de registro público
en Gnosis Chain, que cualquiera puede leer y que es imposible editar o borrar:

- la **llave pública** de cada llave (nunca la llave privada) y su **ID de
  credencial**;
- el **modelo de autenticador** de cada llave (qué gestor de contraseñas o qué llave
  de seguridad la creó) y los indicadores de si se verificó tu identidad y de si la
  llave está sincronizada;
- el **nombre de la wallet** y un **nombre para cada llave**;
- la **dirección de la wallet** y cuándo se creó;
- los **datos de registro firmados** en sí.

El índice de llaves públicas de Vela envía el registro y paga su gas, así que es el
primero en verlo. Nada de lo que contiene puede mover tus fondos; es lo que permite
que cualquiera de tus llaves vuelva a encontrar la wallet en un dispositivo nuevo
([recuperación](/es-MX/docs/recovery)). La [política de privacidad](/privacy) tiene
la lista completa, y la [página del registro](/registry) muestra cada registro.

## Siguientes pasos

- [Recibe tus primeros tokens](/es-MX/docs/send-and-receive)
- [Entiende las redes y las comisiones](/es-MX/docs/networks-and-fees)
- [Qué hacer si pierdes un dispositivo](/es-MX/docs/recovery)
