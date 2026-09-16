---
title: Firma legible
description: Vela decodifica las transacciones a lenguaje claro antes de que las apruebes —intención, montos, direcciones y riesgo— en vez de hexadecimal opaco. Cuando no puede decodificar una llamada, te avisa en lugar de fingir.
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Firma legible

La mayoría de las wallets te pide aprobar un muro de hexadecimal y esperar lo
mejor. La «firma a ciegas» —aprobar llamadas que en realidad no puedes leer— está
detrás de buena parte de las wallets vaciadas. La respuesta de Vela es la **firma
legible**: antes de que firmes, la transacción se traduce a algo que puedas
entender.

## Qué ves

En vez de calldata en crudo, Vela muestra:

- **Intención** — qué hace la transacción: *Enviar*, *Aprobar*, *Intercambiar*,
  etcétera.
- **Lo sustancial** — los montos y direcciones involucrados, con los montos de
  tokens en unidades reales y los destinatarios resueltos a un nombre cuando
  existe.
- **Los detalles** — nonce, fechas límite y la calldata en crudo, disponibles
  cuando los pidas en vez de encajados en la cara.
- **Una indicación de riesgo**, por colores, para que lo que da miedo lo parezca.

## Cómo funciona (ERC-7730)

Vela decodifica tanto **llamadas a contratos** como **datos tipados EIP-712** con
descriptores
[ERC-7730](https://github.com/LedgerHQ/clear-signing-erc7730-registry): definiciones
pequeñas y compartibles de qué significan las funciones de un contrato.

- Cuando existe un **descriptor específico del contrato**, la transacción se marca
  como **verificada** y se etiqueta con el nombre del contrato.
- Cuando no, Vela cae a **descriptores estándar** para las formas comunes —tokens
  ERC-20, NFT ERC-721, bóvedas ERC-4626 y permits ERC-2612— así que la mayoría de
  las acciones cotidianas igual se decodifican.

Los montos de tokens se formatean con los **decimales reales on-chain** del token.
Vela nunca asume 18; si no puede confirmar los decimales, muestra el valor pero
**lo marca como no verificado** en lugar de adivinar.

## Niveles de riesgo

Cada transacción decodificada recibe un nivel de riesgo para que los patrones
peligrosos salten a la vista:

- **Precaución** para aprobaciones y permits: estás dando poder de gasto.
- **Peligro** para lo genuinamente riesgoso, como una **aprobación de token
  ilimitada**.
- Menor riesgo para acciones rutinarias como hacer staking o depositar.

<Callout type="warning" title="Las aprobaciones ilimitadas se bloquean">
Un «approve» que otorga una asignación ilimitada es una de las formas más comunes
de que los fondos se vacíen más adelante. Vela hace más que marcarlas: reescribe la
solicitud a un monto finito que tú eliges, y una revisión final antes del envío se
niega a mandar cualquier aprobación que siguiera siendo ilimitada. Esa barrera lee
la calldata en crudo directamente, así que funciona incluso cuando no existe
descriptor para el contrato.
</Callout>

## Cuando Vela no puede decodificar una llamada

La honestidad importa más que una pantalla limpia. Cuando no hay descriptor
ERC-7730 pero la función aparece en una base pública de selectores, Vela decodifica
la llamada de forma genérica y la etiqueta como **mejor esfuerzo** —decodificada,
pero no verificada— bajo un aviso de precaución. Si ni eso funciona, o si Vela solo
puede decodificar parte de una transacción, **no** finge entenderla.

<Callout type="danger" title="Advertencia explícita de firma a ciegas">
Si una llamada no se puede decodificar, Vela muestra una advertencia clara de firma
a ciegas en vez de un resumen falsamente amable. Si solo puede resolver algunos
campos, te dice que la vista es parcial y mantiene alto el nivel de riesgo. Siempre
sabes cuánto de lo que estás firmando alcanzó a leer Vela.
</Callout>

## Por qué importa

Autocustodia significa que nadie puede revertir una transacción mala por ti. La
defensa no es una mesa de ayuda: es entender qué apruebas **antes** de aprobarlo.
La firma legible convierte «confía en este bloque opaco» en «esto es exactamente lo
que hace». Dónde encaja en el modelo de seguridad general está en el
[whitepaper](/es-MX/docs/whitepaper).
