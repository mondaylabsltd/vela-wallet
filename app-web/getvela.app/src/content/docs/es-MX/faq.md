---
title: Preguntas frecuentes
description: Preguntas comunes sobre Vela — custodia, passkeys, cuentas inteligentes, recuperación, redes soportadas, comisiones y privacidad.
---

# Preguntas frecuentes

## ¿Vela es de autocustodia?

Sí. Tu wallet es una cuenta inteligente controlada por una llave que solo tú puedes
usar, guardada por el sistema operativo de tu dispositivo y que Vela nunca ve. Vela
no puede mover, congelar ni recuperar tus fondos.

## ¿Mi wallet es una cuenta normal o un contrato?

Es una **cuenta inteligente Safe** (un contrato), operada con abstracción de
cuentas ERC-4337. Eso es lo que te permite firmar con una passkey, leer cada
transacción antes de aprobarla y usar la misma dirección en cada red. La
arquitectura está en el [whitepaper](/es-MX/docs/whitepaper).

## ¿De verdad no hay frase semilla?

De verdad. Tu llave de firma es una passkey guardada por el sistema operativo de tu
dispositivo, y Vela nunca la ve. No hay doce palabras que anotar, perder o que te
roben con phishing. Por qué eso es seguro:
[cómo funcionan las passkeys](/es-MX/docs/passkeys).

## ¿Qué pasa si pierdo mi teléfono?

Si tu passkey se sincroniza por el Llavero de iCloud o el Gestor de contraseñas de
Google, inicias sesión en un dispositivo nuevo con la misma cuenta y tu wallet
regresa. El modelo completo y sus límites están en
[recuperación e inicio de sesión](/es-MX/docs/recovery).

## ¿Qué redes y tokens están soportados?

Vela trae **12 redes EVM** — Ethereum, BNB Chain, Polygon, Arbitrum, Optimism,
Base, Avalanche, Gnosis, Unichain, Tempo, Monad y World Chain — más redes
personalizadas, con tokens nativos y ERC-20. Tu dirección es la misma en todas. Ve
[redes y comisiones](/es-MX/docs/networks-and-fees).

## ¿Cuánto cuesta usarla?

La wallet es gratis y Vela **no tiene token**. Pagas el **gas** de la red desde tu
propio saldo, más una comisión del relay. El precio lo cotiza el relay y se muestra
**antes de que firmes**, desglosado en _comisión de red / comisión del relay /
total_: el costo exacto de cada transacción está en la pantalla de confirmación, y
el monto cotizado forma parte de lo que firmas, así que no puede cambiar después.
Las transacciones muy baratas pueden toparse con una comisión mínima pequeña. En
Tempo, que no tiene moneda nativa, el gas se liquida en stablecoins en dólares.
Cada red además necesita un pequeño **depósito no reembolsable para activar su
cuenta de relay de gas** (Vela lo cubre para usuarios nuevos cuando puede); como esa
cuenta se puede agotar, quizá tengas que reponerlo más adelante, así que no es
estrictamente un pago único. Detalles en
[redes y comisiones](/es-MX/docs/networks-and-fees).

## ¿Qué puede ver o hacer Vela (la empresa)?

Vela guarda la llave **pública** de tu passkey y el **nombre** que elegiste, para
permitir el inicio de sesión entre dispositivos. No ve tu llave privada, tus saldos
se leen de cadenas públicas y no hay registro por correo. La versión que manda es
el [aviso de privacidad](/privacy).

## ¿Vela es de código abierto?

Sí: la wallet y sus cuatro servicios de backend (datos de cadena, índice de
passkeys, relay, tipos de cambio) están
[públicos en GitHub](https://github.com/mondaylabsltd/vela-wallet) con licencia
MIT, y puedes hospedarlos tú.

## Tengo una pregunta que no está aquí.

Abre un issue en [GitHub](https://github.com/mondaylabsltd/vela-wallet) o
escríbenos en [X](https://x.com/realvelawallet) o
[Telegram](https://t.me/velawallet).
