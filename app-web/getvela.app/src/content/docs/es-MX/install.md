---
title: Instalar Vela
description: "Todas las formas de usar Vela (web, extensión de navegador, escritorio y celular): cuánto cuesta cada una, qué puede hacer y qué necesita tu dispositivo."
source: fa80f5cfdb95
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Instalar Vela

La misma wallet funciona en varios lugares, y en todos abre la misma dirección con
las mismas llaves. Elige según lo que necesites; puedes usar más de uno. Las
descargas están en [Obtener Vela](/es-MX/get-started).

| | Qué es | Costo | Estado |
| --- | --- | --- | --- |
| **Web** | [wallet.getvela.app](https://wallet.getvela.app/) en cualquier navegador reciente | Gratis | Disponible |
| **Extensión de navegador** | La wallet en tu barra de herramientas; se conecta a dApps | Gratis | Se descarga y se carga a mano; todavía no está en la Chrome Web Store |
| **Escritorio** | App nativa para macOS, Windows y Linux | Gratis | Descarga desde Obtener Vela o GitHub |
| **iPhone, Android** | Apps nativas | Pago único en las tiendas | Todavía no están en las tiendas; puedes compilarlas desde el código fuente |

<a href="https://wallet.getvela.app/" target="_blank" rel="noopener" style="display:inline-block;margin:4px 0 8px;padding:11px 22px;border-radius:10px;background:#e8572a;color:#fff;font-weight:600;text-decoration:none;">Abrir la wallet web →</a>

## Web

No hay nada que instalar. Abre [wallet.getvela.app](https://wallet.getvela.app/),
crea una wallet o inicia sesión, y ahí está. Tu lista de cuentas se guarda en este
navegador; en otro dispositivo, simplemente vuelves a iniciar sesión con una de tus
llaves.

## Extensión de navegador

Para navegadores basados en Chromium: Chrome, Edge y Brave (Chrome 116 o posterior).
Pone la wallet en la barra de herramientas y deja que las dApps se conecten a ella
directamente. Mientras no esté en la Chrome Web Store:

1. Descarga la extensión desde [Obtener Vela](/es-MX/get-started) y descomprímela
   en una carpeta que vayas a conservar: el navegador la ejecuta desde ahí.
2. Abre `chrome://extensions` y activa el **Modo de desarrollador**.
3. Haz clic en **Cargar descomprimida** y elige esa carpeta.

Es la misma wallet: la extensión y la wallet web usan las mismas passkeys de
`getvela.app`, así que las mismas llaves abren la misma dirección.

## Escritorio

Una app nativa, no una página web dentro de una ventana: **Windows** 10 y 11 (x64 y
ARM), **macOS** 11 o posterior, y **Linux** (.deb, .rpm o Flatpak, x64 y ARM).

- **Windows** te advertirá que «protegió su PC», porque el instalador todavía no
  tiene firma de código. Elige **Más información** y luego **Ejecutar de todas
  formas**.
- Las versiones para **macOS** las firma y notariza Apple en un paso aparte, así que
  pueden quedarse atrás de las otras plataformas. Si el botón de Mac dice «Muy
  pronto», la versión notarizada más reciente para Mac está en la página de
  versiones de GitHub.
- **Linux**: para usar una llave de seguridad USB, tu sistema tiene que darle acceso
  a la app; los paquetes .deb y .rpm instalan esa regla por ti.

En macOS y Windows, la app de escritorio trae un navegador integrado para dApps. Las
sumas de verificación de cada paquete están en la
[página de versiones de GitHub](https://github.com/mondaylabsltd/vela-wallet/releases),
y puedes comprobar más que una suma de verificación: mira más abajo.

## iPhone y Android

Apps nativas para iOS 17.4 o posterior y Android 10 o posterior. Se venderán como
pago único en App Store y Google Play; **todavía no están en las tiendas**. El
código es abierto, así que puedes compilarlas tú mismo gratis, con una diferencia:
una versión que firmas tú no puede usar las passkeys del propio celular para wallets
de getvela.app, aunque sí funcionan escanear con otro celular y las llaves de
seguridad USB. Consulta [compilar las apps tú mismo](/es-MX/docs/self-hosting#web-app).

## Verifica lo que descargaste

Una suma de verificación te dice que dos archivos son idénticos. No te puede decir
quién hizo el archivo, y la lista de sumas de verificación está en la misma página
que la descarga. Por eso cada paquete que adjuntamos a una versión también viene
**atestiguado**: la ejecución del flujo de trabajo que lo compiló firma una
declaración que nombra el archivo, el commit y esa misma ejecución, y GitHub la
conserva. Comprobarlo es un solo comando con la
[CLI de GitHub](https://cli.github.com) (inicia sesión una vez con `gh auth login`;
la comprobación es gratis):

```bash
gh attestation verify vela-wallet_0.9.4_amd64.deb --repo mondaylabsltd/vela-wallet
```

Imprime quién compiló el archivo y desde qué commit, o falla. Para esa respuesta,
nada en tu equipo tiene que confiar en nosotros: la firma es de GitHub, hecha al
momento de compilar, y no la puede producir alguien que solo vuelve a subir un
archivo en otro lado.

Las imágenes para Mac están firmadas con nuestro Developer ID y notarizadas por
Apple, algo que macOS comprueba por ti cuando abres una. Para preguntárselo tú mismo:

```bash
xcrun stapler validate VelaWallet-0.9.4-macos-arm64.dmg
spctl -a -t open --context context:primary-signature -v VelaWallet-0.9.4-macos-arm64.dmg
```

<Callout type="warning" title="El aviso de Windows se queda">
El atestiguado no es firma de código. El instalador de Windows no tiene firma de
código, así que SmartScreen lo sigue deteniendo una vez con «Windows protegió su
PC»: elige <strong>Más información</strong> y luego <strong>Ejecutar de todas
formas</strong>. Verificar el atestiguado es la comprobación que te dice que el
archivo sí es nuestro; el aviso es por un certificado que no hemos comprado.
</Callout>

Los paquetes publicados antes de que esto se activara solo traen sus sumas de
verificación.

## Usar Vela con dApps

<span id="dapps"></span>

Las dApps se conectan a Vela igual que a cualquier wallet de navegador (EIP-1193 y
EIP-6963):

- en un navegador de computadora, a través de la **extensión de Vela para el
  navegador**;
- dentro de la **app de escritorio** (macOS, Windows), la **app de iPhone** y la
  **app de Android**, a través de su navegador integrado.

La wallet web de wallet.getvela.app no se conecta a dApps, y no hay WalletConnect.
Cada solicitud que hace una dApp se decodifica y se te muestra antes de que firmes;
consulta [firma legible](/es-MX/docs/clear-signing).

## Qué necesita tu dispositivo

Vela firma con **passkeys**, que son compatibles con casi cualquier dispositivo de
los últimos años:

| Dispositivo | Compatible |
| --- | --- |
| iPhone, iPad, Mac | iOS / iPadOS 16 o posterior; macOS con un Safari o Chrome reciente |
| Android | Un Android reciente con servicios de Google Play, o una llave de seguridad USB |
| Windows | Windows Hello con Chrome o Edge, o una llave de seguridad |
| Linux | Una llave de seguridad, o un celular cerca (escaneando el código QR) |

Si tu dispositivo no puede guardar una passkey por sí mismo, usa otro celular o una
llave de seguridad física. En [Firmantes y llaves de seguridad](/es-MX/docs/signers)
está qué tipos de llave admite cada app.

## Las únicas direcciones oficiales

- **getvela.app**: este sitio y las descargas
- **wallet.getvela.app**: la wallet web
- **github.com/mondaylabsltd**: el código y los paquetes de cada versión

<Callout type="warning" title="Verifica antes de instalar">
Si algo te manda a otro lugar para «instalar Vela» o para «verificar tu wallet»,
detente. Vela nunca te pide una frase semilla: no tiene ninguna.
</Callout>

Sigue: [crea tu wallet](/es-MX/docs/create-wallet).
