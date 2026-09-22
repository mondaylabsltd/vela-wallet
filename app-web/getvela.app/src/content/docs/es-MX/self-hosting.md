---
title: Guía de autoalojamiento
description: "Todo lo que Vela opera por ti, qué hace cada pieza y cómo reemplazarla por la tuya (el relay, el índice de llaves públicas, los datos de cadena, los tipos de cambio y las apps), además de lo único que no puedes reemplazar y cómo arreglártelas sin getvela.app."
source: 5ae6005a9396
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Guía de autoalojamiento

Tu dinero está en un contrato Safe on-chain, controlado por tus llaves. Nada de lo
que opera Vela puede moverlo. Lo que Vela sí opera es la maquinaria que hace cómoda
la wallet: un relay que envía tus transacciones, un índice que ayuda a un
dispositivo nuevo a encontrar tu wallet, un directorio de datos de cadena, una
fuente de tipos de cambio y las propias apps.

Esta página enumera cada una de esas piezas, qué deja de funcionar sin ella y cómo
operar la tuya. También trata la única pieza que no puedes reemplazar (el dominio al
que pertenecen tus passkeys) y qué hacer si getvela.app desaparece.

<Callout type="info" title="Para quién es esta página">
Te conviene tener soltura con una terminal, con Docker o Cloudflare Workers, y con
fondear una dirección en una cadena. Nada de esto hace falta para usar Vela en el
día a día.
</Callout>

## El mapa

