---
title: Preguntas frecuentes
description: "Respuestas cortas sobre custodia, llaves, recuperación, redes, comisiones, qué puede ver Vela, el código abierto y qué pasa si Vela desaparece."
source: 7446e22f990d
---

# Preguntas frecuentes

## ¿Vela es de autocustodia?

Sí. Tu wallet es una cuenta inteligente Safe que solo controlan tus llaves, y estas
se quedan en tus dispositivos, en tu gestor de contraseñas o en tus llaves de
seguridad. Vela no tiene ninguna llave ni ningún rol en ella, así que no puede
mover, congelar ni recuperar tus fondos por su cuenta. Lo que sí hace es escribir el
software que les pide a tus llaves que firmen; consulta el
[modelo de amenazas](/es-MX/docs/whitepaper).

## ¿De verdad no hay frase semilla?

De verdad. Tus llaves son passkeys, y una passkey no tiene ningún secreto que puedas
anotar o escribir. Consulta [cómo funcionan las passkeys](/es-MX/docs/passkeys).

## ¿Qué necesito para crear una wallet?

Un dispositivo compatible con passkeys (un celular o una computadora recientes con
Face ID, huella o Windows Hello), o dos llaves de seguridad físicas. Sin correo, sin
cuenta y sin saldo inicial. Puedes crear la wallet con hasta siete llaves; no se
pueden agregar después. Consulta [crea tu wallet](/es-MX/docs/create-wallet).

## ¿Qué pasa si pierdo mi celular?

Inicia sesión en un dispositivo nuevo con cualquier otra llave: la misma passkey
sincronizada a través del Llavero de iCloud o del Administrador de contraseñas de
Google, otro celular o tu llave de seguridad. Si el celular tenía tu única llave y
esa llave no estaba sincronizada, la wallet no se puede recuperar. Consulta
[recuperación e inicio de sesión](/es-MX/docs/recovery).

## ¿Qué redes y tokens son compatibles?

24 redes EVM integradas, entre ellas Ethereum, Base, Arbitrum, Optimism, Polygon,
BNB Chain, Gnosis y Avalanche, más cualquier red EVM que agregues y que cumpla los
requisitos. Monedas nativas y tokens ERC-20. La dirección es la misma en todas las
redes. Consulta [redes y comisiones](/es-MX/docs/networks-and-fees).

## ¿Cuánto cuesta?

- **Las apps:** la wallet web, la extensión de navegador y las apps de escritorio
  son gratis. Las apps de iOS y Android serán un pago único en las tiendas; también
  puedes compilar cualquier app desde el código fuente gratis.
- **Cada transacción:** una comisión que se paga desde tu wallet al relay que la
  envía (el de Vela, a menos que apuntes la wallet a otro relay u operes el tuyo).
  Cubre el gas más el margen del relay, con un mínimo de alrededor de US$0.01. El
  monto exacto aparece en la pantalla de confirmación antes de que firmes y forma
  parte de lo que firmas. No hay depósito ni suscripción.
  [Cómo se calcula la comisión](/es-MX/docs/networks-and-fees#fee).
- **Ningún token.** Vela no tiene uno ni planea tenerlo.

## ¿Puedo usar Vela con dApps?

Sí, con la extensión de Vela para el navegador (Chrome, Edge, Brave) y con el
navegador integrado de las apps de escritorio (macOS, Windows), iOS y Android. La
wallet web de wallet.getvela.app no se conecta a dApps. Consulta
[instalar](/es-MX/docs/install#dapps).

## ¿Qué puede ver o hacer Vela?

Vela no puede leer tus llaves ni mover tus fondos por su cuenta. Sus servicios ven
tu dirección IP y lo que la app les pregunta: el índice ve tus llaves públicas y el
nombre de tu wallet cuando registra una wallet nueva, y las direcciones que
consultas; el relay ve tu dirección, las operaciones que envías y el endpoint RPC
que usa tu app; el servicio de datos de cadena ve por qué tokens y contratos
pregunta tu app. Lo que se vuelve público on-chain está en
[crea tu wallet](/es-MX/docs/create-wallet#what-is-public). La
[política de privacidad](/privacy) es la versión completa y oficial.

## ¿Vela es de código abierto?

Sí, todo, con licencia MIT: las apps de la wallet y el núcleo, el relay, el índice de
llaves públicas, el servicio de tipos de cambio y el directorio de datos de cadena,
en [GitHub](https://github.com/orgs/mondaylabsltd/repositories). Cada servicio lo
puedes operar tú; consulta la
[guía de autoalojamiento](/es-MX/docs/self-hosting).

## ¿Vela está auditada?

Los contratos donde está tu dinero (Safe y sus módulos, y el EntryPoint de ERC-4337)
están auditados. El código propio de Vela no, y no hay ninguna auditoría programada.
Consulta [auditorías y problemas conocidos](/es-MX/docs/security-audits).

## ¿Qué pasa si Vela cierra?

Tus fondos se quedan en tu Safe, on-chain. Para una wallet existente, la extensión de
Vela para el navegador y las apps que compiles tú siguen funcionando sin
getvela.app, y cada servicio es de código abierto para que alguien más lo opere. La
[guía de autoalojamiento](/es-MX/docs/self-hosting#if-getvela-app-disappears)
enumera los caminos y sus límites.

## Tengo una pregunta que no está aquí.

Abre un issue en [GitHub](https://github.com/mondaylabsltd/vela-wallet/issues), o
escríbenos en [X](https://x.com/realvelawallet) o en [Telegram](https://t.me/velawallet).
