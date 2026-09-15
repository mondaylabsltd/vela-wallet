---
title: Redes y comisiones
description: Las 12 redes que soporta Vela, cómo funcionan las comisiones de gas con abstracción de cuentas, quién opera el relay y cobra las comisiones, cuándo pagas tú la activación de la cuenta de gas, y cómo elige Vela sus endpoints RPC.
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Redes y comisiones

## Redes soportadas

Vela trae **12 redes EVM** integradas:

| Red | Token nativo de comisiones |
| ----------- | ---------------- |
| Ethereum | ETH |
| BNB Chain | BNB |
| Polygon | POL |
| Arbitrum | ETH |
| Optimism | ETH |
| Base | ETH |
| Avalanche | AVAX |
| Gnosis | xDAI |
| Unichain | ETH |
| Tempo | USD |
| Monad | MON |
| World Chain | ETH |

Tu wallet tiene **la misma dirección en todas**, así que compartes una sola en
todos lados.

También puedes **agregar redes personalizadas** (Ajustes → Redes). Como Vela es una
wallet de cuenta inteligente, la red tiene que ofrecer los contratos de los que Vela
depende: el EntryPoint ERC-4337, los contratos Safe y el precompilado de firma
**P-256 (RIP-7212)** que verifica tu passkey on-chain. Vela lo revisa
automáticamente antes de dejarte agregar una red.

<Callout type="info" title="Por qué Gnosis aparece tanto">
Además de ser una de las 12 redes, Gnosis Chain aloja el **índice de passkeys** de
Vela: el contrato que guarda tu llave pública y el nombre de tu cuenta para la
recuperación entre dispositivos. Eso es aparte de en qué red haces transacciones.
</Callout>

## Cómo funcionan las comisiones (abstracción de cuentas)

Vela usa **abstracción de cuentas ERC-4337**, así que una transacción no la
transmites tú directamente: es una **UserOperation** entregada a un **relay**, que
la envía on-chain y recibe el reembolso del gas. (La especificación ERC-4337 llama
*bundler* a ese papel. El de Vela se llama relay porque hace más que agrupar:
cotiza comisiones en banda y opera el protocolo de cuenta de gas que viene abajo, y
ninguna de las dos cosas es parte del estándar.) De ahí se derivan varias cosas:

- **El gas sale del saldo de tu propia wallet**, en el token nativo de la red (ETH,
  BNB, xDAI…) por defecto, o en una stablecoin soportada donde el relay ofrezca una;
  el activo con el que pagas lo eliges en la pantalla de confirmación. Tempo no
  tiene moneda nativa, así que ahí el gas siempre se liquida en stablecoins en
  dólares. No hay un **paymaster** ERC-4337 que patrocine —ni condicione— cada
  transacción. (Vela sí puede cubrir la *activación de la cuenta de gas* de una sola
  vez para usuarios nuevos; eso es aparte y se explica abajo.)
- **El relay cotiza el precio del gas** y es la única fuente de verdad: la wallet
  muestra esa cotización y firma exactamente lo que muestra. No hay selector de
  velocidad: cada transacción se envía con prioridad alta.
- El total es el **costo de red más la comisión de servicio del relay**, con un
  mínimo pequeño en transacciones muy baratas. La cotización del relay es el precio:
  no hay otra lista de tarifas que consultar. Una parte va a los validadores de la
  cadena; el resto paga al relay que adelanta el gas y opera la infraestructura.
- La pantalla de confirmación muestra la **comisión estimada** en el activo de
  comisiones y en tu moneda de visualización antes de que firmes. El monto cotizado
  y su destinatario son parte de lo que firmas, así que al relay le pagan exactamente
  lo que se mostró: un número cambiado invalidaría tu firma.

## Quién opera el relay y quién se queda con las comisiones

Cada red apunta a un relay. Por defecto es **el relay de Vela**, y puedes reemplazar
el endpoint en _Ajustes → Avanzado → Endpoints de servicio_. Un endpoint aplica a
todas las redes integradas; una red personalizada conserva la URL de relay que le
diste al agregarla.

Una salvedad honesta sobre compatibilidad: la app cotiza comisiones con un método
RPC propio de Vela (`vela_getInBandGasQuote`), y sin él el flujo de envío falla. El
endpoint al que apuntes tiene que correr
[vela-relay](https://github.com/mondaylabsltd/vela-relay): la instancia de Vela o
una que alojes tú. Un bundler ERC-4337 genérico como **Pimlico** o **Alchemy** no
implementa ese método, así que en la versión actual no funcionará de punta a punta.

Quien opera el relay de una red **se queda con las comisiones de esa red**: el
margen del relay en cada transacción y el depósito de activación de la cuenta de
gas. Corre tu propio vela-relay y esas comisiones financian tu infraestructura en
vez de la de Vela; del tráfico que mandes a otro lado, Vela no toma nada.

<Callout type="warning" title="La cuenta de gas es parte del protocolo vela-relay">
El paso de **activación de la cuenta de gas** fondea una cuenta de relay dedicada a
tu wallet en cada red. Si apuntas el endpoint a un vela-relay que alojas tú, el
depósito fondea la cuenta de tu propio relay, no la de Vela.
</Callout>

### Activar la cuenta de gas (Vela Relay)

En el relay de Vela, tu primera transacción en cada red **activa una cuenta de gas
dedicada**. La app primero le pide a la tesorería del relay que la fondee por ti:
eso ocurre en silencio dentro del flujo de envío, y una wallet patrocinada nunca ve
una pantalla de fondeo. Solo cuando el patrocinio se rechaza, la app muestra una
solicitud de recarga: mandas una cantidad pequeña del token nativo a la dirección de
la cuenta de gas que te muestra, y te dice por qué no hubo patrocinio.

**La comisión de activación la pagas tú** cuando no hay patrocinio gratis, es decir
cuando:

- **La tesorería de Vela para esa red está vacía o baja**: el fondo gratuito de esa
  cadena se agotó por un rato.
- **Ya usaste tu cuota gratuita**: el patrocinio tiene tope por wallet, y pasadas
  las primeras veces corre por tu cuenta.
- **El relay de Vela no fondea esa red en absoluto**: por ejemplo **redes
  personalizadas o de prueba que agregaste tú**, para las que Vela no mantiene
  tesorería. (Mándalas a tu propio relay si prefieres saltarte la activación por
  completo.)

El depósito de activación **no es reembolsable**: es el saldo inicial de la cuenta
del relay y se repone con el tiempo con los reembolsos de gas, aunque aun así puede
agotarse y necesitar **reactivación** más adelante. La dirección del relay también
puede cambiar con una actualización del servicio, lo que pide una activación nueva.

La comisión sale de tu saldo en el **activo de comisiones** que elegiste, el token
nativo por defecto. Si un envío se bloquea por gas, es que tu saldo en ese activo no
alcanza para la comisión; donde el relay ofrece gas en stablecoin, cambiar el activo
en la pantalla de confirmación puede destrabarlo.

Cuando envías el monto **máximo** de un token nativo, Vela aparta automáticamente lo
suficiente para el gas, para que la transacción no falle.

## Cómo habla Vela con cada red

Vela lee saldos y envía transacciones por un **grupo de endpoints RPC**, no por un
solo proveedor. Junta endpoints de varias fuentes, los califica por latencia y
confiabilidad y **conmuta automáticamente** cuando uno está lento o caído —dejando
en la banca temporalmente a los malos— para que un nodo inestable nunca tumbe la
app.

Sigue: [cómo funcionan las passkeys](/es-MX/docs/passkeys).
