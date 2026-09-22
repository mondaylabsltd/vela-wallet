---
title: Firma legible
description: "Vela decodifica las transacciones en lenguaje claro antes de que las apruebes (intención, montos, direcciones y riesgo) en lugar de hexadecimal opaco. Cuando no puede decodificar una llamada, te advierte en vez de fingir."
source: 7232328b724e
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Firma legible

Muchas wallets todavía muestran datos crudos para cualquier contrato que no
reconocen, y la «firma a ciegas» (aprobar llamadas que en realidad no puedes leer)
es una de las formas en que vacían wallets. La respuesta de Vela es la **firma
legible**: antes de que firmes, la transacción se decodifica en algo que puedas
entender, hasta donde se pueda.

## Qué ves

En lugar de calldata crudo, Vela te muestra:

- **La intención**: qué hace la transacción, como *Enviar*, *Aprobar*,
  *Intercambiar*, etcétera.
- **Lo esencial**: los montos y las direcciones involucradas, con los montos de
  tokens en unidades reales y los destinatarios con su nombre cuando lo tienen.
- **Los detalles**: nonce, fechas límite y el calldata crudo, disponibles cuando los
  pidas en lugar de ponértelos enfrente a la fuerza.
- **Una indicación de riesgo**, con colores para que las acciones peligrosas
  resalten.

## Cómo funciona (ERC-7730)

Vela decodifica tanto las **llamadas a contratos** como los **datos tipados
EIP-712** con descriptores
[ERC-7730](https://github.com/LedgerHQ/clear-signing-erc7730-registry): definiciones
pequeñas y compartibles de lo que significan las funciones de un contrato.

Vela busca un descriptor en este orden:

1. **Integrado en la app**: descriptores para contratos muy usados, como los routers
   de Uniswap, PancakeSwap y SushiSwap, WETH, el pool de Aave v3, 1inch, Lido y
   wstETH, y Seaport.
2. **Obtenido del servidor de datos de cadena de Vela**, que vuelve a publicar el
   registro público de ERC-7730.
3. **Formas estándar**: tokens ERC-20, NFT ERC-721 y ERC-1155, bóvedas ERC-4626 y
   permisos ERC-2612, para que la mayoría de las acciones cotidianas se sigan
   decodificando.

**Verificada** está reservada para la primera fuente. Una transacción se etiqueta
como verificada solo cuando la descripción viene de un descriptor integrado en la app
que estás usando, o del servidor de datos de cadena y es idéntica a la copia
integrada, lo que demuestra que nada se cambió en el camino. Todo lo demás que manda
el servidor se sigue decodificando y se sigue mostrando, con una línea que dice que
viene del servicio de descriptores y que nada lo autenticó. Ese servicio no está
firmado, así que es tan confiable como quien lo opere. Esa es una de las razones por
las que puedes [operar el tuyo](/es-MX/docs/self-hosting#chain-data).

Los montos de tokens se formatean con los **decimales reales on-chain** del token.
Si Vela no puede confirmar los decimales de un token, muestra el monto como si el
token tuviera 18 y lo **marca como no verificado**, para que un número equivocado
nunca parezca uno comprobado.

## Niveles de riesgo

Cada transacción decodificada recibe un nivel de riesgo para que los patrones
peligrosos resalten:

- **Precaución** para aprobaciones y permisos: estás otorgando poder de gasto.
- **Peligro** para lo que de verdad es riesgoso, como una **aprobación de tokens
  ilimitada**.
- Un riesgo menor para acciones de rutina, como hacer staking o depositar.

<Callout type="warning" title="Las aprobaciones on-chain «ilimitadas» no se pueden enviar">
Una aprobación on-chain por un monto ilimitado es una de las formas más comunes en
que después vacían fondos. Cuando una dApp pide una (<code>approve</code>,
<code>increaseAllowance</code> o el <code>approve</code> de Permit2) en el nivel
«ilimitado» (2^200 o más, o 2^152 para Permit2, que es lo que usan las dApps para
decir «ilimitado»), Vela no la envía hasta que la cambies por un monto específico,
tu saldo o una revocación; una última revisión antes del envío lee el calldata
crudo, así que funciona con o sin descriptor. Lo que no detiene: una
<strong>aprobación finita grande</strong> (aunque esté muy por encima de tu saldo),
los <strong>permisos firmados</strong> (firmas EIP-2612 y Permit2) y el
<code>setApprovalForAll</code> de NFT; cada uno se muestra con una advertencia, y la
decisión es tuya.
</Callout>

## Cuando Vela no puede decodificar una llamada

Cuando no existe un descriptor ERC-7730 pero la función aparece en una base de datos
pública de selectores, Vela decodifica la llamada de forma genérica y la etiqueta
como **mejor esfuerzo** (decodificada, pero no verificada), bajo un aviso de
precaución. Si ni eso funciona, o si Vela solo puede decodificar una parte de la
transacción, **no** finge entenderla.

<Callout type="danger" title="Advertencia explícita de firma a ciegas">
Si una llamada no se puede decodificar, Vela muestra una advertencia clara de firma
a ciegas en lugar de un resumen falsamente amigable. Si solo puede resolver algunos
de los campos, te dice que la vista es parcial y mantiene elevado el nivel de
riesgo. Siempre sabes qué tanto de lo que firmas pudo leer Vela en realidad.
</Callout>

## Por qué importa

Autocustodia significa que nadie puede revertir por ti una mala transacción. La
defensa no es una mesa de ayuda: es entender lo que apruebas **antes** de
aprobarlo. La firma legible es la forma en que Vela intenta mostrártelo, y tiene
límites: solo puede ser tan honesta como la app que la muestra, y por eso importa
una [revisión independiente](/es-MX/docs/clear-signing-self-host). Consulta el
[whitepaper](/es-MX/docs/whitepaper) para ver dónde encaja en el modelo de seguridad
de Vela.
