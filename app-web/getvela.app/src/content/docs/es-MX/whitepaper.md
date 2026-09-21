---
title: Whitepaper
description: "Cómo funciona Vela y en qué tienes (y en qué no tienes) que confiar para usarla: la cuenta, las llaves, la comisión, el modelo de amenazas, la recuperación y qué pasa si Vela desaparece."
source: d072d6710855
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Whitepaper

<Callout type="info" title="Estado: alfa · última revisión en septiembre de 2026">
Esta página describe cómo funciona Vela hoy y en qué tienes y en qué no tienes que
confiar para usarla. Vela está en <a href="/blog/vela-is-in-alpha">alfa</a>: empieza
con montos pequeños. Vela no tiene token. Todo lo que dice esta página se puede
contrastar con el código abierto; donde el código y esta página no coincidan, el
código tiene la razón y esta página tiene un bug.
</Callout>

## Resumen

Vela es una **wallet de contrato inteligente de autocustodia** para Ethereum y otras
redes EVM. Cada wallet es una cuenta **Safe v1.4.1** sin modificar, operada a través
de **ERC-4337** y controlada por hasta siete **passkeys**: llaves WebAuthn P-256 que
guardan tus dispositivos, tu gestor de contraseñas o llaves de seguridad físicas. No
hay frase semilla.

Vela, la empresa, nunca tiene tus llaves ni ningún rol en tu Safe, así que **no
puede mover, congelar ni confiscar tus fondos** por su cuenta. Lo que sí hace es
escribir y servir el software que les pide a tus llaves que firmen, y por eso importa
el modelo de amenazas de más abajo. Las apps, el relay que envía las transacciones y
los servicios de apoyo son de código abierto, y puedes operar tu propia copia de cada
uno; hoy el relay todavía lee los datos de cadena del servidor de Vela, a menos que
cambies su código. En qué confías, en pocas palabras: en los contratos, en los
autenticadores que guardan tus llaves, en el código de la app con la que firmas, en
el dominio al que pertenecen tus passkeys y en los servicios a los que apuntas la
app.

## Por qué existe Vela

- Las **wallets con frase semilla** ponen un secreto de 12 a 24 palabras frente a
  cada usuario: un punto único de falla y un blanco permanente de phishing.
- Las **wallets custodiales** eliminan la frase semilla quedándose con la custodia de
  los fondos.
- Las **wallets con passkey** que dependen de los servidores y del código cerrado de
  una sola empresa eliminan la frase semilla, pero te dejan varado si la empresa
  desaparece.
- La **firma a ciegas** (aprobar datos opacos que no puedes leer) sigue siendo común,
  y es una de las formas en que vacían wallets.

Vela busca la comodidad de una passkey sin ninguna de esas dependencias: una cuenta
estándar, código abierto, servicios reemplazables y transacciones que puedes leer
antes de firmar.

## Principios de diseño

1. **Autocustodia, sin excepciones.** Las llaves las crean y las guardan tus
   autenticadores. Los servicios de Vela nunca las ven; lo que sí ven está en la
   sección de Privacidad.
2. **Contratos estándar, sin modificar.** Ningún contrato en el camino hacia tus
   fondos lo escribió Vela.
3. **Verifica, no confíes.** Las apps y los servicios son públicos; los servicios se
   pueden autoalojar.
4. **Decodificar antes de firmar.** Lo que no se puede decodificar lleva una
   advertencia explícita de firma a ciegas.
5. **Hacer menos.** La wallet envía, recibe y firma para las dApps que tú eliges.

## Arquitectura

```text
Apps de Vela — web, extensión de navegador, escritorio (macOS/Windows/Linux), iOS, Android
  un solo núcleo compartido en Rust (reglas, criptografía, ABI, firma legible) + una capa nativa en cada una
  • arman la UserOperation y te muestran lo que hace
  • le piden a tu llave una aserción WebAuthn
        │  UserOperation firmada (comisión incluida)
        ▼
Relay (vela-relay, autoalojable)
  • cotiza la comisión, paga el gas por adelantado, envía handleOps
  • no puede cambiar la operación
        ▼
Cadena EVM
  EntryPoint v0.7 → tu Safe v1.4.1 → módulo 4337 de Safe
  El módulo de passkey de Safe verifica P-256 con el precompilado RIP-7212
```

