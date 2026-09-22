---
title: Enviar y recibir
description: "Cómo recibir y enviar con Vela: una dirección en todas las redes, envíos a una o a muchas personas, cómo se nombra a los destinatarios, qué confirmas y cómo mueve el relay tus fondos."
source: 9e280dfc853b
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Enviar y recibir

## Recibir

1. Abre tu wallet y toca **Recibir**.
2. Comparte tu dirección: cópiala o muestra el código QR. La wallet web también
   puede crear una solicitud de pago que incluye un monto.
3. Cuando la transferencia se confirma on-chain, aparece en tu saldo.

- Tu dirección es **la misma en todas las redes**, así que compartes una sola
  dirección; aun así, quien te envía tiene que usar una red que Vela admita o una
  que tú hayas agregado.
- Puedes **recibir antes de que tu wallet esté desplegada** en una red. Se
  despliega sola en tu primer envío desde ahí.

## Enviar

1. Toca **Enviar** y elige el **token**.
2. Ingresa el **monto** (en el token o en tu moneda de visualización) y la
   **dirección del destinatario**: pégala, escanea un código QR o elige un
   contacto.
3. **Revisa.** Vela te muestra qué va a pasar, la comisión y el nombre que encontró
   para el destinatario, si encontró alguno.
4. **Confirma** con una de tus llaves: Face ID, una huella, un PIN, o un toque y un
   PIN en tu llave de seguridad.

### Enviar a muchas personas, o juntar

- **Repartir**: envía un token a varias personas en una sola transacción. Puedes
  pegar una lista o importar una hoja de cálculo, e ingresar los montos en tu
  moneda.
- **Juntar**: envía varios tokens a una misma dirección en una sola transacción.

En los dos casos firmas una sola vez, y la transacción paga una sola comisión.

### Nombres para las direcciones

Cuando ingresas una dirección, Vela busca un nombre para ella: primero en su propio
registro (el nombre de otra wallet de Vela) y luego en los registros inversos de
`.bnb`, `.arb`, `.g`, Basename y ENS, leídos directamente de cada cadena. Esto
funciona en un solo sentido: le pone nombre a una dirección que ingresaste. Escribir
un nombre como `alice.eth` no busca ninguna dirección. Tus **contactos** guardados
también muestran su nombre.

Un nombre de esos registros inversos solo se muestra si **resuelve hacia adelante a
la misma dirección**. Cualquiera puede poner en su propio registro inverso el texto
que quiera, así que el registro por sí solo no prueba nada; la wallet le pregunta al
servicio de nombres a qué dirección apunta ese nombre, y solo muestra el nombre
cuando los dos coinciden. Si la comprobación no se puede hacer (un endpoint que no
responde, un resolver que falla), ves la dirección y ningún nombre, nunca uno sin
comprobar.

### Moneda de la comisión y velocidad

La pantalla de confirmación muestra la comisión en la moneda con la que pagas y en tu
moneda. Puedes pagar con la moneda de la red o, donde el relay la acepte, con una
stablecoin en dólares, y elegir una velocidad (por defecto: rápida). Cuando envías
el **máximo** de una moneda nativa, Vela aparta lo suficiente para la comisión.
[Cómo se calcula la comisión](/es-MX/docs/networks-and-fees).

### Qué pasa cuando confirmas

1. Vela arma una **UserOperation** de ERC-4337 para tu Safe, que incluye el pago de
   la comisión al relay.
2. Tu llave la firma con una aserción **WebAuthn (P-256)** después de verificar que
   eres tú.
3. La operación firmada va al **relay**, que la envía al EntryPoint; tu Safe
   verifica la firma P-256 on-chain y la ejecuta.

<Callout type="info" title="El relay no puede cambiar tu transacción">
El relay recibe una operación que ya está firmada. No puede cambiar el destinatario,
el monto ni la comisión: cualquier cambio invalida tu firma. Sí puede retrasarla o
rechazarla, y decide cuándo entra a la cadena. Es de código abierto, y puedes
[operar el tuyo](/es-MX/docs/self-hosting#relay).
</Callout>

Antes de que firmes, Vela decodifica lo que hace la transacción y te advierte de lo
que no puede decodificar; consulta [firma legible](/es-MX/docs/clear-signing).

## Antes de darle a enviar

- **Revisa el principio y el final de la dirección.** El malware que cambia
  direcciones existe, y también las direcciones parecidas que alguien siembra en tu
  historial.
- **Confirma la red.** Enviar en la red equivocada es un error común y caro.
- **Empieza con poco cuando el destinatario es nuevo.** Una transferencia de prueba
  pequeñita es un seguro barato.

Las transacciones son irreversibles. Nadie puede recuperar un envío a la dirección
equivocada: así es la autocustodia.

## Tu actividad

Tu actividad combina lo que enviaste desde este dispositivo con las transferencias
de tokens que se leen de los logs de cada cadena. Una transferencia simple de moneda
nativa que te llega a través de otro contrato (por ejemplo, algunos retiros de
exchanges) puede no generar ningún log en algunas redes, así que puede aparecer en
tu saldo sin aparecer en la actividad. Los saldos se leen en vivo a través de un
conjunto de endpoints RPC con conmutación automática; un indicador de carga
significa «todavía consultando», no «los fondos desaparecieron».

Sigue: [redes y comisiones](/es-MX/docs/networks-and-fees).
