---
title: Instalar Vela
description: "Vela corre en tu navegador: sin instalación y sin tienda de apps. Abre la wallet web, o revisa primero qué necesita tu dispositivo para las passkeys."
---

# Instalar Vela

Vela corre **en tu navegador**: no hay nada que descargar ni tienda de apps por la
que pasar. Abre la wallet web y en menos de un minuto puedes crear o restaurar una
wallet.

<a href="https://wallet.getvela.app/" target="_blank" rel="noopener" style="display:inline-block;margin:4px 0 8px;padding:11px 22px;border-radius:10px;background:#e8572a;color:#fff;font-weight:600;text-decoration:none;">Abrir la wallet web →</a>

La misma wallet, hecha desde una sola base de código, también corre en iOS y
Android. **Las apps móviles nativas vienen pronto** y, cuando salgan, tu passkey y
tu wallet se pasan tal cual, porque la cuenta vive on-chain y no dentro de una app
en particular.

## Qué necesita tu dispositivo

Vela firma con **passkeys** (WebAuthn), así que necesitas un dispositivo y un
navegador que las soporten — o sea, prácticamente cualquier cosa de los últimos
años:

| Plataforma | Soporte de passkeys | Sincronizado por |
| -------- | --------------- | --------- |
| iPhone / iPad / Mac | iOS/iPadOS 16+, Safari reciente | Llavero de iCloud |
| Android | Android 9+, Chrome actual | Gestor de contraseñas de Google |
| Escritorio | Chrome, Edge, Safari, Firefox actuales | El servicio de passkeys de tu plataforma |

Para que tu wallet te siga a un dispositivo nuevo, deja prendida la sincronización
de passkeys de tu plataforma (Llavero de iCloud en Apple, Gestor de contraseñas de
Google en Android/Chrome). Cómo funciona está en
[recuperación e inicio de sesión](/es-MX/docs/recovery).

## Las únicas URL oficiales

Vela es de código abierto, y ese es justo el punto — pero también significa que
debes asegurarte de estar en el sitio real. Las únicas direcciones oficiales son:

- **getvela.app** — este sitio
- **wallet.getvela.app** — la wallet

Si algo te manda a otro lado a «instalar Vela», detente y compara con estas dos. El
código es público en
[github.com/mondaylabsltd/vela-wallet](https://github.com/mondaylabsltd/vela-wallet).

Sigue: [crea tu wallet](/es-MX/docs/create-wallet).