Servicios de apoyo, todos de código abierto: un **índice de llaves públicas** que
registra las wallets nuevas en un registro on-chain y responde búsquedas, un
directorio de **datos de cadena** y una fuente de **tipos de cambio**. Consulta la
[guía de autoalojamiento](/es-MX/docs/self-hosting).

### La cuenta

Tu wallet es un proxy de **Safe v1.4.1** (singleton SafeL2), con el **módulo 4337
v0.3.0** de Safe habilitado como su módulo y su fallback handler, operado a través del
**EntryPoint v0.7**. Sus dueños son firmantes de passkey del **módulo de passkey
v0.2.1** de Safe: la primera llave la verifica el firmante compartido, y cada llave
adicional, su propio contrato firmante creado por la fábrica de Safe. El umbral es
**1**.

La dirección es **determinista y contrafactual**: se calcula con `CREATE2` a partir
de los datos de configuración del Safe, que incluyen cada llave fundadora, antes de
desplegar nada. Es la misma en todas las redes. Puedes recibir en ella de inmediato;
tu primera transacción en cada red despliega la wallet y paga el despliegue dentro de
la comisión de esa transacción.

### Las llaves

Una wallet tiene **de una a siete llaves**, fijas desde que la creas. Cualquiera de
ellas puede firmar sola (1-of-n). Una llave puede ser:

- una passkey en el dispositivo que estás usando, sincronizada por el Llavero de
  iCloud, el Administrador de contraseñas de Google u otro gestor de contraseñas si
  lo permites;
- otro celular, al que llegas escaneando un código QR (el transporte híbrido de
  WebAuthn);
- una llave de seguridad física por USB o NFC, que no se sincroniza en ningún lado.

Cada firma necesita la verificación de usuario del propio autenticador: un dato
biométrico o el PIN del dispositivo, o el PIN y un toque en una llave de seguridad.
No hay llave de sesión. Las llaves no se pueden agregar, quitar ni reemplazar
después: en cada cadena donde la wallet todavía no está desplegada, la dirección
sigue representando al conjunto fundador, así que cambiar los dueños en una cadena
haría que la cuenta fuera distinta de una cadena a otra.

Las passkeys pertenecen a una relying party; las de Vela se crean para
**`getvela.app`**. Los navegadores solo las ofrecen a páginas de getvela.app o de sus
subdominios, que es lo que las hace resistentes al phishing; también es una
dependencia a la que este documento vuelve más abajo.

### Flujo de firma

1. **Armar** una UserOperation para tu Safe (que incluye una transferencia que le
   paga al relay) y simularla.
2. **Decodificarla** en una intención legible y mostrártela.
3. **Firmar**: tu autenticador produce una aserción WebAuthn sobre el hash de la
   operación después de verificar que eres tú.
4. **Codificar** la aserción como la firma de Safe que espera el módulo de passkey.
5. **Enviar** la operación firmada al relay, que llama al EntryPoint.
6. **Verificar on-chain**: el módulo de passkey comprueba la firma P-256 con el
   precompilado RIP-7212 antes de que el Safe ejecute nada. No hay verificador de
   respaldo; una red sin el precompilado no se puede agregar.

### Comisiones

- Al relay se le paga **dentro de la operación**: la operación declara comisiones de
  EntryPoint en cero e incluye una transferencia de tu Safe a la dirección del relay.
  El monto y el destinatario forman parte de lo que firmas, así que pagas exactamente
  lo que mostró la pantalla de confirmación.
