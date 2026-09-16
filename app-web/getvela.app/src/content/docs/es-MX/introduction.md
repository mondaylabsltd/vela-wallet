---
title: Introducción
description: Qué es Vela, para quién es y las ideas detrás de una wallet inteligente de autocustodia sin frase semilla.
---

# Introducción

Vela es una **wallet inteligente de autocustodia** para redes EVM. Las llaves son
tuyas, pero no hay frase semilla que anotar: firmas con una passkey, con tu cara o
tu huella.

Esta documentación cubre cómo empezar, crear una wallet, mover tokens y entender
el modelo de seguridad que hay detrás.

## La versión corta

- **Autocustodia.** Tus fondos los controla una llave que solo tú puedes usar.
  Vela (la empresa) no puede mover, congelar ni recuperar tu dinero.
- **Sin frase semilla.** Tu llave de firma es una passkey guardada en el hardware
  seguro de tu dispositivo. No hay doce palabras que perder ni que te puedan robar
  con phishing.
- **Una cuenta inteligente Safe.** Cada wallet es un contrato
  [Safe](https://github.com/safe-fndn/safe-smart-account), operado con abstracción
  de cuentas ERC-4337: justo eso es lo que te deja firmar con una passkey y leer
  cada transacción antes de aprobarla.
- **12 redes, una sola dirección.** Ethereum, BNB Chain, Polygon, Arbitrum,
  Optimism, Base, Avalanche, Gnosis, Unichain, Tempo, Monad y World Chain — más
  las redes que agregues tú — todas en la misma dirección.
- **Firma legible.** Donde existe un descriptor, la transacción se traduce a una
  intención legible (ERC-7730); donde no, Vela se va a una decodificación de mejor
  esfuerzo y te avisa. Las llamadas que no puede leer las marca, no las esconde.
- **Código abierto.** La wallet y todos sus servicios están
  [públicos en GitHub](https://github.com/mondaylabsltd/vela-wallet), para que
  cualquiera verifique qué hacen.
- **Software en alfa.** Vela funciona y ya guarda dinero real, pero no lleva años
  de producción encima. Empieza con montos chicos. El
  [post sobre la alfa](/blog/vela-is-in-alpha) explica qué significa eso.

## Para quién es

Vela es para quien quiere autocustodia de verdad sin la trampa de administrar una
frase semilla — y para quien ya se quemó con eso. Si sabes desbloquear tu
teléfono, sabes usar Vela.

## A dónde ir después

- [Instalar Vela](/es-MX/docs/install) — corre en tu navegador, no hay nada que
  descargar.
- [Crea tu wallet](/es-MX/docs/create-wallet) — tu primera wallet en cosa de un
  minuto.
- [Cómo funcionan las passkeys](/es-MX/docs/passkeys) — el modelo de seguridad,
  explicado en claro.
- [Whitepaper](/es-MX/docs/whitepaper) — la arquitectura completa y el modelo de
  confianza.

Si te interesa más el *porqué* que el *cómo*, el [blog](/blog) cuenta cómo se está
construyendo Vela.
