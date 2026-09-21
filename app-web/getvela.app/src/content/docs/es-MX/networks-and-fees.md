---
title: Redes y comisiones
description: "Las 24 redes integradas en Vela, cómo agregar otra, cómo se calcula exactamente la comisión de una transacción y quién la recibe, y qué pasa cuando un relay se queda sin gas."
source: 84328d162a3a
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Redes y comisiones

## Redes integradas

Vela trae **24 redes** integradas, todas son mainnets:

| Red | Gas se paga en | Red | Gas se paga en |
| --- | --- | --- | --- |
| Ethereum | ETH | Arc | USDC (la moneda nativa) |
| BNB Chain | BNB | X Layer | OKB |
| Polygon | POL | Stable | USDT0 (la moneda nativa) |
| Arbitrum | ETH | Soneium | ETH |
| Optimism | ETH | MegaETH | ETH |
| Base | ETH | Robinhood Chain | ETH |
| Avalanche | AVAX | Mantle | MNT |
| Gnosis | xDAI | Kaia | KAIA |
| Unichain | ETH | Celo | CELO |
| Tempo | pathUSD (sin moneda nativa) | Ink | ETH |
| Monad | MON | Plume | PLUME |
| World Chain | ETH | XRPL EVM | XRP |

En la mayoría también puedes pagar la comisión con una stablecoin en dólares que el
relay acepte en esa red (ver abajo).

Tu wallet tiene **la misma dirección en todas las redes**, porque la dirección se
calcula a partir de tus llaves, no de la cadena.

## Agregar otra red

Puedes agregar cualquier red EVM en **Ajustes → Redes**, siempre que tenga lo que
necesita una wallet de Vela: once contratos estándar (el EntryPoint v0.7 de
ERC-4337, los contratos de Safe v1.4.1, los módulos 4337 y de passkey de Safe,
MultiSend, Multicall3 y dos desplegadores deterministas) y el precompilado
**RIP-7212** que verifica las firmas de passkey en la dirección `0x100`. La wallet
revisa todo eso, incluida una verificación de firma real contra el precompilado,
antes de dejarte agregar la red.

El precompilado es un requisito indispensable. Su dirección forma parte de cómo se
calcula cada dirección de Vela, así que no hay verificador de respaldo ni forma de
desplegar uno después. Si una cadena tiene el precompilado pero le faltan algunos
contratos, la página de [configuración de cadenas](/es-MX/chain-setup) te muestra
qué falta y despliega lo que cualquiera puede desplegar. La revisión tiene un hueco:
una wallet con más de una llave también necesita en la red la fábrica de firmantes
de passkey de Safe, que todavía no se revisa; sin ella, ahí solo puede firmar la
primera llave.

## Cómo se paga una transacción

Vela es una wallet ERC-4337: no transmites la transacción tú mismo. La app arma una
**UserOperation**, tú la firmas con una de tus llaves, y un **relay** la envía a la
cadena y paga el gas por adelantado. (ERC-4337 llama *bundler* a este rol.) Al
relay se le paga **dentro de tu operación**: el pago es una transferencia de tu
wallet al relay que va en el mismo lote que tu transacción, así que tu firma lo
cubre. No hay paymaster, nadie patrocina tu gas y nadie puede rechazar tu
transacción por una política de patrocinio.

### Cuánto es la comisión

La pantalla de confirmación muestra un solo monto, en la moneda de la comisión y en
tu moneda de visualización. Se calcula así:

- **Gas que reserva la wallet.** La wallet simula la transacción y reserva más gas
  del que espera usar: las estimaciones de verificación y de ejecución se aumentan
  un 50% cada una, con mínimos (por ejemplo, la verificación es de al menos 300,000
  unidades de gas una vez desplegada la wallet, y de 2,000,000 en la transacción que
  la despliega).
- **Precio del gas.** El más alto entre la lectura que hace la propia wallet del
  precio del gas de la red y el precio del relay para la velocidad que elegiste. La
  velocidad por defecto es *Rápido*, que el relay cotiza en unas 1.8 veces la
  tarifa base más el doble de la tarifa de prioridad.
- **Comisión = 3 × gas reservado × precio del gas**, con un mínimo de alrededor de
  US$0.01. En Tempo el múltiplo es 2 y la comisión se paga en pathUSD.