- La comisión es **el triple del gas que la wallet reserva para la operación** (las
  estimaciones simuladas, aumentadas un 50%, con mínimos), **al precio más alto
  entre la lectura que hace la propia wallet del precio del gas y el precio del relay
  para la velocidad elegida**, con un mínimo de alrededor de US$0.01. En Tempo el
  múltiplo es dos. Por ese margen en la reserva y en el precio, la comisión suele ser
  diez veces o más el costo on-chain real de la operación, y más en la primera
  transacción en una red; el relay se queda con la diferencia.
- La comisión se paga en la moneda de la red o en una stablecoin en dólares que el
  relay acepte (pathUSD en Tempo, que no tiene moneda nativa). **No hay paymaster**:
  nadie patrocina el gas, y nadie puede filtrar transacciones con una política de
  patrocinio.
- Si la tesorería de gas de un relay en una red está vacía, la wallet te lo dice
  antes de que firmes. No hay depósito por usuario.

Detalles: [redes y comisiones](/es-MX/docs/networks-and-fees).

### Firma legible

Las llamadas y los mensajes EIP-712 se decodifican con descriptores **ERC-7730**
(integrados en la app para los contratos comunes, obtenidos del servicio de datos de
cadena o emparejados con formas estándar de tokens) y luego, como último recurso, con
una base de datos pública de selectores, con la etiqueta de mejor esfuerzo. Lo que
quede recibe una advertencia explícita de firma a ciegas. Los descriptores obtenidos
no están autenticados criptográficamente. Una aprobación on-chain de nivel
«ilimitado» (2^200 o más) no se puede enviar hasta que la reduzcas; una aprobación
finita grande y los permisos firmados se muestran con una advertencia, pero no se
bloquean. Detalles: [firma legible](/es-MX/docs/clear-signing).

### Redes

Vela trae 24 redes integradas (Ethereum, BNB Chain, Polygon, Arbitrum, Optimism,
Base, Avalanche, Gnosis, Unichain, Tempo, Monad, World Chain, Arc, X Layer, Stable,
Soneium, MegaETH, Robinhood Chain, Mantle, Kaia, Celo, Ink, Plume y XRPL EVM) y
acepta cualquier red EVM que tenga los once contratos que revisa y el precompilado
RIP-7212. (Las llaves de la dos a la siete también necesitan en esa red la fábrica de
firmantes de passkey de Safe, que la revisión todavía no cubre.)

## Modelo de seguridad

**Lo que Vela no puede hacer**

- Mover, gastar ni congelar tus fondos por su cuenta: solo tus llaves autorizan tu
  Safe, y Vela no tiene ningún rol en él. (Lo que Vela sí puede hacer es publicar
  software que te pida firmar; consulta las amenazas de más abajo.)
- Cambiar una transacción después de que la firmas: cualquier cambio invalida la
  firma.
- Leer tus llaves privadas: se quedan en tus autenticadores.
- Agregar una llave a tu wallet, o quitar una.

**Lo que «no puede congelar» no cubre: el token.** USDC, USDT y la mayoría de los
tokens respaldados por dinero fiat le permiten a su emisor meter cualquier dirección
en una lista negra, incluida la tuya. Ese poder es del emisor y existe sin importar
qué wallet uses. Lo que te da la autocustodia es que Vela no sea una segunda parte
que pueda hacerlo.

**En qué confías**

- En los **contratos**: Safe, sus módulos 4337 y de passkey, el EntryPoint v0.7 y el
  precompilado RIP-7212 de la cadena.
- En el **dominio**: cualquier página servida desde getvela.app o uno de sus
  subdominios puede pedirles a tus llaves una firma.
- En los **autenticadores** que guardan tus llaves y, en el caso de las passkeys
  sincronizadas, en la cuenta de Apple, de Google o del gestor de contraseñas que
  está detrás.
- En **el código de la app con la que firmas.** Arma la transacción y te muestra lo
  que hace. Una app comprometida puede mostrarte una cosa y pedirte que firmes otra;
  el aviso del autenticador no te va a decir la diferencia.
- En los **endpoints RPC** de los que lees: un nodo que miente puede mostrar saldos
  equivocados o una vista previa de simulación equivocada. Puedes configurar los
  tuyos.
