---
title: Auditorías y problemas conocidos
description: "Cada contrato del que depende Vela, quién auditó qué versión, si la versión auditada es la que está desplegada, los hallazgos abiertos que vigilamos y lo que no se ha auditado en absoluto."
source: c4c50ad89f2f
---

«Auditado» es una afirmación sobre un código específico en una versión específica,
así que esta página cita los informes, los commits y las direcciones desplegadas, y
enumera lo que **no** está auditado, que importa igual.

Última revisión: 22 de septiembre de 2026. Si encuentras un error, avísanos y lo
corregimos.

## El camino de los fondos

Cada contrato que puede tocar tu dinero es un despliegue canónico de código de
terceros con revisiones publicadas.

### Safe v1.4.1: la cuenta en sí

Tu wallet es un proxy de [Safe](https://github.com/safe-fndn/safe-smart-account/tree/v1.4.1)
que usa el singleton SafeL2 y SafeProxyFactory. Los lotes pasan por MultiSend.

[Ackee Blockchain auditó Safe v1.4.0](https://github.com/safe-global/safe-smart-account/blob/main/docs/audit_1_4_0.md)
(informe final del 16 de marzo de 2023, revisión de correcciones el 28 de marzo): 11
hallazgos, ninguno crítico ni alto; los dos hallazgos medios se reconocieron en lugar
de cambiarse. El alcance fue SafeL2, SafeProxyFactory, CompatibilityFallbackHandler,
MultiSendCallOnly y SignMessageLib. v1.4.1 difiere de v1.4.0 en una sola línea
funcional, una corrección de compatibilidad con ERC-4337 en la configuración de
módulos ([PR #572](https://github.com/safe-global/safe-smart-account/pull/572)); Safe
consultó a Ackee y concluyó que no hacía falta volver a auditar. La lógica de
MultiSend no ha cambiado desde v1.3.0, que
[auditó G0 Group](https://github.com/safe-global/safe-smart-account/tree/main/docs).
Todas las direcciones coinciden con
[safe-deployments](https://github.com/safe-global/safe-deployments). Los contratos
centrales entran en el alcance del
[programa de recompensas por bugs de la Safe Foundation](https://docs.safefoundation.org/security/bug-bounty),
cuyo nivel más alto paga hasta 1,000,000 de dólares.

El incidente de Bybit de 2025 no es un hallazgo en los contratos: los atacantes
alteraron el JavaScript que se servía a la interfaz web de Safe, y la
[declaración forense](https://safefoundation.org/blog/safe-ecosystem-foundation-statement)
de Safe no encontró ninguna vulnerabilidad en los contratos.
[Nuestra página sobre el tema](/es-MX/docs/bybit-attack) explica por qué la misma
clase de ataque afecta a toda interfaz de wallet, incluida la nuestra.

### Safe4337Module v0.3.0: el adaptador de ERC-4337

Desplegado en `0x75cf11467937ce3F2f357CE24ffc3DBF8fD5c226` (coincidencia exacta en
Sourcify), y configurado también como el fallback handler de tu Safe. Revisado tres
veces ([informes aquí](https://github.com/safe-global/safe-modules/blob/main/modules/4337/docs/v0.3.0/audit.md)):

- **Ackee Blockchain**, informe final de marzo de 2024: una advertencia (el uso del
  optimizador del compilador) reconocida, y nada de mayor nivel abierto.
- **Certora**, agosto de 2026: un hallazgo **medio**, reconocido y **no corregido**
  en v0.3.0: *los cambios de autorización no invalidan las UserOperations
  posteriores que ya se validaron en el mismo bundle*. Consulta «Problemas
  conocidos» más abajo.
- **Nethermind**, agosto de 2026: sin hallazgos.

SafeModuleSetup v0.3.0 (`0x2dd6…5b47`), que habilita el módulo cuando se despliega
una wallet, quedó cubierto por las revisiones de Certora y Nethermind.

La historia del módulo tiene un problema divulgado: v0.1.0 no firmaba `initCode` ni
`paymasterAndData`, un vector de gas griefing
[corregido en v0.2.0](https://safefoundation.org/blog/strengthening-security-addressing-the-incident-of-the-canonical-4337-module);
Safe informa que v0.1.0 no se usó fuera de testnets. Vela usa v0.3.0 con EntryPoint
v0.7 y Safe 1.4.1, la configuración que describe la versión del módulo.

### Módulo de passkey de Safe v0.2.1: los firmantes

Tu primera llave la verifica **SafeWebAuthnSharedSigner** en
`0x94a4F6affBd8975951142c3999aEAB7ecee555c2`. «Compartido» significa que el
despliegue del contrato es compartido, igual que el singleton de Safe; tu llave no
lo es. Cada Safe guarda su propia llave pública P-256 en su propio almacenamiento.

Cada llave adicional tiene su propio contrato firmante, creado por
**SafeWebAuthnSignerFactory** en `0x1d31F259eE307358a26dFb23EB365939E8641195` como
un proxy hacia el **singleton SafeWebAuthnSigner** en
`0x4E27b51350e6c2083EE19011120F50DAfEc5CA50`.

Las revisiones que cubren estos contratos en v0.2.1
([informes](https://github.com/safe-global/safe-modules/blob/main/modules/passkey/docs/v0.2.1/audit.md)):

- Una [competencia de auditoría de Hats Finance](https://github.com/safe-global/safe-modules/blob/main/modules/passkey/docs/v0.2.1/audit-competition-report-hats.md)
  (junio y julio de 2024): ningún hallazgo alto ni medio; tres bajos, todos
  corregidos.
- La revisión de **Certora** del commit de la versión: sin hallazgos nuevos. (La
  auditoría anterior de v0.2.0 señala que el firmante compartido todavía no se había
  auditado; se agregó después de esa auditoría.)
- **Nethermind**, agosto de 2026: sin hallazgos.

No se ha divulgado ninguna vulnerabilidad a nivel de contrato desde su lanzamiento,
y los contratos de passkey entran en el alcance del programa de recompensas de la
Safe Foundation.

Las firmas de passkey las verifica el precompilado **EIP-7951 / RIP-7212** de la cadena, sin
verificador de respaldo. Antes de habilitar una red, la app prueba el precompilado
con una firma real. Dos salvedades: la especificación original de RIP-7212 tiene
fallas en casos límite que corrige [EIP-7951](https://eips.ethereum.org/EIPS/eip-7951)
(solo afectan entradas que de todos modos deberían fallar, no firmas WebAuthn bien
formadas), y una prueba no puede detectar todas las formas en que la implementación
de una cadena podría desviarse.

### EntryPoint v0.7: ejecuta tu operación

Desplegado en `0x0000000071727De22E5E9d8BAf0edAc6f37da032`, la
[versión canónica v0.7.0](https://github.com/eth-infinitism/account-abstraction/releases/tag/v0.7.0).
[Auditado por OpenZeppelin](https://www.openzeppelin.com/news/erc-4337-account-abstraction-incremental-audit)
para la Ethereum Foundation (enero de 2024): ningún hallazgo crítico ni alto, cinco
medios, y los 24 hallazgos resueltos; el commit de la revisión de correcciones
coincide con la versión publicada. Entra en el alcance del
[programa de recompensas por bugs de ERC-4337](https://docs.erc4337.io/community/bug-bounty)
de la Ethereum Foundation (hasta 250,000 dólares).

## Problemas conocidos que vigilamos

### Cambios de autorización dentro de un mismo bundle (Safe4337Module, Certora M-01)

El EntryPoint valida todas las operaciones de un bundle antes de ejecutar
cualquiera de ellas. Así que, si una operación quita a un dueño, una operación
firmada por ese dueño y colocada después en el mismo bundle sigue pasando la
validación y se ejecuta. Safe lo reconoció y no cambió v0.3.0.

Las apps de Vela nunca arman cambios de dueños, así que Vela por sí misma nunca
provoca esto. Aun así importa: una dApp puede pedirle a tu wallet que cambie sus
propios dueños (consulta «Huecos» más abajo), y quien quite una llave comprometida
con otras herramientas de Safe no podría contar con que quede bloqueada dentro del
mismo bundle.

### Intercepción de una operación firmada (EntryPoint anterior a v0.9)

En febrero de 2026, unos investigadores
[divulgaron](https://erc4337.substack.com/p/improving-useroperation-execution) un
vector de griefing y censura que afecta a todos los EntryPoint anteriores a v0.9,
incluido v0.7. Alguien que consigue una operación firmada antes de que se mine puede
ejecutarla dentro de una llamada que controla y forzar que la ejecución interna se
revierta: la operación falla y hay que volver a firmarla. (Con la comisión dentro de
la operación que usa Vela, la transferencia de la comisión también se revierte, así
que el gas lo absorbe el relay y no tú.) Afecta a las operaciones que llaman a
contratos protegidos contra reentrada o que pueden hacerse revertir con un estado
temporal; las transferencias simples no se ven afectadas. Si se usa repetidamente
contra flujos de retiro, podría dejar fondos sin disponibilidad por un tiempo. No
puede falsificar una firma ni desviar fondos.

El relay de Vela envía las operaciones directamente en lugar de a través de una
mempool compartida, pero una transacción `handleOps` pendiente sigue siendo visible
en la mempool pública, así que esto reduce la exposición en lugar de eliminarla. La
corrección solo existe en EntryPoint v0.9 (noviembre de 2025); v0.7 no se puede
parchar. Migrar depende de que el módulo 4337 de Safe sea compatible con v0.9, y esta
página dirá cuándo ocurra.

### Huecos en las defensas propias de Vela

No son hallazgos en los contratos, sino lugares donde la wallet te protege menos de
lo que podrías suponer. Cada uno tiene seguimiento para corregirse:

- **Las llamadas de tu wallet a sí misma no se bloquean.** Una dApp puede pedir
  `enableModule`, `addOwnerWithThreshold`, `setFallbackHandler` o `setGuard` sobre tu
  propio Safe; cualquiera de ellas, firmada una sola vez, entrega la cuenta. Vela
  decodifica esas llamadas pero no las detiene. Rechaza cualquier solicitud cuyo
  destino sea tu propia dirección.
- **La protección de aprobaciones solo detiene los montos «ilimitados»** (2^200 o
  más; 2^152 para Permit2). Una aprobación finita grande, un permiso firmado o un
  `setApprovalForAll` de NFT reciben una advertencia, no un bloqueo.
- **Los descriptores obtenidos no están autenticados.** Un descriptor del servidor
  de datos de cadena se muestra como «verificado» si coincide con el contrato; es
  tan confiable como ese servidor.
- **La página de firma independiente no está conectada** a ninguna app todavía.
- **La revisión de redes no busca la fábrica de firmantes de passkey de Safe**, que
  necesitan las llaves de la dos a la siete; en una red agregada sin ella, solo puede
  firmar la primera llave.
- **El sitio web carga un script de analítica de terceros** en el mismo dominio que
  las passkeys. El sitio prohíbe que sus páginas usen passkeys (con un encabezado
  Permissions-Policy) y mantiene el script fuera de la página que guarda una llave.

## Qué no está auditado

- **Los contratos propios de Vela.** El
  [registro de llaves públicas](https://github.com/mondaylabsltd/p256-index/tree/main/contracts)
  en `0x94fD1A891EB6c5F340622Baf2F3A0cb70A941EA9` (Gnosis; la misma dirección en
  Ethereum y Base), el despliegue original del registro en
  `0x5266DfF591B9F9EecfEdb8E7EfEf6c687854edaf` (su dirección forma parte del dominio
  de firma de cada registro) y el índice anterior al que reemplazaron
  (`0xdd93420BD49baaBdFF4A363DdD300622Ae87E9c3`, historial de solo lectura). No están
  auditados. No guardan fondos, no tienen dueño y no se pueden actualizar; son una
  capa de descubrimiento, no una capa de autorización. El poder de gasto viene solo
  de las llaves configuradas en tu Safe. La peor falla realista es que una wallet se
  vuelva más difícil de encontrar en un dispositivo nuevo, no que se mueva dinero.
- **Multicall3.** Su README [dice](https://github.com/mds1/multicall3) «This contract
  is unaudited» (este contrato no está auditado). Vela lo usa solo para lecturas en
  lote (saldos, detalles de tokens, cotizaciones de precios), nunca con aprobaciones
  ni fondos.
- **Los desplegadores deterministas** (el proxy CREATE2 de Arachnid y la singleton
  factory de Safe): estándares del ecosistema y sin estado, sin auditorías formales.
  La revisión de redes de Vela falla de forma segura si no están; comprueba que haya
  código en la dirección, no que coincida byte por byte.
- **Tempo.** Una de las 24 redes integradas, sin moneda nativa; ahí Vela paga el gas
  con la stablecoin pathUSD. A septiembre de 2026, la
  [política de seguridad](https://github.com/tempoxyz/.github/blob/main/SECURITY.md)
  de Tempo dice que el protocolo sigue en auditoría y no tiene un programa de
  recompensas por bugs activo. Los fondos que tengas en Tempo, y el gas que pagues
  ahí, cargan con ese riesgo a nivel de cadena; tómala como la cadena más nueva y
  menos probada de la lista.
- **La propia Vela.** Las apps, los servicios de backend y los contratos de arriba no
  han tenido una auditoría de terceros, y no hay ninguna programada. Es la salvedad
  más grande de esta página. Los detalles están en
  [Vela is in alpha](/blog/vela-is-in-alpha). Empieza con montos pequeños y lee el
  código.

## Compruébalo tú mismo

Cada dirección de abajo es un despliegue público canónico. Verifícalas contra
[safe-deployments](https://github.com/safe-global/safe-deployments),
[safe-modules-deployments](https://github.com/safe-global/safe-modules-deployments)
y la [versión publicada del EntryPoint](https://github.com/eth-infinitism/account-abstraction/releases/tag/v0.7.0):

| Contrato                                  | Dirección                                    |
| ----------------------------------------- | -------------------------------------------- |
| SafeL2 singleton v1.4.1                   | `0x29fcB43b46531BcA003ddC8FCB67FFE91900C762` |
| SafeProxyFactory v1.4.1                   | `0x4e1DCf7AD4e460CfD30791CCC4F9c8a4f820ec67` |
| MultiSend v1.4.1                          | `0x38869bf66a61cF6bDB996A6aE40D5853Fd43B526` |
| CompatibilityFallbackHandler v1.4.1 ¹     | `0xfd0732Dc9E303f09fCEf3a7388Ad10A83459Ec99` |
| SafeModuleSetup v0.3.0                    | `0x2dd68b007B46fBe91B9A7c3EDa5A7a1063cB5b47` |
| Safe4337Module v0.3.0                     | `0x75cf11467937ce3F2f357CE24ffc3DBF8fD5c226` |
| SafeWebAuthnSharedSigner v0.2.1           | `0x94a4F6affBd8975951142c3999aEAB7ecee555c2` |
| SafeWebAuthnSignerFactory v0.2.1          | `0x1d31F259eE307358a26dFb23EB365939E8641195` |
| SafeWebAuthnSigner singleton v0.2.1       | `0x4E27b51350e6c2083EE19011120F50DAfEc5CA50` |
| EntryPoint v0.7                           | `0x0000000071727De22E5E9d8BAf0edAc6f37da032` |
| Multicall3                                | `0xcA11bde05977b3631167028862bE2a173976CA11` |
| Registro de llaves públicas (Vela, sin auditar) | `0x94fD1A891EB6c5F340622Baf2F3A0cb70A941EA9` |

¹ Se revisa cuando se agrega una red; tu Safe usa en su lugar el módulo 4337 como
fallback handler.
