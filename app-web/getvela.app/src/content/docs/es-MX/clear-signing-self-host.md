---
title: Aloja tú la página de firma
description: La página sin dependencias y la extensión de Chrome que decodifican una transacción por su cuenta y la firman con tu passkey — cómo correr tu propia copia y cuál copia puede firmar por tu wallet.
---

# Aloja tú la página de firma

Vela decodifica cada transacción antes de que la apruebes, y ese trabajo es
trabajo de verdad — pero lo hace la misma app que armó la transacción. Si la app, o
la ruta por la que te llega, está alterada, puede mostrarte una cosa y firmar otra.
Es exactamente lo que le pasó a [Bybit](/es-MX/docs/bybit-attack).

La página de firma existe para partir eso en dos: la transacción viene de un lado,
y la revisión y la firma ocurren en otro que tú controlas.

## Qué es

Una carpeta —`app-web/clearsigning` en el repositorio— que es a la vez página web y
extensión de Chrome. Puro HTML, CSS y JavaScript: sin framework, sin bundler, sin
paso de compilación, sin dependencias y sin peticiones de red propias.

Cuando recibe una solicitud de firma, no confía en el resumen que viene con ella.
Decodifica la calldata en crudo por su cuenta, calcula su propio digest, te muestra
qué autorizará realmente la firma y solo entonces pide tu passkey.

Como no hay paso de compilación, los archivos que lees son los que corren. Puedes
comparar la carpeta contra el repositorio y saber qué estás sirviendo.

## Cuál copia puede firmar por tu wallet

Una passkey queda atada al dominio donde se creó. Tus llaves de Vela están
registradas bajo `getvela.app`, y un navegador solo se las ofrecerá a una página
cuya relying party sea `getvela.app`. Esa sola regla decide qué forma de correr tu
copia te sirve.

**Como extensión de Chrome: esta es la que se usa con tu wallet existente.** La
relying party de la extensión es `getvela.app` sin importar de dónde vino la
carpeta, así que tus llaves actuales pueden firmar ahí, mientras que el código es la
carpeta que cargaste e inspeccionaste.

1. Abre `chrome://extensions` y activa el **modo de desarrollador**.
2. **Cargar extensión sin empaquetar** y elige la carpeta `app-web/clearsigning`.
3. El ícono de la barra abre la página en una pestaña.

**Como página en tu propio dominio, o en localhost.** Servida por HTTP(S), la
relying party de la página es su propio nombre de host, así que puede firmar con
llaves registradas bajo *ese* host, no con llaves registradas bajo `getvela.app`.
Esa es la forma correcta de probar toda la ceremonia de punta a punta, de correr el
flujo de escritorio y de firmar por una wallet cuya llave se creó en tu propio
dominio. No es una forma de firmar por una wallet de `getvela.app` ya existente.

```sh
cd app-web/clearsigning
python3 -m http.server 8080   # → http://localhost:8080
```

Todas las rutas de la app son relativas, así que un subdirectorio en un host
existente también funciona; abrir `index.html` directo del disco (`file://`) sirve
para echar un ojo — sin origen no hay relying party y no se puede firmar nada.

## Qué hace antes de firmar

- **Decodifica la transacción por su cuenta.** Qué hace la llamada, a quién y por
  cuánto, desde la calldata — incluidas las llamadas anidadas dentro de un lote.
- **Solo firma un digest que ella misma calculó.** Los digests EIP-191, EIP-712,
  SafeOp y SafeMessage se calculan en la página y se contrastan con `vela-core`, el
  mismo código que usa la wallet. Un digest que no puede calcular es un rechazo, no
  una firma.
- **Revisa que la transacción sea la que se pidió.** La llamada que pidió el sitio
  tiene que estar realmente dentro de la operación que se va a firmar.
- **Rechaza una aprobación ilimitada.** No una advertencia: un rechazo, con una
  indicación de qué hacer en su lugar.
- **Dice cuando no puede leer algo,** en vez de mostrar un resumen amable del que no
  puede responder.
- **Muestra la dirección y el identicon de la cuenta,** y no muestra un nombre de
  destinatario suministrado por quien pidió la firma. Todo lo que controla el
  solicitante o se quita o se etiqueta como suyo.

## Qué no tiene a propósito

- **Ningún editor.** La solicitud queda fija al llegar: la firmas o no. Un selector
  de comisiones o un editor de límites reescribiría la calldata, que es justo la
  enfermedad que esta página existe para evitar.
- **No crea llaves.** La página de firma no puede crear una passkey. Crear una sería
  crear una cuenta distinta.
- **Sin peticiones de red.** Si no hay nada que traer, no hay nada que interceptar.

## Cómo le llega una solicitud

| Quien solicita | Canal |
| -------------------------------------------- | -------------------------------------------------------------------------- |
| Una página en el mismo navegador | `postMessage` |
| Una página del mismo navegador, hacia la extensión | Puerto de la extensión |
| Una app de escritorio en la misma máquina | Fragmento de URL + callback en loopback |
| Un teléfono u otra computadora | Bluetooth LE (protocolo implementado; la radio todavía no se prueba en hardware real) |

El formato de intercambio, los digests y una tabla de de dónde viene cada elemento
en pantalla están en `PROTOCOL.md`, junto al código.

## Cuándo usarla

Desde el día en que la cuenta guarda dinero que te dolería perder — y de ahí en
adelante, para cada firma. No solo para montos grandes: una aprobación chiquita
puede entregar lo suficiente para vaciar una cuenta. Un hábito de firma guardado
para ocasiones especiales no está puesto el día que hace falta.