Como la reserva queda muy por encima de lo que la transacción va a usar y el precio
incluye margen, **la comisión suele ser diez veces o más lo que la transacción
cuesta de verdad on-chain**, y más en la primera transacción en una red. El relay
paga el costo real y se queda con el resto; no se reembolsa nada. En redes baratas
son centavos; en la mainnet de Ethereum puede ser una cantidad considerable. El monto
exacto está en la pantalla de confirmación antes de que firmes.

<Callout type="info" title="Lo que ves es lo que pagas">
El monto de la comisión y la dirección a la que va forman parte de la operación que
firmas. Un relay que cambiara cualquiera de los dos invalidaría tu firma, así que
pagas exactamente el monto que se muestra, ni un centavo más, aunque el gas suba
antes de que la transacción entre. Si un relay cotiza un precio del gas de más del
triple de la lectura de la propia wallet, la cotización se rechaza.
</Callout>

### Con qué puedes pagar

- La **moneda nativa** de la red, siempre.
- Una **stablecoin en dólares** de la lista del relay para esa red, cuando el relay
  puede ponerle precio a la moneda nativa. Las stablecoins de las que no tienes nada
  no aparecen.
- En **Tempo**, que no tiene moneda nativa, solo **pathUSD**.

La moneda de la comisión y la velocidad (*Lento*, *Estándar* o *Rápido*) las eliges
en la pantalla de confirmación y en Ajustes.

### Tu primera transacción en una red

Puedes recibir en cualquier red antes de que tu wallet exista ahí. La primera vez
que envías desde una red, esa transacción también despliega el contrato de tu wallet
(y un pequeño contrato firmante por cada llave adicional). El gas del despliegue va
incluido en la comisión de esa transacción, así que el primer envío en cada red
cuesta más que los siguientes.

Cuando envías el **máximo** de una moneda nativa, Vela aparta lo suficiente para la
comisión.

## Quién opera el relay y quién recibe la comisión

Por defecto, todas las redes usan **el relay de Vela**, y la comisión es para Vela.
Puedes apuntar la wallet a otro relay en **Ajustes → Avanzado → Endpoints de
servicio**; una sola dirección sirve para todas las redes integradas, y una red
personalizada conserva la dirección del relay con la que se agregó. El relay tiene
que ser [vela-relay](https://github.com/mondaylabsltd/vela-relay) (el de Vela o uno
que operes tú), porque la wallet le pide la cotización de la comisión con un método
propio de Vela que los bundlers genéricos, como Pimlico o Alchemy, no implementan.
Quien opere el relay que usas recibe la comisión; la
[guía de autoalojamiento](/es-MX/docs/self-hosting#relay) explica cómo operar uno.

El relay recibe una operación que ya está firmada. No puede cambiar el
destinatario, el monto, la comisión ni nada más. Sí puede retrasarla o rechazarla, y
elige cuándo entra a la cadena; así que, en un swap, en principio podría adelantarse a
tu operación dentro de tu tolerancia al deslizamiento.

### Cuando un relay se queda sin gas

Un relay paga el gas desde su propia **tesorería** en cada red. Si esa tesorería
está vacía, la pantalla de envío te avisa antes de que firmes:

- En una red que atiende el relay de Vela, el operador del relay (Vela) tiene que
  recargarla; puedes reportarlo. Si no puedes esperar, puedes, **si quieres**,
  enviar tú mismo una pequeña cantidad de la moneda nativa a la tesorería. Esa
  aportación **no es reembolsable** y **no** paga tu propia transacción.
- En una red personalizada, fondear el relay le toca a quien lo opere, que puedes
  ser tú.

No hay una cuenta de gas por wallet ni un depósito de activación: una versión
anterior de Vela tenía uno, y ya no existe.

## Cómo lee Vela cada red

Vela lee los saldos y simula las transacciones a través de un **conjunto de
endpoints RPC** por red (los integrados, respaldos públicos y cualquier llave de
proveedor o endpoint que agregues) y pasa al siguiente cuando uno está lento o
caído. Puedes configurar tu propio endpoint por red en **Ajustes → Redes**. (La app
de Android por ahora usa un solo endpoint por red, sin conmutación, y la app de
iPhone todavía no te deja cambiarlo.)

Sigue: [cómo funcionan las passkeys](/es-MX/docs/passkeys).
