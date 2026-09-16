---
title: Firmantes y llaves de seguridad
description: Una wallet Vela puede tener hasta siete firmantes — passkeys, un dispositivo cercano o una llave de seguridad tipo YubiKey — y basta cualquiera de ellos para firmar. Se eligen al crear la wallet, y esta página explica por qué eso no es una limitación que se nos olvidó quitar.
---

# Firmantes y llaves de seguridad

Una wallet Vela es un Safe, y un Safe tiene dueños. La tuya puede tener **hasta
siete**, con umbral de **uno**: cualquier firmante puede autorizar una transacción
por su cuenta. Se escribe `1-of-n`.

## Qué puede ser un firmante

Tres tipos, y los puedes mezclar libremente:

| Método | Qué es | Ejemplo típico |
| --- | --- | --- |
| **Plataforma** | El autenticador integrado en el dispositivo que estás usando | Face ID / Touch ID en este teléfono o laptop, sincronizado por el Llavero de iCloud o el Gestor de contraseñas de Google |
| **Dispositivo cercano** | Otro dispositivo al que llegas escaneando un código | Tu teléfono firmando para tu computadora, por el transporte híbrido de WebAuthn |
| **Llave de seguridad** | Un autenticador removible en USB o NFC | YubiKey y otras llaves FIDO2 |

Las tres son credenciales WebAuthn en la curva **P-256**. Para el Safe son
indistinguibles: cada una es un dueño cuya firma revisa igual el verificador
WebAuthn on-chain.

Una llave de seguridad puede ser tu **primer** firmante, no solo un respaldo. Si
prefieres que tu wallet nunca dependa de una cuenta de Apple o Google, esa es la
configuración que lo logra: registra una YubiKey al crearla y firma con ella.

## Por qué se eligen al crear

Esta es la parte que sorprende, así que aquí va el mecanismo en lugar de una
disculpa.

La dirección de tu wallet se **deriva** de su conjunto de dueños. Vela la calcula
con `CREATE2` a partir de los datos de configuración del Safe —que incluyen la
llave pública de cada firmante— antes de que se despliegue nada on-chain. Eso es
justo lo que te permite recibir fondos en una dirección que todavía no existe.

La consecuencia es aritmética, no política: **un conjunto de llaves distinto es una
dirección distinta**. Agregar un octavo firmante después no extendería tu wallet;
calcularía una wallet nueva, en una dirección nueva, sin un centavo de tu dinero
adentro.

Así que «¿puedo agregar una llave después?» tiene dos respuestas honestas:

- **Antes de meterle fondos:** sí — la dirección no se ha comprometido con nada,
  vuelve a crear la wallet con las llaves que quieras.
- **Después de meterle fondos:** esa dirección es donde está tu dinero. Cambiar los
  dueños de un Safe ya desplegado es una operación de Safe que Vela hoy no expone.
  Planea el conjunto de llaves al crear.

## De qué te protege esto en realidad

**Perder un dispositivo.** Con más de un firmante, perder el teléfono es una
molestia: firma otra llave. Con exactamente un firmante y la sincronización del
sistema apagada, perder el teléfono es perder la wallet — por eso «tu passkey se
sincroniza automáticamente» describe una configuración que tú controlas, no una
garantía que podamos dar por ti.

**Una cuenta de plataforma en la que ya no confías.** Si tu passkey vive en el
Llavero de iCloud o en el Gestor de contraseñas de Google, quien controle esa cuenta
podría llegar a usarla. Una llave de seguridad la tienes tú y no se sincroniza a
ningún lado.

Y de lo que **no** te protege, porque `1-of-n` corta para los dos lados: agregar una
segunda llave agrega una segunda *entrada*, no un segundo candado. Quien obtenga
cualquiera de tus firmantes puede firmar solo. Más llaves significa más resistencia
a la pérdida y más superficie ante el robo; ese es el trato, y lo decides tú.

## Recuperar no es agregar

Son dos cosas distintas, y la documentación las mantiene separadas:

- [Recuperación e inicio de sesión](/es-MX/docs/recovery) — volver a una wallet
  existente en un dispositivo nuevo con una llave que ya tienes.
- Esta página — decidir, desde el principio, qué llaves existen.