- En los **servicios de datos de cadena y de tipos de cambio**: proporcionan las
  listas de tokens, los descriptores, la lista de tokens para pagar comisiones y los
  tipos de cambio que se usan para convertir un monto en moneda fiat en un monto en
  tokens.
- En el **relay**: no puede cambiar lo que firmaste, pero puede retrasarlo o
  rechazarlo, elegir cuándo entra a la cadena (así que podría adelantarse a un swap
  dentro de tu tolerancia al deslizamiento) y fijar el precio del gas en el que se basa
  tu comisión, hasta el triple de la lectura de la propia wallet.

**Amenazas consideradas**

- **Dispositivo perdido o robado**: un ladrón todavía tiene que pasar la
  verificación del autenticador; otra llave te devuelve el acceso. Pero una llave no
  se puede quitar: si alguna puede estar en manos de alguien más, pasa tus fondos a
  una wallet nueva, porque esa llave puede seguir gastando desde la dirección vieja
  en todas las redes.
- **Phishing**: una passkey no se puede escribir en un sitio falso, y los navegadores
  solo la ofrecen a páginas de getvela.app y sus subdominios.
- **dApp maliciosa**: la cubren la firma legible y la protección de aprobaciones, con
  un hueco serio: una dApp puede pedir una llamada de tu Safe a sí mismo
  (`enableModule`, `addOwnerWithThreshold`, `setFallbackHandler`, `setGuard`), y
  cualquiera de ellas, firmada una sola vez, entrega la cuenta tan completamente como
  lo hizo la carga de Bybit. Vela decodifica esas llamadas pero todavía no las
  bloquea. Rechaza cualquier solicitud cuyo destino sea la dirección de tu propia
  wallet.
- **Servicio de backend comprometido** (relay, índice, datos de cadena, tipos de
  cambio): sin poder de firma, pero con influencia real: negar el servicio,
  descriptores o listas de tokens engañosos, tipos de cambio equivocados que cambian
  cuánto se envía por un monto en moneda fiat y (en el caso del relay) el momento y el
  precio del gas mencionados arriba. Los descriptores obtenidos no se tratan como
  autenticados, y cada servicio se puede reemplazar.
- **Entrega de la app comprometida**: un despliegue web, una actualización de la
  extensión o una compilación de la app alterados podrían presentarte una transacción
  maliciosa para firmar. Es la clase de ataque de [Bybit](/es-MX/docs/bybit-attack).
  Las mitigaciones de hoy son limitadas: la decodificación y la protección de
  aprobaciones de la propia app, las versiones de macOS notarizadas y compilar tú
  mismo la extensión o las apps desde el código fuente (los paquetes publicados traen
  sumas de verificación SHA-256, no firmas). Hay una página de firma independiente que
  no comparte el código de la app, ya construida pero todavía no conectada.
- **Cualquier cosa servida desde el dominio**: cualquier página de getvela.app o de
  sus subdominios, incluido un script que cargue, podría pedir firmas a las passkeys
  de Vela, y el aviso solo muestra «getvela.app». Por eso el sitio web prohíbe que sus
  propias páginas usen passkeys, y mantiene su script de analítica fuera de la página
  que guarda una llave. Si el dominio cambiara de manos, su nuevo dueño también
  controlaría qué apps pueden usar las passkeys. La extensión y las apps compiladas
  por ti llevan su propio código, aunque por defecto siguen obteniendo descriptores y
  usando servicios bajo getvela.app.

## Recuperación

Crear una wallet publica sus llaves públicas y su dirección en un **contrato de
registro** público en Gnosis (que se puede copiar a Ethereum). En un dispositivo
nuevo inicias sesión con **cualquiera** de tus llaves; la app encuentra la wallet a
través del índice o, si eso falla, directamente desde el registro, y comprueba que
las llaves den de nuevo la dirección registrada. Una wallet de una sola llave también
se puede reconstruir a partir de dos firmas, sin ningún registro.

