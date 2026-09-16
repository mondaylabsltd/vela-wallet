---
title: Enviar y recibir
description: Cómo recibir y enviar tokens en Vela — una sola dirección en todas las redes, transacciones firmadas de forma legible, y cómo la abstracción de cuentas mueve realmente tus fondos.
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Enviar y recibir

## Recibir

1. Abre tu wallet y toca **Recibir**.
2. Comparte tu dirección: cópiala o deja que quien envía escanee el código QR.
3. Cuando la transferencia se confirma on-chain, el saldo aparece en tu wallet.

Dos cosas que vale la pena saber:

- Tu dirección es **la misma en cada red soportada**, así que compartes una sola
  dirección en todos lados; solo asegúrate de que quien envía use la red correcta.
- Puedes **recibir antes de que tu wallet esté desplegada**. Las cuentas de Vela son
  cuentas inteligentes contrafactuales: los fondos pueden llegar a tu dirección
  antes de que el contrato exista en esa cadena; se despliega solo en tu primer
  envío ahí.

## Enviar

1. Toca **Enviar** y elige el **token**.
2. Escribe el **monto** (puedes alternar entre el token y tu moneda de
   visualización) y el **destinatario**. Donde puede, Vela resuelve destinatarios
   conocidos a un nombre: una cuenta Vela, un nombre ENS, un Basename, etcétera.
3. **Revisa y confirma.** Vela muestra la transferencia y luego pide tu passkey
   (Face ID / Touch ID / huella).

### Qué pasa cuando confirmas

Vela no solo «transmite» una transacción. Por debajo:

1. Arma una **UserOperation** ERC-4337 para tu cuenta Safe.
2. Tu dispositivo la firma con una aserción **WebAuthn (P-256)** después de tu
   verificación biométrica.
3. La operación firmada va al **relay**, que la envía al EntryPoint; tu Safe
   verifica la firma P-256 **on-chain** y ejecuta.

<Callout type="info" title="El relayer no puede alterar tu transacción">
El relay recibe una UserOperation <strong>ya firmada</strong>. Puede demorar o
negarse a retransmitir, pero no puede cambiar destinatario, monto ni ningún otro
campo: cualquier cambio invalida tu firma. Es una ayuda para que la transacción
llegue, no un custodio, y es código abierto, así que puedes correr el tuyo.
</Callout>

### Firma legible: nada de aprobar a ciegas

Antes de que firmes, Vela decodifica la transacción con descriptores **ERC-7730** y
muestra la **intención** (Enviar, Aprobar, Intercambiar…), los **montos y
direcciones** y una indicación de riesgo, en vez de hexadecimal opaco. Cuando no
puede decodificar del todo una llamada, muestra una **advertencia explícita de
firma a ciegas** en lugar de fingir que la entiende. Una aprobación de token
ilimitada no solo se marca: Vela la reescribe a un monto finito y se niega a enviar
una aprobación que siguiera siendo ilimitada.

## Antes de darle enviar

- **Revisa los primeros y últimos caracteres de la dirección.** El malware que
  cambia direcciones existe de verdad.
- **Confirma la red.** Mandar en la red equivocada es el error caro más común. Ve
  [redes y comisiones](/es-MX/docs/networks-and-fees).
- **Con un destinatario nuevo, empieza chico.** Una transferencia de prueba mínima
  es un seguro barato.

Las transacciones son irreversibles. No hay mesa de ayuda que recupere un envío a
la dirección equivocada: así es la autocustodia.

## Leer tu historial

Saldos e historial se leen en vivo desde un grupo de endpoints RPC públicos, con
conmutación automática. Si la red va lenta, el historial puede tardar un momento:
una ruedita girando significa «todavía estoy trayendo datos», no «se fueron los
fondos».
