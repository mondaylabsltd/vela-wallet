---
title: Firmantes y llaves de seguridad
description: "Una wallet de Vela puede tener hasta siete firmantes (passkeys, un celular cercano o una llave de seguridad tipo YubiKey), y cualquiera de ellos firma. Se eligen al crear la wallet; esta página explica por qué y qué hacer si una llave queda comprometida."
source: 072891bd4768
---

# Firmantes y llaves de seguridad

Una wallet de Vela es un Safe, y un Safe tiene dueños. La tuya puede tener **hasta
siete**, con un umbral de **uno**: cualquier firmante puede autorizar una
transacción por sí solo. Eso se escribe `1-of-n`.

## Qué puede ser un firmante

Tres tipos, y los puedes combinar como quieras:

| Método | Qué es | Ejemplo típico |
| --- | --- | --- |
| **Plataforma** | El autenticador integrado en el dispositivo que estás usando | Face ID / Touch ID en este celular o esta laptop, sincronizado por el Llavero de iCloud o el Administrador de contraseñas de Google |
| **Dispositivo cercano** | Otro dispositivo al que llegas escaneando un código | Tu celular firmando por tu computadora, con el transporte híbrido de WebAuthn |
| **Llave de seguridad** | Un autenticador removible, por USB o NFC | YubiKey y otras llaves FIDO2 |

Los tres son credenciales WebAuthn sobre la curva **P-256**. Para el Safe son
indistinguibles: cada uno es un dueño cuya firma el módulo de passkey de Safe
verifica on-chain de la misma manera. (La primera llave la verifica el firmante
compartido de Safe; cada llave adicional, su propio contrato firmante pequeño, que
la fábrica de Safe crea la primera vez que la wallet se despliega en una cadena.)

Qué tipos puede usar cada app:

| App | Este dispositivo | Celular cercano (QR) | Llave de seguridad |
| --- | --- | --- | --- |
| Wallet web, extensión de navegador | Sí | Sí | USB o NFC, a través del navegador |
| Escritorio (macOS, Windows, Linux) | macOS y Windows | Sí | USB |
| Android | Sí (con servicios de Google Play) | Sí | USB |
| iOS | Sí | Sí | YubiKey USB-C o Lightning, firmware 5.8 o posterior |

La opción «este dispositivo» de la app de escritorio (Touch ID, Windows Hello), y
su compatibilidad con Windows en general, son recientes y están menos probadas que
las otras vías; hoy, ahí, lo confiable es un celular o una llave de seguridad.

Una llave de seguridad puede ser tu **primer** firmante, no solo un respaldo. Si
prefieres que tu wallet nunca dependa de una cuenta de Apple o de Google, créala con
**dos** llaves de seguridad y guarda una en un lugar seguro. (No se puede crear una
wallet cuya única llave no esté sincronizada en ningún lado: la app te pide una
segunda llave, porque perder ese único dispositivo sería perder la wallet.)

## Por qué se eligen al crear la wallet

Esta es la parte que sorprende a la gente, así que aquí va el mecanismo en lugar de
una disculpa.

La dirección de tu wallet se **deriva** de su conjunto de dueños. Vela la calcula
con `CREATE2` a partir de los datos de configuración del Safe (que incluyen la llave
pública de cada firmante) antes de desplegar nada on-chain. Eso es lo que te permite
recibir fondos en una dirección que todavía no existe.

Para la dirección, la consecuencia es aritmética: **un conjunto de llaves distinto
es una dirección distinta**. Agregar otro firmante después no ampliaría tu wallet;
calcularía una wallet nueva, en una dirección nueva, sin nada de tu dinero.

Así que la pregunta «¿puedo agregar una llave después?» tiene dos respuestas
honestas:

- **Antes de fondearla**: sí. La dirección todavía no está comprometida con nada,
  así que vuelve a crear la wallet con las llaves que quieras.
- **Después de fondearla**: la dirección es donde está tu dinero. El propio Safe
  puede cambiar de dueños en una cadena donde tu wallet ya está desplegada, pero en
  cada cadena donde todavía no lo está, la misma dirección sigue representando las
  llaves originales, así que los conjuntos de dueños se irían separando de una
  cadena a otra. Mantenerlos sincronizados entre cadenas es posible (algunas smart
  wallets lo hacen), pero Vela no lo ha construido, así que no ofrece cambios de
  dueños. Planea el conjunto de llaves al crear la wallet.

## De qué te protege esto en realidad

**De perder un dispositivo.** Con más de un firmante, perder el celular es una
molestia: otra llave firma. Con un solo firmante y la sincronización del sistema
desactivada, perder el celular es perder la wallet; por eso «tu passkey se
sincroniza automáticamente» describe un ajuste que tú controlas, no una garantía
que podamos darte.

**De una cuenta de plataforma en la que ya no confías.** Si tu passkey vive en el
Llavero de iCloud o en el Administrador de contraseñas de Google, quien controle esa
cuenta podría usarla. Una llave de seguridad la tienes tú y no se sincroniza en
ningún lado.

Y de lo que **no** te protege, porque `1-of-n` corta en los dos sentidos: agregar una
segunda llave agrega una segunda forma de *entrar*, no un segundo candado.
Cualquiera que consiga cualquiera de tus firmantes puede firmar solo. Más llaves
significan más resistencia ante una pérdida y más superficie ante un robo; ese es el
intercambio, y la decisión es tuya.

## Si una llave puede estar comprometida

Una llave no se puede quitar. Si una de tus llaves puede estar en manos de alguien
más (un celular desbloqueado que se perdió, un código de desbloqueo que alguien vio,
una cuenta de Apple o de Google que ya no controlas), **pasa todo a una wallet
nueva** creada con llaves en las que confíes. Esa llave puede seguir gastando desde
la dirección vieja en todas las redes, incluidos los fondos que alguien le envíe
después.

## Recuperar no es lo mismo que agregar

Son cosas distintas, y la documentación las mantiene separadas:

- [Recuperación e inicio de sesión](/es-MX/docs/recovery): volver a una wallet
  existente en un dispositivo nuevo con una llave que ya tienes.
- Esta página: decidir, de entrada, qué llaves existen.