<Callout type="warning" title="Tus llaves son tu recuperación">
No hay frase semilla, ni recuperación social, ni guardianes: nada que Vela pudiera
perder, filtrar o verse obligada a usar. Si se pierden todas las llaves fundadoras,
la wallet no se puede recuperar. Crea la wallet con más de una llave, mantén activada
la sincronización de passkeys si dependes de ella y protege la cuenta que está
detrás.
</Callout>

Detalles: [recuperación e inicio de sesión](/es-MX/docs/recovery).

## Si Vela desaparece

Tus fondos se quedan en tu Safe, on-chain. Los contratos no dependen de Vela, y cada
servicio que opera Vela es de código abierto para que alguien más lo opere; el relay
necesita un cambio en el código para dejar de leer datos de cadena del servidor de
Vela. Lo único que no se puede mover es la relying party de las passkeys,
`getvela.app`: una copia de la wallet web en otro dominio crea otra wallet. Para las
wallets existentes, la extensión de Vela para el navegador (que puede usar passkeys
de `getvela.app` por permiso) y las apps que compiles tú (con un celular o una llave
de seguridad) siguen funcionando sin getvela.app. La
[guía de autoalojamiento](/es-MX/docs/self-hosting#if-getvela-app-disappears)
detalla cada camino y sus límites. El acceso independiente a una cadena también
requiere que esa cadena admita RIP-7212.

## Privacidad

Sin cuenta, sin correo, sin KYC. Lo que se vuelve público se escribe en el registro
cuando creas una wallet: la llave pública y el ID de credencial de cada llave, el
modelo de autenticador, el nombre de tu wallet y los nombres de tus llaves, la
dirección y los datos de registro firmados. El índice de Vela ve ese registro antes
de enviarlo, y las direcciones cuyo nombre buscas; el relay de Vela ve tu dirección,
las operaciones que envías y el endpoint RPC que usa tu app (incluida cualquier llave
de API que traiga su URL), y guarda las operaciones por un tiempo limitado para
reintentar y diagnosticar. Todos los servicios ven tu dirección IP. El sitio web usa
analítica sin cookies. La [política de privacidad](/privacy) es la lista oficial.

## Código abierto

La wallet (todas las apps y el núcleo), el relay y el servicio de tipos de cambio
tienen licencia MIT; el directorio de datos de cadena también es MIT. El índice de
llaves públicas es público, pero todavía no tiene archivo de licencia. Código:
[github.com/mondaylabsltd](https://github.com/orgs/mondaylabsltd/repositories).

## Sin token

Vela no tiene token ni planea tener uno. No hay nada que comprar ni que farmear, ni nada con
qué especular. Las comisiones se pagan en la moneda de cada red o en una stablecoin.

## Estado de las auditorías y limitaciones

Los contratos de Safe, sus módulos 4337 y de passkey, y el EntryPoint v0.7 están
auditados de forma independiente y son muy usados. **El código propio de Vela (las
apps, los servicios de backend y el contrato de registro) no ha tenido una auditoría
independiente de terceros, y no hay ninguna programada**; una auditoría profesional
es una meta para cuando el proyecto pueda pagarla, no un compromiso con fecha.
Mientras tanto, la revisión es informal: el código es abierto, miembros capaces de la
comunidad lo leen y se revisa con herramientas de IA. Eso ayuda; no equivale a una
auditoría profesional. Trata a Vela como software en alfa. Detalles:
[auditorías y problemas conocidos](/es-MX/docs/security-audits).

## Referencias

- ERC-4337: abstracción de cuentas a través del EntryPoint
- EIP-1271: validación de firmas para contratos
- ERC-7730: descriptores de firma legible
- EIP-5792: agrupación de llamadas de la wallet (`wallet_sendCalls`)
- RIP-7212 / EIP-7951: precompilado de verificación de firmas P-256
- WebAuthn / FIDO2: passkeys
- [Safe smart account v1.4.1](https://github.com/safe-fndn/safe-smart-account/tree/v1.4.1)
