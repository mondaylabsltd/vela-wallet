---
title: Whitepaper
description: Cómo funciona Vela y qué tienes —y qué no— que dar por bueno para usarla. Arquitectura, modelo de seguridad, recuperación y cómo verificarlo todo por tu cuenta.
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Whitepaper

<Callout type="info" title="Estado: alfa · v0.1">
Esta página describe cómo funciona Vela hoy y qué tienes o no tienes que dar por
bueno para usarla. Prefiere la honestidad al marketing. Vela está en
<a href="/blog/vela-is-in-alpha">alfa</a>: empieza con montos chicos. Vela no tiene
token. Todo lo de aquí es verificable contra el código abierto.
</Callout>

## Resumen

Vela es una **wallet de contrato inteligente en autocustodia** para redes EVM. Cada
wallet es una cuenta inteligente
[Safe](https://github.com/safe-fndn/safe-smart-account) controlada por una
**passkey**: una credencial WebAuthn (P-256) guardada por el sistema operativo de
tu dispositivo, cifrada de extremo a extremo y desbloqueada con Face ID, Touch ID o
huella. No hay frases semilla ni llaves privadas que copiar, guardar o perder.

Vela, la empresa, nunca tiene tus llaves ni tus fondos y **no puede moverlos,
congelarlos ni incautarlos**. La app, el relay de transacciones y los servicios de
apoyo son de código abierto y autohospedables. Lo que tienes que dar por bueno se
reduce a contratos auditados, a la bóveda de passkeys de tu sistema operativo y —
solo para disponibilidad— a un relay que puedes reemplazar o correr tú.

## Por qué existe Vela

La mayoría de las wallets te obliga a un trade-off:

- **Las wallets con frase semilla** ponen un secreto de 12 a 24 palabras frente a
  cada usuario. Es el punto único de falla y un blanco constante de phishing.
- **Las wallets custodiales** quitan la frase semilla pero se quedan con la custodia
  de tus fondos, reintroduciendo el riesgo de contraparte que las cripto venían a
  eliminar.
- **La firma a ciegas** —aprobar hexadecimal que no puedes leer— se normalizó en
  todo el ecosistema y está detrás de buena parte de las wallets vaciadas.

Vela busca ser tan fácil como una app custodial y dejarte en autocustodia completa:
sin frase semilla, sin custodia ajena y sin transacciones que no puedas leer antes
de firmarlas.

## Principios de diseño

1. **Autocustodia, sin excepciones.** Las llaves se generan en tu dispositivo y las
   guarda el servicio de passkeys de tu sistema, cifradas de extremo a extremo. Los
   servidores de Vela solo ven datos públicos.
2. **Verifica, no confíes.** Todo el stack —la app y los cuatro servicios de
   backend— es de código abierto con licencia MIT.
3. **Nada de firma a ciegas.** Las transacciones se traducen a intención legible
   donde exista un descriptor; las llamadas desconocidas se marcan, no se esconden.
4. **Hacer menos.** La wallet guarda ETH y ERC-20 y se conecta a las dApps que tú
   elijas. Menos código en el que confiar, menor superficie de ataque.

## Arquitectura

```text
App Vela (iOS / Android / Web, una sola base de código)
  • Passkey (WebAuthn P-256, servicio de passkeys del sistema)
  • Construcción y firma de la UserOperation
  • Interfaz de firma legible (ERC-7730)
        │  UserOperation firmada
        ▼
Relay Vela (ERC-4337, autohospedable)
  • envía handleOps al EntryPoint
  • no puede alterar ni falsificar tu transacción
        ▼
Cadena EVM
  EntryPoint v0.7 → cuenta inteligente Safe
  El firmante WebAuthn verifica P-256 on-chain
```

### Modelo de cuenta

Tu wallet es una cuenta inteligente **Safe v1.4.1** (un contrato proxy) operada con
abstracción de cuentas **ERC-4337** (EntryPoint v0.7), con el **Safe 4337 Module** y
un **firmante WebAuthn** como dueño de la cuenta.

La dirección es **determinista** y **contrafactual**: se calcula desde la llave
pública de tu passkey con `CREATE2` antes de que se envíe ninguna transacción, así
que puedes recibir fondos antes de que se despliegue. La cuenta se despliega sola,
pagando de su propio saldo, en tu primera transacción.

### Llaves y autenticación

La autenticación usa **passkeys WebAuthn** en la curva **P-256**. La llave privada
se genera en tu dispositivo y la guarda, cifrada de extremo a extremo, el servicio
de passkeys de tu sistema (Llavero de iCloud o Gestor de contraseñas de Google), que
la sincroniza entre tus dispositivos. **Los servidores de Vela solo ven tu llave
pública.** Firmar exige una verificación biométrica nueva cada vez: no hay llave de
sesión de larga vida. Todo el detalle en
[cómo funcionan las passkeys](/es-MX/docs/passkeys).

### Firma y flujo de la transacción

1. **Construir** una `UserOperation` ERC-4337 para tu Safe y estimar el gas.
2. **Decodificar** la llamada a intención legible y mostrarla para revisión.
3. **Firmar**: tras la verificación biométrica, tu dispositivo produce una aserción
   WebAuthn sobre el hash de la operación.
4. **Codificar** la aserción como firma de contrato **EIP-1271**.
5. **Retransmitir** la operación firmada al relay, que la envía al EntryPoint.
6. **Verificar on-chain**: el Safe verifica la firma P-256 on-chain con el
   precompilado RIP-7212 antes de ejecutar. El precompilado es requisito duro: no
   hay verificador de respaldo, y Vela se niega a habilitar una red que no lo tenga.

El relay recibe una operación **ya firmada**. No puede cambiar destinatario, monto
ni ningún otro campo sin invalidar la firma.

### Relay y modelo de gas

- El gas se paga **del saldo de tu propia wallet**, en el token nativo de la red por
  defecto, o en una stablecoin soportada donde el relay ofrezca una. Tempo, que no
  tiene moneda nativa, siempre liquida gas en stablecoins en dólares. **No hay
  paymaster** ni un tercero que patrocine —o condicione— tus transacciones.
- **El relay es la única fuente de verdad para el precio del gas.** Lo cotiza desde
  las condiciones reales de la cadena; la wallet muestra esa cotización y firma
  exactamente lo que muestra.
- El cobro del relay de Vela es deliberadamente simple: el total es el **costo de
  red más la comisión de servicio del relay**, con un mínimo pequeño en
  transacciones muy baratas. Una parte va a los validadores de la cadena; el resto
  paga al relay que opera la infraestructura y mantiene fondeada tu cuenta de gas.
- La wallet **muestra la comisión estimada antes de que confirmes** —en el activo de
  comisiones y en tu moneda de visualización— y el monto cotizado junto con su
  destinatario son parte de lo que firmas, así que al relay se le paga exactamente
  lo mostrado. Sin sobreprecio escondido.
- Cada Safe tiene una **cuenta de relay dedicada** (cuenta de gas) por cadena,
  activada con un depósito **no reembolsable**. Puede agotarse con el tiempo, así
  que quizá necesite **reactivarse** más adelante: no es estrictamente un depósito
  de una sola vez.

El relay es una dependencia de **disponibilidad**, no de **custodia**: puede
demorar o negarse, pero nunca alterar, falsificar ni robar. Es de código abierto y
puedes correr el tuyo — y como el precio se **cotiza y se muestra** en vez de
esconderse, incluso la comisión de un relay propio o de terceros siempre te queda a
la vista antes de firmar. Ve
[redes y comisiones](/es-MX/docs/networks-and-fees).

### Firma legible (ERC-7730)

Vela decodifica calldata y datos tipados EIP-712 con descriptores **ERC-7730** y
presenta la **intención** (Intercambiar, Enviar, Aprobar…), lo **sustancial**
(montos, direcciones) y, a petición, los **detalles** (nonce, fecha límite, calldata
en crudo), con colores por riesgo. Cuando ningún descriptor coincide, Vela muestra
una advertencia explícita de firma a ciegas en vez de fingir que entiende la
llamada.

### Redes

Vela soporta 12 redes EVM —Ethereum, BNB Chain, Polygon, Arbitrum, Optimism, Base,
Avalanche, Gnosis, Unichain, Tempo, Monad y World Chain— más redes personalizadas.
Una red personalizada solo se puede agregar si ya aloja los contratos de los que
Vela depende (el EntryPoint, los contratos Safe, el firmante WebAuthn) y el
precompilado P-256 RIP-7212; Vela lo revisa antes de habilitarla.

## Modelo de seguridad

**Lo que Vela no puede hacer:**

- Mover, gastar o transferir tus fondos: solo tu passkey puede autorizar al Safe.
- Congelar o incautar tu cuenta: el Safe es tu contrato on-chain; Vela no tiene
  ningún rol privilegiado sobre él.
- Firmar por ti: cada transacción necesita una aserción biométrica nueva.
- Ver tu llave privada: nunca llega a Vela; solo tu dispositivo puede usarla para
  firmar.
- Alterar una transacción después de que la firmas: cualquier cambio invalida la
  firma.

**Lo que «no puede congelar» no cubre: el *token*.** Una stablecoin con permisos
—USDC, USDT y la mayoría de los tokens respaldados por moneda fiduciaria— trae una
función de lista negra que su emisor puede invocar contra cualquier dirección,
incluida la tuya. Ese poder es del emisor y existe tengas el token en la wallet que
lo tengas; ninguna wallet de autocustodia, Vela incluida, puede quitarlo. Lo que la
autocustodia te da es que **nosotros** no somos una segunda parte que pueda.

**En qué sí confías:**

- En los **contratos Safe** (auditados, muy usados) y en el firmante WebAuthn que
  verifica tu llave P-256.
- En el **servicio de passkeys de tu sistema** (Apple / Google) para proteger y
  sincronizar tu credencial.
- En los **proveedores de RPC** que consultas (Vela usa un grupo multi-fuente con
  conmutación; puedes poner los tuyos).
- En el **relay**, solo para disponibilidad — y lo puedes autohospedar.

**Amenazas consideradas:**

- **Dispositivo perdido o robado**: quien lo tenga sigue necesitando tu biometría o
  tu PIN para firmar.
- **Phishing / dApp maliciosa**: se atiende con la firma legible.
- **Servidor de Vela comprometido**: no da capacidad de firma; el radio del daño es
  servicio degradado, no pérdida de fondos.
- **Riesgo de cadena de suministro**: se mitiga con código abierto y autohospedaje.

## Recuperación

Tu passkey la respalda el servicio de tu sistema operativo; en un dispositivo nuevo,
iniciar sesión con la misma cuenta de Apple o Google la restaura, y tu wallet
reaparece.

<Callout type="warning" title="El respaldo de passkeys de tu plataforma es tu recuperación">
La recuperación de Vela es tu passkey, sincronizada por el Llavero de iCloud o el
Gestor de contraseñas de Google. Por diseño no hay frase semilla, ni recuperación
social, ni guardianes: nada que Vela pueda perder, filtrar o verse obligada a usar.
El otro lado es real: si pierdes <strong>tanto</strong> tu dispositivo
<strong>como</strong> la passkey sincronizada en la nube, sin otra copia, la cuenta
no se puede recuperar. Deja activo el respaldo de passkeys de tu plataforma y
asegura esa cuenta.
</Callout>

El modelo completo de recuperación, con sus límites honestos, está en
[recuperación e inicio de sesión](/es-MX/docs/recovery).

## Si Vela desaparece

Autocustodia significa que tus llaves y tus fondos no dependen de que Vela siga en
línea. Los fondos viven en **tu contrato Safe on-chain**, y el relay es de código
abierto y reemplazable.

Una salvedad honesta: WebAuthn ata una passkey a un dominio de relying party
(`getvela.app`). Si ese dominio se perdiera de forma permanente, las passkeys atadas
a él necesitarían ayuda para funcionar en otro lado: una herramienta capaz de
presentarle al autenticador la relying party original. Vela distribuía antes una
extensión de navegador de nivel desarrollador para ese caso y la retiró en
septiembre de 2026; una ruta de recuperación para la pérdida del dominio apta para
usuarios comunes sigue siendo trabajo pendiente, y lo decimos en vez de insinuar que
existe. El acceso on-chain independiente también depende del soporte P-256
(RIP-7212) de la cadena destino, que va mejorando en varias cadenas.

## Privacidad

Sin cuentas, sin correo, sin KYC, sin frase semilla que recolectar. Los servidores
solo guardan tu **llave pública** y un nombre de cuenta que elegiste (para la
recuperación entre dispositivos), publicados on-chain por diseño. El contenido de
las transacciones no se registra. El sitio usa analítica autohospedada y sin
cookies. Ve el [aviso de privacidad](/privacy).

## Verificabilidad y código abierto

Todo es **de código abierto con licencia MIT**: la app y los cuatro servicios de
backend (datos de cadena, índice de passkeys, relay, tipos de cambio), que puedes
**autohospedar** (Ajustes → Avanzado → Endpoints de servicio). Lee el código en
[github.com/mondaylabsltd/vela-wallet](https://github.com/mondaylabsltd/vela-wallet).

## Sin token

Vela **no tiene token** ni planes de tenerlo. No hay nada que comprar, farmear ni
especular. El gas se paga en el activo nativo de cada red.

## Estado de auditorías y limitaciones

Los **contratos Safe** en el centro de cada cuenta Vela están auditados de forma
independiente y probados en batalla. La **integración propia de Vela** alrededor de
ellos **no ha pasado por una auditoría independiente de terceros**, y por ahora no
hay ninguna programada: una auditoría profesional es una meta para cuando el
proyecto pueda pagarla, no un compromiso con fecha. Hasta entonces la revisión es
informal: el código es abierto y depende de que miembros capaces e interesados de la
comunidad lo lean, más revisión asistida por IA. Ayuda, pero no equivale a una
auditoría profesional. Trata a Vela como software en alfa y usa montos que te
sientas cómodo poniendo en algo tan joven.

## Referencias

- ERC-4337 — abstracción de cuentas vía EntryPoint
- EIP-1271 — estándar de validación de firmas para contratos
- ERC-7730 — firma legible / descriptores de datos estructurados
- EIP-5792 — agrupación de llamadas de wallet
- RIP-7212 — precompilado para verificación de firmas secp256r1 (P-256)
- WebAuthn / FIDO2 — autenticación con passkeys
- [Cuenta inteligente Safe v1.4.1](https://github.com/safe-fndn/safe-smart-account/tree/release/v1.4.1)
