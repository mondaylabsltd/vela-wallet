---
title: El ataque a Bybit y el camino que usó
description: "En febrero de 2025, Bybit perdió unos 1,500 millones de dólares. Los contratos de Safe no fallaron; la interfaz sí. Esta página explica el camino que siguió el ataque y qué parte del diseño de Vela lo cierra."
source: ac56b16b531f
---

# El ataque a Bybit y el camino que usó

El 21 de febrero de 2025, Bybit perdió alrededor de **1,500 millones de dólares**
de una wallet fría multifirma de Safe. Es el robo más grande en la historia de la
industria, y vale la pena leerlo con cuidado, porque casi todo en él era *correcto*
salvo una cosa.

## Qué pasó

La versión corta, según los análisis públicos posteriores al incidente:

1. Un atacante comprometió la **computadora de un desarrollador de
   `Safe{Wallet}`** e inyectó JavaScript malicioso en el bucket de AWS S3 que
   servía el front-end de `Safe{Wallet}`. El código entró el 19 de febrero y se
   activó el 21 de febrero, dirigido específicamente al Safe de Bybit.
2. Los firmantes de Bybit abrieron la interfaz y revisaron una transacción que se
   veía normal.
3. Lo que en realidad se envió a sus **wallets de hardware** no era esa
   transacción. Era un `delegatecall` que sobrescribía el `masterCopy` del proxy del
   Safe (el slot 0) y reemplazaba toda la implementación de la cuenta por la del
   atacante.
4. Los firmantes aprobaron. Las firmas eran válidas. El contrato hizo exactamente lo
   que se le ordenó.

La atribución pública apuntó a actividad vinculada con Corea del Norte (el FBI
nombró al grupo TraderTraitor).

## Qué *no* falló

- **Los contratos de Safe, no.** Ejecutaron una instrucción firmada válidamente. No
  se explotó ningún bug de Safe.
- **La criptografía, no.** Todas las firmas eran auténticas.
- **Las wallets de hardware, no.** Había dispositivos Ledger en el proceso y firmaron
  de todos modos, porque una wallet de hardware te muestra lo que le dan, y lo que
  le dieron fue la carga maliciosa. Un dispositivo que no puede decodificar un
  `delegatecall` en algo que una persona pueda evaluar protege la *llave*, no la
  *decisión*.

Lo que falló fue el supuesto que está debajo de toda interfaz de wallet: **que la
pantalla que describe una transacción y los bytes que se firman son lo mismo.**

## Por qué es el caso general y no un evento raro

La mayoría de las firmas que se producen en una wallet web descansan en ese
supuesto. La interfaz arma la carga, la interfaz muestra el resumen, y nada
independiente comprueba que uno corresponda con el otro. Si se reemplaza el código
que sirve esa interfaz (por un pipeline de compilación comprometido, un CDN
secuestrado, una dependencia maliciosa o una credencial de despliegue robada), el
resumen pasa a ser lo que el atacante quiera, y tu firma es real.

Ese es el riesgo al que apunta el diseño de firma de Vela. No el phishing. No una
llave filtrada. **Una pantalla de firma que te está mintiendo.**

## Qué hace Vela al respecto

**Firma legible, hasta el calldata.** Cada transacción se decodifica en una
intención legible antes de que la apruebes: monto, destinatario, qué hace en
realidad la llamada ([ERC-7730](/es-MX/docs/clear-signing)). Una llamada que no
podemos decodificar se **marca como no decodificable**, no se muestra en silencio
como si estuviera bien. La carga de Bybit era un `delegatecall` que cambiaba una
dirección de implementación; es justo el tipo de cosa que debería frenar en seco a
un firmante, y esconderla detrás de un resumen amigable es la razón por la que no lo
hizo.

