---
title: Aloja tú mismo la página de firma
description: "La página y la extensión de Chrome sin dependencias que decodifican una transacción por su cuenta y la firman con tu passkey: cómo ejecutar tu propia copia y qué copia puede firmar para tu wallet."
source: c81389ef7aa2
---

# Aloja tú mismo la página de firma

Vela decodifica cada transacción antes de que la apruebes, y esa decodificación es
un trabajo honesto, pero lo hace la misma app que armó la transacción. Si alguien
altera la app, o el camino por el que te llega, puede mostrarte una cosa y firmar
otra. Eso es exactamente lo que le pasó a [Bybit](/es-MX/docs/bybit-attack).

La página de firma existe para partir eso en dos: la transacción viene de un lugar,
y la revisión y la firma ocurren en otro que controlas tú.


Cuando recibe una solicitud de firma, no confía en el resumen que viene con ella.
Decodifica el calldata crudo por su cuenta, calcula su propio digest, te muestra lo
que la firma va a autorizar en realidad y solo entonces se lo pide a tu passkey.

Como no hay paso de compilación, los archivos que lees son los archivos que se
ejecutan. Puedes comparar la carpeta con el repositorio y saber qué estás
sirviendo.

## Qué copia puede firmar para tu wallet

Una passkey está ligada al dominio en el que se creó. Tus llaves de Vela están
registradas bajo `getvela.app`, y un navegador solo las ofrece a una página cuya
relying party sea `getvela.app`. Esa sola regla decide qué forma de ejecutar tu
propia copia te sirve.

**Como página en tu propio dominio, o en localhost.** Servida por HTTPS (o desde
localhost), la relying party de la página es su propio nombre de host, así que puede
firmar con llaves registradas bajo _ese_ nombre de host, no con llaves registradas
bajo `getvela.app`. Por eso es la forma correcta de probar toda la ceremonia de
punta a punta, de ejecutar el flujo de escritorio y de firmar para una wallet cuya
llave se creó en tu propio dominio. No es una forma de firmar para una wallet
existente de `getvela.app`.

```sh
cd app-web/trusted-signer
python3 -m http.server 8080   # → http://localhost:8080
```

Todas las rutas de la app son relativas, así que también funciona en un subdirectorio
de un host que ya tengas, y abrir `index.html` directamente desde el disco
(`file://`) sirve para darle una vuelta: sin origen no hay relying party, y no se
puede firmar nada.

## Qué hace antes de firmar

- **Decodifica la transacción por su cuenta.** Qué hace la llamada, a quién y por
  cuánto, a partir del calldata, incluidas las llamadas anidadas dentro de un lote.
- **Solo firma un digest que calculó ella.** Los digests EIP-191, EIP-712, SafeOp y
  SafeMessage se calculan en la página y se contrastan con `vela-core`, el mismo
  código que usa la wallet. Un digest que no puede calcular es un rechazo, no una
  firma.
- **Comprueba que la transacción sea la que se solicitó.** La llamada que pidió el
  sitio tiene que estar realmente dentro de la operación que se firma.
- **Rechaza una aprobación de nivel «ilimitado».** No es una advertencia: es un
  rechazo, con una indicación de qué hacer en su lugar.
- **Dice cuando no puede leer algo,** en lugar de mostrar un resumen amigable que no
  puede respaldar.
- **Muestra la dirección y el identicon de la cuenta,** y no muestra un nombre de
  destinatario proporcionado por quien pidió la firma. Todo lo que controla el
  solicitante se descarta o se etiqueta como suyo.

## Lo que a propósito no tiene

- **Nada de editores.** La solicitud queda fija cuando llega: la firmas o no. Un
  selector de comisión o un editor de montos autorizados reescribiría el calldata,
  que es justo el mal que esta página existe para evitar.
- **No crea llaves.** La página de firma no puede crear una passkey. Crear una sería
  crear otra cuenta.
- **Nada de datos de la red.** Nada de lo que muestra o firma se obtiene de fuera. Lo
  único que carga son los logos de los tokens, como imágenes, desde el servidor de
  datos de cadena de Vela; si fallan, una letra ocupa su lugar.

## Cómo le llega una solicitud

| Solicitante                                  | Canal                                                                      |
| -------------------------------------------- | -------------------------------------------------------------------------- |
| Una página en el mismo navegador             | `postMessage`                                                              |
| Una página en el mismo navegador, hacia la extensión | Puerto de la extensión                                             |
| Una app de escritorio en la misma computadora | Fragmento de URL + callback de loopback (demo en `samples/`; la app de escritorio de Vela todavía no lo usa) |
| Un celular u otra computadora                | Bluetooth LE (protocolo implementado; el radio todavía no se ha probado en hardware real) |

El formato de transmisión, los digests y una tabla de dónde sale cada elemento de la
pantalla están en `PROTOCOL.md`, junto al código.

## Dónde encaja

Una vez que las apps puedan pasarle sus solicitudes, el uso previsto es sencillo:
desde el día en que la cuenta tenga dinero que no quieras perder, cada firma pasa por
una página cuyo código cargaste tú mismo. No solo para montos grandes: una aprobación
pequeña puede entregar lo suficiente para vaciar una cuenta. Mientras tanto, la
página es una forma de leer y probar exactamente cómo va a funcionar esa segunda
opinión.
