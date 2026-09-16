---
title: Auditorías y problemas conocidos
description: Cada contrato on-chain del que Vela depende, quién lo auditó, si la versión auditada coincide con la que está desplegada y qué no está auditado en absoluto.
---

«Auditado» es una afirmación sobre una versión específica de un código específico,
así que esta página no agita la palabra: cita los reportes exactos, las direcciones
exactas de despliegue y las diferencias entre la versión auditada y la desplegada.
También lista lo que _no_ está auditado, porque esa lista pesa tanto como la
primera.

Última revisión: agosto de 2026. Si encuentras un error aquí, dinos y lo
corregimos.

## La ruta de los fondos

Cuatro capas de contratos pueden tocar tu dinero. Las cuatro son contratos de
terceros con auditorías publicadas, y en cada caso la dirección desplegada es el
despliegue canónico oficial.

### Safe v1.4.1 — la cuenta misma

Tu wallet es un proxy [Safe](https://github.com/safe-global/safe-smart-account):
singleton SafeL2, fábrica de proxys, manejador de respaldo de compatibilidad y
MultiSend para lotes.

[Ackee Blockchain auditó Safe v1.4.0](https://github.com/safe-global/safe-smart-account/blob/main/docs/audit_1_4_0.md)
(reporte final de marzo de 2023): 11 hallazgos, ninguno crítico ni alto. El v1.4.1
que desplegamos se diferencia del v1.4.0 auditado por una corrección de
compatibilidad ERC-4337 de una sola línea
([PR #572](https://github.com/safe-global/safe-smart-account/pull/572)). La lógica
de MultiSend no ha cambiado desde el
[v1.3.0 auditado por G0 Group](https://github.com/safe-global/safe-smart-account/tree/main/docs).
Todas las direcciones coinciden con los despliegues canónicos de
[safe-deployments](https://github.com/safe-global/safe-deployments), y los contratos
entran en el
[bug bounty de la Safe Foundation](https://docs.safefoundation.org/security/bug-bounty)
(hasta 1,000,000 USD por hallazgos críticos).

Algo que una auditoría no cubre: el incidente de Bybit de 2025. Ese ataque
comprometió la cadena de compilación del frontend web oficial de Safe, no los
contratos — la
[conclusión forense oficial](https://safefoundation.org/blog/safe-ecosystem-foundation-statement)
no encontró vulnerabilidad en los contratos de Safe. Nosotros lo leemos como una
lección sobre la capa web y de operaciones, que es justo la capa en la que también
deberías escrutarnos a nosotros.

### Safe4337Module v0.3.0 — el adaptador ERC-4337

Desplegado en `0x75cf11467937ce3F2f357CE24ffc3DBF8fD5c226`, la dirección canónica de
v0.3.0 (coincidencia exacta en Sourcify: el bytecode on-chain es el código
auditado).
[Auditado por Ackee Blockchain](https://github.com/safe-global/safe-modules/blob/main/modules/4337/docs/v0.3.0/audit.md)
(reporte final de marzo de 2024), sin hallazgos sin resolver por encima del nivel
informativo. La combinación v0.3.0 + EntryPoint v0.7 + Safe ≥ 1.4.1 que usamos es
exactamente la configuración que describen la auditoría y las notas de versión.

La historia del módulo incluye un problema divulgado: v0.1.0 (2023) no firmaba
`initCode` ni `paymasterAndData`, un vector de griefing de gas. Se
[corrigió en v0.2.0](https://safefoundation.org/blog/strengthening-security-addressing-the-incident-of-the-canonical-4337-module),
y v0.1.0 nunca salió de las testnets. Nosotros usamos v0.3.0, que hereda el arreglo.

### SafeWebAuthnSharedSigner v0.2.1 — el firmante de passkeys

Desplegado en `0x94a4F6affBd8975951142c3999aEAB7ecee555c2`, la dirección canónica de
v0.2.1 (la misma en cada cadena vía la fábrica de singletons de Safe).

Qué significa «shared» y qué no: lo compartido es el _despliegue del contrato_, del
mismo modo que se comparte el singleton de Safe. Tu llave no. Cada Safe llama a
`configure()` por delegatecall y guarda su propia llave pública P-256 en su propio
almacenamiento. Una instancia del firmante representa exactamente una passkey por
Safe, y el Safe de nadie más puede usar la tuya.

Aquí la versión importa. La auditoría de v0.2.0
[decía explícitamente](https://github.com/safe-global/safe-modules/blob/main/modules/passkey/docs/v0.2.0/audit.md)
que el firmante compartido estaba fuera de alcance: el contrato todavía no existía.
Las auditorías que cubren lo que desplegamos son las de v0.2.1: un
[concurso de auditoría de Hats Finance](https://github.com/safe-global/safe-modules/blob/main/modules/passkey/docs/v0.2.1/audit-competition-report-hats.md)
(junio–julio de 2024: cero altos, cero medios, tres hallazgos bajos, todos
corregidos) más una
[revisión de Certora del commit de lanzamiento](https://github.com/safe-global/safe-modules/blob/main/modules/passkey/docs/v0.2.1/audit.md)
sin hallazgos nuevos. Desde el lanzamiento no se ha divulgado ninguna vulnerabilidad
a nivel de contrato; los contratos de passkeys entran en el bounty de la Safe
Foundation.

La propia documentación de Safe recomienda acompañar la propiedad por passkey con
una ruta de recuperación, en vez de tratar una sola credencial como la única llave
de la cuenta. Cómo lo maneja Vela está documentado en
[recuperación e inicio de sesión](/es-MX/docs/recovery).

La verificación P-256 on-chain usa directamente el precompilado RIP-7212, sin
verificador de respaldo en Solidity. Antes de habilitar cualquier red, la app prueba
el precompilado con una firma real y rechaza la red si la verificación falla. Dos
salvedades honestas: la especificación original de RIP-7212 tiene fallas en casos
borde que el [EIP-7951](https://eips.ethereum.org/EIPS/eip-7951) vino a corregir (no
afectan firmas WebAuthn bien formadas), y una prueba no puede atrapar todas las
formas en que la implementación de una cadena podría divergir en contextos de
ejecución inusuales.

### EntryPoint v0.7 — el punto de entrada ERC-4337

Desplegado en `0x0000000071727De22E5E9d8BAf0edAc6f37da032`, el
[despliegue canónico de v0.7.0](https://github.com/eth-infinitism/account-abstraction/releases/tag/v0.7.0).
[Auditado por OpenZeppelin](https://www.openzeppelin.com/news/erc-4337-account-abstraction-incremental-audit)
(por encargo de la Ethereum Foundation, enero de 2024): cero críticos, cero altos,
cinco hallazgos medios, todos resueltos — y el commit auditado es la versión
desplegada. EntryPoint v0.7.0 entra en el
[bug bounty de ERC-4337](https://docs.erc4337.io/community/bug-bounty) de la
Ethereum Foundation (hasta 250,000 USD).

## Problemas conocidos que estamos vigilando

### El vector de griefing del EntryPoint

En febrero de 2026, investigadores de seguridad de Trust Security
[divulgaron](https://erc4337.substack.com/p/improving-useroperation-execution)
un vector de griefing y censura que afecta a todo EntryPoint anterior a v0.9,
incluido el v0.7 que usamos. Un atacante que intercepte una UserOperation firmada
antes de que se mine puede ejecutarla dentro de un marco de llamada que él controla
y forzar que la ejecución interna revierta: la operación falla, pero el gas se cobra
igual. La Ethereum Foundation pagó a los investigadores un bounty de 50,000 USD;
clasificó el problema como vector de censura/griefing, no de robo de fondos, y nunca
se ha explotado.

Lo que puede hacer: desperdiciar una comisión y retrasar una transacción. Lo que no
puede: robar fondos ni falsificar una firma. La exposición de Vela es angosta porque
las UserOperations van directo a un relay y no por un mempool público —así que hay
poca oportunidad de interceptar una— y el peor caso está acotado por la comisión que
ya aceptaste. El arreglo solo existe en EntryPoint v0.9 (noviembre de 2025); v0.7 en
sí no se puede parchar. Esperamos migrar conforme el stack alrededor —en particular
la línea del módulo 4337 de Safe— soporte v0.9, y lo anotaremos aquí cuando pase.

## Qué no está auditado

- **Los contratos propios de Vela.** Dos contratos pequeños que escribimos nosotros,
  desplegados en Gnosis: el
  [índice de llaves públicas de passkeys](https://github.com/atshelchin/webauthnp256-publickey-index.biubiu.tools)
  (un registro de solo anexado que ayuda a tus dispositivos a encontrar tu llave
  pública) y su ayudante por lotes. No están auditados. Por construcción no guardan
  fondos, no tienen dueño y no se pueden actualizar: son una capa de descubrimiento,
  no de autorización. El poder de gasto siempre viene de la passkey configurada
  dentro de tu Safe. La peor falla realista es griefing (que alguien ocupe una
  entrada del índice), lo que puede hacer la recuperación menos cómoda pero no puede
  mover dinero. Un contrato divisor para la liquidación de gas, de un diseño de
  comisiones anterior, ya no es parte del flujo de transacciones.
- **Multicall3.** Su propio README lo
  [dice claro](https://github.com/mds1/multicall3): «This contract is unaudited.» Lo
  usamos exactamente como sus autores describen que es seguro: llamadas de solo
  lectura en lote para saldos, metadatos de tokens y precios. Vela nunca le otorga
  aprobaciones y nunca guarda fondos ahí. El peor caso de un bug es una lectura
  incorrecta.
- **El desplegador CREATE2.** El
  [proxy de despliegue determinista de Arachnid](https://github.com/Arachnid/deterministic-deployment-proxy)
  es el desplegador sin estado estándar del ecosistema; no tiene auditoría formal.
  Nuestras verificaciones de red fallan del lado seguro si falta o está alterado en
  una cadena.
- **Tempo y pathUSD.** Tempo, una de nuestras doce redes integradas, no tiene moneda
  nativa; ahí el gas se liquida en la stablecoin pathUSD. A agosto de 2026, ni el
  protocolo base de Tempo ni pathUSD tienen auditoría de seguridad publicada ni bug
  bounty, y una
  [evaluación independiente de colateral de DefiLlama](https://artifacts.llama.fi/md-exports/pathusd-collateral-assessment-april2026-1776332825042.md)
  (abril de 2026) calificó a pathUSD como de alto riesgo. Es riesgo a nivel de
  cadena que ninguna wallet puede mitigar: los fondos que tengas en Tempo, y la
  liquidación de gas ahí, lo heredan. Trata a Tempo como la cadena más nueva y menos
  probada de la lista y dimensiona tus saldos en consecuencia. Actualizaremos esta
  sección conforme se publiquen auditorías.
- **Vela misma.** Nuestra app y servicios de backend no han tenido auditoría de
  terceros. Esa es la mayor salvedad de esta página, lo decimos en el encabezado del
  sitio, y los detalles honestos están en
  [Vela está en alfa](/blog/vela-is-in-alpha). Empieza con montos chicos. Lee el
  código.

## Compruébalo tú

Cada dirección de arriba es un despliegue público y canónico que puedes contrastar
con los registros oficiales:
[safe-deployments](https://github.com/safe-global/safe-deployments),
[safe-modules-deployments](https://github.com/safe-global/safe-modules-deployments)
y las
[notas de versión del EntryPoint](https://github.com/eth-infinitism/account-abstraction/releases/tag/v0.7.0):

| Contrato | Dirección |
| ----------------------------------- | -------------------------------------------- |
| SafeL2 singleton v1.4.1 | `0x29fcB43b46531BcA003ddC8FCB67FFE91900C762` |
| SafeProxyFactory v1.4.1 | `0x4e1DCf7AD4e460CfD30791CCC4F9c8a4f820ec67` |
| CompatibilityFallbackHandler v1.4.1 | `0xfd0732Dc9E303f09fCEf3a7388Ad10A83459Ec99` |
| MultiSend v1.4.1 | `0x38869bf66a61cF6bDB996A6aE40D5853Fd43B526` |
| SafeModuleSetup v0.3.0 | `0x2dd68b007B46fBe91B9A7c3EDa5A7a1063cB5b47` |
| Safe4337Module v0.3.0 | `0x75cf11467937ce3F2f357CE24ffc3DBF8fD5c226` |
| SafeWebAuthnSharedSigner v0.2.1 | `0x94a4F6affBd8975951142c3999aEAB7ecee555c2` |
| EntryPoint v0.7 | `0x0000000071727De22E5E9d8BAf0edAc6f37da032` |
| Multicall3 | `0xcA11bde05977b3631167028862bE2a173976CA11` |
| Índice de llaves públicas de passkeys (Gnosis) | `0xdd93420BD49baaBdFF4A363DdD300622Ae87E9c3` |