Hay dos límites que conviene precisar. Una dApp no puede pedirle a Vela un
`delegatecall` directamente (las solicitudes que puede hacer una página producen
llamadas normales), así que la carga de Bybit en sí no podría llegar por ese camino.
Pero una página *sí* puede pedir una llamada de tu Safe a sí mismo:
`enableModule`, `addOwnerWithThreshold`, `setFallbackHandler`, `setGuard`.
Cualquiera de ellas, firmada una sola vez, entrega la cuenta tan completamente como
lo hizo la carga de Bybit: un módulo habilitado puede luego ejecutar un
`delegatecall` propio. Vela decodifica esas llamadas pero todavía no las bloquea;
**rechaza cualquier solicitud cuyo destino sea la dirección de tu propia wallet.** Y
si el código de Vela se reemplazara, como pasó con el de `Safe{Wallet}`, la
decodificación también sería la del atacante; para eso es el siguiente punto.

**Un camino independiente que puede revisar la interfaz.** Vela construyó una
[página de firma](/es-MX/docs/clear-signing-self-host) sin compilación y sin
dependencias que decodifica la solicitud y hace la firma WebAuthn por su cuenta: una
sola carpeta de archivos estáticos que puedes leer de principio a fin, servir tú
mismo o cargar como extensión del navegador. Su propósito es ser una segunda opinión
que no comparta la cadena de suministro de la app principal. *Estado: construida y
probada; no publicada, y ninguna app de Vela le envía solicitudes todavía.* Esta
página lo dirá claramente cuando eso cambie.

**Ningún rol de administrador que podamos perder.** Las cuentas de Vela son
[Safe v1.4.1 sin modificar](/es-MX/docs/account-contract), y Vela no tiene ningún
rol privilegiado en ellas: ni llave de administrador ni ruta de actualización propia
que nos pudieran obligar o engañar a usar. Hay que ser claros sobre lo que eso *no*
elimina: la primitiva que usaron los atacantes de Bybit (un `delegatecall` firmado
por un dueño que reescribe la implementación de la cuenta) sigue existiendo en todo
Safe, incluido el de Vela (las transacciones en lote de la propia Vela usan
`delegatecall` hacia el MultiSend de Safe). Requiere una firma válida de una de tus
llaves. Las defensas contra que te convenzan de darla son la decodificación de
arriba y la revisión independiente.

**Una verificación nueva para cada firma.** Cada firma necesita la confirmación de
tu propia llave: Face ID, una huella, un PIN, o un toque y un PIN en una llave de
seguridad. No hay una llave de sesión de larga duración, así que no hay ninguna
ventana en la que algo pueda firmar por ti sin que estés presente.

**El autoalojamiento como último respaldo.** Las apps y los servicios de backend son
de código abierto. Si no quieres confiar para nada en nuestro pipeline de
compilación, compila tú mismo la extensión o una app y opera los servicios que
necesites; la [guía de autoalojamiento](/es-MX/docs/self-hosting) te lleva paso a
paso. Eso saca nuestro pipeline de compilación de la cadena de confianza; sigues
confiando en el código que compilas, así que léelo.

## Lo que Vela no afirma

El front-end de Vela podría quedar comprometido de la misma forma que el de
`Safe{Wallet}`. Nuestro código no está auditado. Decir otra cosa sería justo el tipo
de garantía que este incidente debió haber terminado.

Lo que el diseño intenta es angostar el camino: hacer que la carga sea legible en
lugar de opaca, no tener ningún rol de administrador del que se pueda abusar en tu
nombre y darte una forma de verificar con algo que no seamos nosotros. El resumen
honesto es que **esta clase de ataque está mitigada por diseño, no eliminada**, y
las partes que la reforzarían más están enumeradas, sin terminar, en
[auditorías y problemas conocidos](/es-MX/docs/security-audits).

## Fuentes

- [SlowMist — Bybit's $1.5 billion theft unveiled: `Safe{Wallet}` front-end code tampered](https://slowmist.medium.com/bybits-1-5-billion-theft-unveiled-safe-wallet-front-end-code-tampered-84b78f0fa9c2)
- [NCC Group — Bybit hack: in-depth technical analysis](https://www.nccgroup.com/research/in-depth-technical-analysis-of-the-bybit-hack/)
- [Sygnia — Investigation into the Bybit hack](https://www.sygnia.co/blog/sygnia-investigation-bybit-hack/)
- [BlockSec — A web2 breach enables the largest crypto hack in history](https://blocksec.com/blog/bybit-incident-a-web2-breach-enables-the-largest-crypto-hack-in-history)