| Pieza | Qué hace | Valor predeterminado de Vela | ¿Puedes reemplazarla? | Sin ella |
| --- | --- | --- | --- | --- |
| **Relay** | Recibe tu operación firmada, paga el gas, la envía y cobra la comisión que firmaste | `vela-relay-cf.getvela.app` | Sí: opera [vela-relay](#relay) y apunta la wallet a él | No puedes enviar |
| **Índice de llaves públicas** | Registra on-chain las llaves de una wallet nueva; responde «¿de qué wallet forma parte esta llave?» | `p256-index-v2.getvela.app` | Sí: opera [p256-index](#index) | No se pueden crear wallets nuevas; el inicio de sesión recurre a leer la cadena |
| **Contrato de registro** | El registro público y permanente de las llaves de cada wallet | `0x94fD…1EA9` en Gnosis | No hace falta: no tiene dueño y la wallet lo lee directamente | — |
| **Datos de cadena** | Detalles de las redes, listas de tokens, logos, descriptores de firma legible | `ethereum-data.getvela.app` | Sí: opera [ethereum-data](#chain-data) | Sin listas de tokens ni logos; se decodifican menos transacciones; agregar redes falla |
| **Tipos de cambio** | Valores en tu moneda de visualización | `vela-currency.getvela.app` | Sí: opera [vela-currency](#exchange-rates) o cualquier fuente compatible con Frankfurter | Las apps recurren a los precios on-chain de Chainlink donde pueden (la de escritorio muestra USD) |
| **Nodos RPC** | Leer saldos, simular transacciones | Endpoints públicos por red | Sí: por red, en Ajustes → Redes | Vela cambia de un endpoint a otro |
| **Las apps** | La wallet en sí | wallet.getvela.app, versiones publicadas | Sí: [compílalas](#web-app) | — |
| **getvela.app** | El dominio al que pertenecen tus passkeys | — | **No**: consulta [más abajo](#if-getvela-app-disappears) | — |

También se contactan algunos servicios de terceros que no son de Vela: las bases de
datos públicas de selectores de funciones (sourcify, openchain, 4byte), que se usan
como último recurso al decodificar una transacción; el directorio de autenticadores
que pone nombre al modelo de tu llave de seguridad; y los servidores de túnel de
Apple y Google cuando firmas con un celular escaneando un código QR.

## Lo único que no puedes reemplazar: el dominio de la passkey

<span id="if-getvela-app-disappears"></span>

Una passkey pertenece al sitio web para el que se creó. Las llaves de Vela se crean
para `getvela.app`. Los navegadores solo las ofrecen a páginas de getvela.app o de
sus subdominios (o a orígenes que getvela.app declara como relacionados), y las
passkeys integradas de un celular solo funcionan en apps que getvela.app avala.
Fuera del navegador la regla es más flexible: Chrome deja que una extensión con
permiso para getvela.app las use, y un programa en tu computadora puede pedirle
directamente a una llave de seguridad o a un celular una firma para getvela.app;
así funcionan las apps compiladas por ti, y por eso importa qué software ejecutas.
De ahí salen dos cosas.

**Una copia de la wallet web en tu propio dominio es otra wallet.** Servido desde
`wallet.example.com`, el mismo código crea passkeys para `wallet.example.com`:
llaves nuevas y, por lo tanto, una dirección nueva. No puede firmar para una wallet
creada en wallet.getvela.app. Esa copia sigue siendo útil: para una wallet que crees
ahí, o para operar tú mismo todo el stack desde cero.

**Para una wallet existente, esto sigue funcionando si getvela.app está caído o
desaparece:**

| Forma de entrar | Llaves que puede usar | Dónde conseguirla |
| --- | --- | --- |
| La **extensión de Vela para el navegador** (navegadores Chromium: Chrome, Edge, Brave) | Cualquier llave a la que llegue el navegador: la passkey de este dispositivo, una llave de seguridad USB (NFC donde la computadora lo admita), un celular por QR | Un zip de una versión publicada en [GitHub](https://github.com/mondaylabsltd/vela-wallet/releases), o [compílala](#web-app) |
| Una **app de escritorio o de celular que compiles tú** | Un celular por QR y llaves de seguridad USB | [Compílala](#web-app) |
| Las **apps de las tiendas y las apps de escritorio notarizadas** | Siempre un celular por QR y llaves de seguridad; las passkeys de «este dispositivo» solo mientras el sistema operativo todavía pueda verificar la app contra getvela.app | Versiones publicadas en GitHub (las tiendas, más adelante) |

La extensión puede usar llaves de `getvela.app` porque Chrome deja que una extensión
con permiso para un sitio use las passkeys de ese sitio. El navegador comprueba ese
permiso de forma local; lo medimos funcionando, aunque todavía no con el dominio
realmente caído. Una app compilada por ti puede usar un celular o una llave de
seguridad porque Vela habla con ellos directamente; la passkey del propio celular
(«este dispositivo») necesita que la app esté firmada por Vela, y la tuya no lo
está.

La [página de firma](/es-MX/docs/clear-signing-self-host) no es una forma de entrar
por sí sola: firma solicitudes que le envía otro programa, y ninguna app de Vela se
las envía todavía.

<Callout type="warning" title="Quien controla el dominio puede pedir una firma">
Cualquier página servida desde getvela.app o uno de sus subdominios (o por quien
controle el dominio en el futuro) puede pedirles a tus llaves una firma, y el aviso
del sistema muestra «getvela.app», no la transacción. Así funcionan las passkeys en
todas partes. Por esa razón, el sitio web de Vela prohíbe que sus propias páginas
usen passkeys. También por eso importan la extensión y las apps compiladas por ti:
llevan su propio código, aunque por defecto siguen obteniendo descriptores y usando
servicios bajo getvela.app.
</Callout>

## Apunta la wallet a tus servicios

Cada app tiene cuatro campos en **Ajustes → Avanzado → Endpoints de servicio** (en
escritorio, **Ajustes → Endpoints de servicio**): **Índice de datos de cadena**,
**Índice de passkey**, **Vela Relay** y **Tipos de cambio fiat**. Cada campo muestra
el valor predeterminado de Vela hasta que lo cambias; **Restablecer a los valores
predeterminados** restaura los cuatro. Para el relay, el índice y los datos de
cadena, la wallet llama a `/api/health` y muestra un indicador, que solo se pone en
verde cuando el endpoint dice ser el servicio correcto y reporta `status: "ok"`.
Guarda lo que escribas de cualquier forma, así que espera a que se ponga en verde.

| Servicio | `service` en `/api/health` |
| --- | --- |
| Relay | `vela-relay` |
| Índice de llaves públicas | `webauthn-p256-publickey-registry` |
| Datos de cadena | `ethereum-data` |
| Tipos de cambio | no se revisa por nombre; tiene que devolver una lista de tipos de cambio con base en USD |

Qué tan bien respeta hoy cada app estos ajustes:

| App | Endpoints de servicio | RPC por red |
| --- | --- | --- |
| Web y extensión | Datos de cadena, relay y tipos de cambio. El índice de passkey se usa para buscar nombres, pero crear una wallet e iniciar sesión siguen usando el índice de Vela | Sí |
| Escritorio | Los cuatro; un índice de passkey nuevo se aplica después de reiniciar o cerrar sesión | Sí |
| Android | Los cuatro, salvo que buscar nombres para direcciones todavía consulta el índice de Vela | Sí |
| iOS | **Todavía no**: la página muestra valores de ejemplo y no guarda. El índice de passkey se puede cambiar en la pantalla de inicio de sesión cuando el predeterminado no responde | Solo lectura |

Estos huecos son bugs, y tienen seguimiento.

## Opera tu propio relay

<span id="relay"></span>

El relay es [vela-relay](https://github.com/mondaylabsltd/vela-relay) (Rust, MIT).
Un solo despliegue sirve para todas las cadenas: la wallet llama a
`https://your-relay/<chainId>`. Tiene que ser vela-relay: la wallet pide la
cotización de la comisión con un método propio de Vela que los bundlers ERC-4337
genéricos no implementan.

**Qué necesitas**

- O bien Docker más un Redis y un servidor [Iggy](https://iggy.apache.org) que ya
  operes, o bien una cuenta de Cloudflare con **Workers Paid**, y Node.js y una
  toolchain de Rust (con el target `wasm32-unknown-unknown`) en tu máquina.
- Un `OPERATOR_SECRET` (hex, de al menos 32 bytes). De él se derivan una dirección
  de tesorería y un conjunto de direcciones de relayer, las mismas en todas las
  cadenas. Mantenlo en secreto: controla los fondos del relay.
- Gas en cada cadena que quieras atender: envía la moneda de la cadena (pathUSD en
  Tempo) a tu dirección de tesorería. La tesorería recarga a los relayers.

**Docker**

```sh
git clone https://github.com/mondaylabsltd/vela-relay
cd vela-relay
cp .env.example .env
# en .env: VELA_RELAY_IGGY_URL, VELA_RELAY_REDIS_URL, OPERATOR_SECRET,
# VELA_RELAY_CHAIN_DIRECTORY_URL si operas tus propios datos de cadena,
# y VELA_RELAY_IMAGE con una imagen publicada en la que confíes (ver docs/docker.md)
docker compose pull relay
docker compose up -d --no-build
curl --fail http://127.0.0.1:4567/readyz
```

Mejor usa la imagen publicada: compilar desde el código fuente con
`docker compose up --build` puede fallar con el Dockerfile actual. Sin Docker,
`cargo run --release --bin vela-relay` lo ejecuta directamente.

**Cloudflare Workers**

```sh
cd vela-relay/vela-relay-cf
npx wrangler queues create vela-relay-ops
npx wrangler queues create vela-relay-dlq
npx wrangler secret put OPERATOR_SECRET
# datos de cadena propios: agrega "VELA_RELAY_CHAIN_DIRECTORY_URL" en "vars" de wrangler.jsonc
npx wrangler deploy
```

**Compruébalo**

```sh
curl https://your-relay/api/health        # {"service":"vela-relay","status":"ok",…}
curl https://your-relay/v1/treasury/100   # tu dirección de tesorería en Gnosis, y si necesita gas
```

Luego pon `https://your-relay` en el campo **Vela Relay**.

**Toma en cuenta**

- La comisión que paga la wallet va a tu tesorería. La wallet la calcula igual sin
  importar qué relay uses (consulta [redes y comisiones](/es-MX/docs/networks-and-fees)).
- Una red personalizada que hayas agregado antes de cambiar el relay conserva la
  dirección del relay con la que se agregó.
- El relay lee los detalles de cada cadena y las stablecoins que acepta de un
  directorio de cadenas: `ethereum-data.getvela.app`, a menos que configures
  `VELA_RELAY_CHAIN_DIRECTORY_URL` con [el tuyo](#chain-data). Esta variable existe
  desde septiembre de 2026; una versión anterior del relay siempre lee la copia de
  Vela.

## Opera tu propio índice de llaves públicas

<span id="index"></span>

El índice es [p256-index](https://github.com/mondaylabsltd/p256-index) (Rust, MIT).
Cuando se crea una wallet, verifica la prueba de cada llave, luego escribe el grupo
en el **contrato de registro** en Gnosis y paga el gas. Sigue usando el registro
existente en `0x94fD1A891EB6c5F340622Baf2F3A0cb70A941EA9`: no tiene dueño,
cualquier dirección con fondos puede escribir en él y todas las apps de Vela lo leen
directamente. Un registro propio sería invisible para ellas.

**Qué necesitas**

- Docker con Redis e Iggy (el servidor), o una cuenta de Cloudflare (la versión
  Worker, cuyo propio README señala que su escritura on-chain todavía no se ha
  probado de punta a punta).
- Una llave privada de Gnosis con xDAI. Registrar una wallet cuesta alrededor de
  1.1 millones de unidades de gas con una llave y alrededor de 3.6 millones con
  siete.
- Estos ajustes:

```dotenv
P256_INDEX_IGGY_URL=iggy+tcp://user:password@iggy.example:5100
P256_INDEX_REDIS_URL=redis://redis.example:6379/0
P256_INDEX_CONTRACT_ADDRESS=0x94fD1A891EB6c5F340622Baf2F3A0cb70A941EA9
P256_INDEX_DOMAIN_REGISTRY=0x5266DfF591B9F9EecfEdb8E7EfEf6c687854edaf
PRIVATE_KEY=0x…
```

`P256_INDEX_DOMAIN_REGISTRY` importa aunque el archivo de ejemplo del servidor no lo
incluya: sin él, el servidor entrega desafíos que el contrato rechaza, y todos los
registros fallan.

**Ejecútalo y compruébalo**

```sh
git clone https://github.com/mondaylabsltd/p256-index
cd p256-index
cargo run --release -p p256-index-server
curl https://your-index/api/health   # "service":"webauthn-p256-publickey-registry","status":"ok"
```

El servidor escucha en HTTP simple (en el puerto 11256 por defecto); ponle enfrente
un proxy con TLS, porque la wallet solo acepta endpoints `https://`. Al momento de
escribir esto, puede que el Dockerfile del código fuente no compile; compilar con
Cargo sí funciona.

**Si no responde ningún índice**, las wallets existentes siguen funcionando: al
iniciar sesión, la app lee el contrato de registro en Gnosis (y luego en Ethereum) a
través de tus nodos RPC. Una wallet con una sola llave incluso se puede reconstruir
a partir de dos firmas, sin ningún registro de por medio. Crear una wallet nueva sí
necesita un índice, porque algo tiene que pagar el registro.

## Opera tus propios datos de cadena

<span id="chain-data"></span>

Los datos de cadena son [ethereum-data](https://github.com/atshelchin/ethereum-data)
(MIT): JSON estático e imágenes de unas 2,600 redes y sus tokens, además de los
descriptores ERC-7730 que usa Vela para explicar las transacciones.

```sh
docker run -d --name ethereum-data -p 3000:3000 --restart unless-stopped \
  ghcr.io/atshelchin/ethereum-data:latest
curl http://localhost:3000/api/health   # "service":"ethereum-data","status":"ok"
```

Su README también explica cómo compilarlo desde el código fuente y desplegarlo en
Cloudflare. Sírvelo por HTTPS y pon la dirección en el campo **Índice de datos de
cadena**.

El relay también lee estos archivos, incluido un campo propio de Vela (la lista
`stables` decide qué stablecoins pueden pagar comisiones). Apúntalo a tu copia con
`VELA_RELAY_CHAIN_DIRECTORY_URL=https://your-chain-data`; guarda en caché la entrada
de cada red durante una hora.

## Opera tus propios tipos de cambio

<span id="exchange-rates"></span>

[vela-currency](https://github.com/mondaylabsltd/vela-currency) (MIT) vuelve a
publicar los tipos de cambio diarios del Banco Central Europeo. No necesita llaves.

```sh
docker run -d -p 8080:8080 -v rates-data:/data ghcr.io/mondaylabsltd/vela-currency:latest
curl "http://localhost:8080/v2/rates?base=USD"
```

Pon `https://your-host/v2/rates?base=USD` en el campo **Tipos de cambio fiat**.
Cualquier servicio compatible con Frankfurter también funciona. Conserva
`?base=USD`: todas las conversiones lo dan por hecho.

## Compila las apps tú mismo

<span id="web-app"></span>

Todas las apps están en [un solo repositorio](https://github.com/mondaylabsltd/vela-wallet)
(MIT). El README enumera los pasos de compilación de cada app; la versión corta:

| App | Compilación | ¿Firma para tu wallet existente de getvela.app? |
| --- | --- | --- |
| Extensión de navegador | `cd app-web/vela-wallet && pnpm install && pnpm build:extension`, y luego carga `extension/dist` descomprimida en `chrome://extensions` | Sí, con cualquier llave |
| Wallet web | `cd app-web/vela-wallet && pnpm install && pnpm build`; se despliega como Cloudflare Worker | No: en tu dominio es otra wallet (ver arriba) |
| Escritorio | `cd app-desktop/vela-wallet && cargo run` (los scripts de empaquetado están en su README) | Sí, con un celular por QR o una llave de seguridad USB |
| Android | Genera los bindings del núcleo y luego `./gradlew :app:installDebug` | Sí, con un celular por QR o una llave de seguridad USB |
| iOS | `./rust/scripts/build-ios-xcframework.sh`, y luego compila en Xcode con tu propio equipo | Sí, con un celular por QR o una YubiKey USB-C / Lightning (firmware 5.8 o posterior) |

La passkey de «este dispositivo» de una app compilada por ti no funciona para
wallets de getvela.app: Apple y Google solo dejan que las apps firmadas por Vela
usen passkeys de `getvela.app`.

## Agrega una red que Vela no trae

Vela funciona en cualquier cadena EVM que tenga el precompilado P-256 y los
contratos estándar que revisa. La página de [configuración de cadenas](/es-MX/chain-setup)
te dice qué le falta a una cadena y despliega lo que cualquiera puede desplegar;
[redes y comisiones](/es-MX/docs/networks-and-fees) explica los requisitos. Un
hueco: una wallet con más de una llave también necesita en esa cadena la fábrica de
firmantes de passkey de Safe, que la revisión todavía no busca; sin ella, ahí solo
puede firmar la primera llave.

## Lo que sigue apuntando a Vela después de todo esto

Si reemplazas todo lo anterior, queda esto:

- **El directorio de autenticadores** que pone nombre a los modelos de llaves de
  seguridad: es cosmético; las apps recurren a un nombre genérico.
- **Los archivos de asociación de getvela.app**, que las apps de las tiendas
  necesitan para las passkeys de «este dispositivo». Un celular o una llave de
  seguridad no los necesitan.

Y esto no es de Vela: las bases de datos públicas de selectores, los túneles de
Apple y Google para iniciar sesión con el celular, y los proveedores de RPC que tú
elijas.

Sigue: [la página de firma que puedes alojar tú mismo](/es-MX/docs/clear-signing-self-host).
