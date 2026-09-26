---
title: Trusted Signer
description: "Una página de un solo archivo en sign.getvela.app que decodifica una solicitud y la firma con tu passkey por su cuenta: qué revisa, qué apps la usan y cómo recompilarla o ejecutar tu propia copia."
source: 43a1af6ffdcd
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Trusted Signer

Vela decodifica cada transacción antes de que la apruebes, y esa decodificación es
un trabajo honesto, pero lo hace la misma app que armó la transacción. Si alguien
altera la app, o el camino por el que te llega, puede mostrarte una cosa y firmar
otra. Eso es exactamente lo que le pasó a [Bybit](/es-MX/docs/bybit-attack).

El Trusted Signer existe para partir eso en dos: la app solo entrega la solicitud, y
la revisión y la firma ocurren en una página aparte, una que puedes leer de principio
a fin, recompilar byte por byte o ejecutar tú mismo.

## Dónde se ejecuta

La página oficial se sirve desde **sign.getvela.app**. Las apps de escritorio (macOS,
Windows, Linux), iPhone y Android pueden enviarle una solicitud: la app abre la página
en una pestaña del navegador con la solicitud en el enlace, ahí la revisas y la firmas
con tu passkey, y la página le devuelve la firma a la app mediante un enlace
`velawallet://`. La wallet web no puede usarlo.

Es opcional. Lo eliges como tu forma de firmar al crear una wallet o al iniciar
sesión, y a partir de entonces cada firma de esa wallet en ese dispositivo pasa por
él. También puede crear las llaves de la wallet. En `sign.getvela.app` usa las mismas
passkeys de `getvela.app` que las apps.

<Callout type="info" title="Lo que se ha probado hasta ahora">
Ejecuciones de punta a punta registradas contra la página publicada: Android y
Windows 11 (crear una wallet e iniciar sesión). Las apps de macOS, Linux e iPhone usan
la misma conexión; ninguna tiene todavía una ejecución completa registrada.
</Callout>

## Qué hace antes de firmar

- **Decodifica la solicitud por su cuenta.** Qué hace la llamada, a quién y por
  cuánto, a partir del calldata, incluidas las llamadas anidadas dentro de un lote.
- **Solo firma un digest que calculó él mismo.** Los digests EIP-191, EIP-712, SafeOp
  y SafeMessage se calculan en la página, nunca se toman del solicitante; hay pruebas
  que contrastan los digests SafeOp y SafeMessage con `vela-core`, el código que usa la
  wallet, y la app rechaza una firma sobre cualquier digest distinto del que calculó
  ella misma.
- **Comprueba que la transacción sea la que se solicitó.** La llamada que pidió el
  sitio tiene que estar realmente dentro de la operación que se firma; si no, la página
  la rechaza.
- **Avisa cuando una aprobación es ilimitada.** No puede cambiar un monto (firma los
  bytes que llegaron o nada), así que una aprobación o un permiso ilimitados (2^128 o
  más en esta página) se muestran en rojo con ese motivo y se pueden firmar tal como
  están; el tope on-chain se elige en la pantalla de aprobación de la propia wallet,
  antes de que la solicitud llegue aquí. Una aprobación para toda una colección de NFT
  se rechaza.
- **Rechaza lo que no puede respaldar:** `eth_sign`, un método que no conoce, un token
  enviado al contrato del propio token, una operación que no puede leer y un inicio de
  sesión cuyo desafío (challenge) proporcionó el solicitante.
- **Rechaza lo que entregaría tu cuenta,** con la misma regla que aplican las apps:
  una llamada de tu cuenta a una de sus propias funciones de propietarios, módulos,
  guard o fallback, también dentro de un lote; un `delegatecall`, salvo hacia el
  contrato MultiSend de Safe, que agrupa las llamadas de una operación; y una firma
  `SafeTx`. Revisa cada llamada de la operación que armó la app, no solo las que
  pidió el sitio.
- **Muestra la dirección de la cuenta y un identicon calculado en la página.** Los
  destinatarios y los contratos nunca se nombran a partir de la solicitud: solo la
  tabla revisada de la propia página puede ponerle nombre a un contrato. El nombre de
  la cuenta, que la app envía para que elijas la passkey correcta, se muestra junto a
  su dirección.
- **Pide verificación de usuario** (tu huella, tu rostro o tu PIN) en cada firma.

## Lo que a propósito no tiene

- **Nada de editores.** La solicitud queda fija cuando llega: la firmas o no. Un
  selector de comisión o un editor de montos autorizados reescribiría el calldata,
  que es justo el mal que esta página existe para evitar.
- **Nada de acceso a la red.** La página es un solo archivo cuya política de seguridad
  de contenido (`default-src 'none'`) está dentro de sus propios bytes, así que no
  puede descargar nada, abrir una conexión ni cargar una imagen. Lo único que sale de
  ella es su respuesta, cuando sigue el enlace de callback de la solicitud
  (`velawallet://` cuando la pidió una app de Vela). Los logos de los tokens se dibujan
  como letras.

## Lo que la app revisa a cambio

La app tampoco confía en la página. Solo acepta una firma cuando el desafío firmado es
el digest **que calculó la app**, se hizo la verificación de usuario, la llave es una
de las de tu wallet y la firma P-256 es válida para esa llave.

## Cada versión publicada, verificable

Cada versión se compila desde `app-web/trusted-signer/src/` en un solo archivo, de
forma reproducible (Bun y Node producen los mismos bytes), y se publica en su propia
dirección, `sign.getvela.app/b/<sha256>/sign.html`, junto a todas las versiones
anteriores. La lista está en `sign.getvela.app/index.json`.

```sh
cd app-web/trusted-signer
node samples/build-single.mjs --check   # rebuilds a version listed in dist/
curl -sL https://sign.getvela.app/b/<sha256>/sign.html | shasum -a 256
```

Al arrancar, la app de escritorio descarga la versión publicada que va a abrir,
calcula su hash y lo compara con las versiones que trae integradas. El resultado solo
se registra en el log, y una página que no coincide se abre de todos modos. Las apps
de celular todavía no hacen esta revisión.

## Ejecuta tu propia copia

Ajustes guarda la dirección de la página que abren tus apps, así que puedes apuntarla
a tu propio despliegue: cualquier dirección HTTPS, o `localhost` para pruebas.
Compílala con `bun samples/build-single.mjs` (o `node`) y copia `dist/` a tu host.

Una copia en tu propio dominio firma con passkeys creadas para **ese** dominio, no con
las passkeys de `getvela.app`; así que sirve para crear y usar una wallet cuyas llaves
viven bajo tu dominio, no para firmar por una wallet existente de `getvela.app`. Todas
las llaves de una wallet comparten un mismo dominio.

El código, y los scripts que lo compilan y lo verifican, están en
`app-web/trusted-signer/`.
