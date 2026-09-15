---
title: El ataque a Bybit
description: En febrero de 2025, Bybit perdió alrededor de 1,500 millones de dólares. Los contratos Safe no fallaron: falló la interfaz. Esta página explica la ruta que usó y qué cierra el diseño de Vela.
---

# El ataque a Bybit

El 21 de febrero de 2025, Bybit perdió cerca de **1,500 millones de dólares** de
una cold wallet multifirma de Safe. Es el robo más grande en la historia de la
industria y vale la pena leerlo con calma, porque casi todo estuvo *correcto* menos
una cosa.

## Qué pasó

La versión corta, según los post-mortems públicos:

1. Un atacante comprometió la **máquina de un desarrollador de `Safe{Wallet}`** e
   inyectó JavaScript malicioso en el bucket de AWS S3 que servía el frontend de
   `Safe{Wallet}`. El código entró el 19 de febrero y se activó el 21, apuntado al
   Safe específico de Bybit.
2. Los firmantes de Bybit abrieron la interfaz y revisaron una transacción que se
   veía normal.
3. Lo que de verdad llegó a sus **wallets de hardware** no era esa transacción. Era
   un `delegatecall` que sobrescribía el `masterCopy` del proxy de Safe —el slot
   0— reemplazando toda la implementación de la cuenta por la del atacante.
4. Los firmantes aprobaron. Las firmas eran válidas. El contrato hizo exactamente
   lo que se le dijo.

La atribución pública apuntó a actividad ligada a Corea del Norte (el FBI nombró al
grupo TraderTraitor).

## Qué *no* falló

- **No los contratos Safe.** Ejecutaron una instrucción firmada válidamente. No se
  explotó ningún bug de Safe.
- **No la criptografía.** Cada firma era auténtica.
- **No las wallets de hardware.** Había dispositivos Ledger en el flujo y firmaron
  igual, porque una wallet de hardware muestra lo que le dan, y lo que le dieron era
  la carga maliciosa. Un dispositivo que no puede traducir un `delegatecall` a algo
  que un humano pueda evaluar protege la *llave*, no la *decisión*.

Lo que falló es el supuesto que hay debajo de toda interfaz de wallet: **que la
pantalla que describe una transacción y los bytes que se firman son la misma cosa.**

## Por qué este es el caso general y no un accidente raro

Cada firma que has hecho en una wallet web se apoyó en ese supuesto. La interfaz
arma la carga, la interfaz dibuja el resumen, y nada independiente revisa que
coincidan. Si el código que sirve esa interfaz se reemplaza —una cadena de
compilación comprometida, un CDN secuestrado, una dependencia maliciosa, una
credencial de despliegue robada— el resumen se vuelve lo que el atacante quiera, y
tu firma es real.

Ese es el riesgo al que apunta el diseño de firma de Vela. No al phishing. No a una
llave filtrada. **A una pantalla de firma que te está mintiendo.**

## Qué hace Vela al respecto

**Firma legible, hasta la calldata.** Cada transacción se traduce a intención
legible antes de que apruebes: monto, destinatario, qué hace realmente la llamada
([ERC-7730](/es-MX/docs/clear-signing)). Una llamada que no podemos decodificar se
**marca como no decodificable**, no se dibuja en silencio como si estuviera bien. La
carga de Bybit era un `delegatecall` que cambiaba una dirección de implementación:
justo la clase de cosa que debería frenar en seco a un firmante, y esconderla tras
un resumen amable es la razón por la que no lo hizo.

**Una ruta independiente que puede revisar la interfaz.** Vela está construyendo
una página de firma sin compilación ni dependencias, que dibuja la intención y
realiza la firma WebAuthn por su cuenta: una sola carpeta de archivos estáticos que
puedes leer de principio a fin, servir tú mismo o correr como extensión del
navegador. Su propósito entero es ser una segunda opinión que no comparte la cadena
de suministro de la app principal. *Estado: construida y probada, todavía no
desplegada.* Cuando salga será opcional, y esta página lo dirá con todas sus letras
cuando eso cambie.

**Ningún contrato que podamos actualizar.** La carga de Bybit funcionó reemplazando
la implementación de la cuenta. Las cuentas de Vela son
[Safe v1.4.1 sin modificar](/es-MX/docs/account-contract) y Vela no tiene ningún rol
privilegiado sobre ellas: sin llave de administrador, sin ruta de actualización a la
que puedan obligarnos o comprometernos.

**Una verificación biométrica nueva por cada firma.** No hay llave de sesión de
larga vida, así que no hay ventana en la que algo pueda firmar por ti sin ti.

**Autohospedaje como red de seguridad.** La app y cada servicio de backend son de
código abierto. Si no quieres confiar en nuestra cadena de compilación, corre la
tuya: es la única respuesta a esta clase de ataque que no exige confiar en alguien.

## Qué no afirma Vela

El frontend de Vela podría comprometerse de la misma forma que el de
`Safe{Wallet}`. Nuestro código no está auditado. Decir otra cosa sería exactamente
el tipo de garantía con el que este incidente debió acabar.

Lo que el diseño intenta es angostar la ruta: hacer la carga legible en vez de
opaca, quitar la primitiva de actualización de la que dependió el ataque y darte una
forma de verificar con algo que no seamos nosotros. El resumen honesto es que **esta
clase de ataque está mitigada por diseño, no eliminada** — y las piezas que la
endurecerían más están listadas, sin terminar, en
[auditorías y problemas conocidos](/es-MX/docs/security-audits).

## Fuentes

- [SlowMist — Bybit's $1.5 billion theft unveiled: `Safe{Wallet}` front-end code tampered](https://slowmist.medium.com/bybits-1-5-billion-theft-unveiled-safe-wallet-front-end-code-tampered-84b78f0fa9c2)
- [NCC Group — Bybit hack: in-depth technical analysis](https://www.nccgroup.com/research/in-depth-technical-analysis-of-the-bybit-hack/)
- [Sygnia — Investigation into the Bybit hack](https://www.sygnia.co/blog/sygnia-investigation-bybit-hack/)
- [BlockSec — A web2 breach enables the largest crypto hack in history](https://blocksec.com/blog/bybit-incident-a-web2-breach-enables-the-largest-crypto-hack-in-history)
