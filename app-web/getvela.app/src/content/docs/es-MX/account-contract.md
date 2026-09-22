---
title: El contrato de la cuenta
description: "Tu wallet de Vela es un Safe v1.4.1 sin modificar. Ningún contrato en el camino hacia tu dinero lo escribió Vela: aquí está exactamente qué contratos son, qué ganas con eso y qué cuesta."
source: 17fbc25a3149
---

# El contrato de la cuenta

Tu wallet no es una estructura de datos privada de una app. Es una cuenta
inteligente **Safe v1.4.1** (el contrato que usan muchas tesorerías on-chain
grandes), desplegada exactamente como la publica Safe, sin ninguna modificación.

## Nada en el camino es nuestro

Cada contrato que puede tocar tu dinero lo escribió Safe o los autores de ERC-4337:

| Contrato | Papel en tu wallet | Autor |
| --- | --- | --- |
| [Safe v1.4.1](https://github.com/safe-fndn/safe-smart-account/tree/v1.4.1) (SafeL2, a través de un proxy) | La cuenta en sí: dueños, umbral, ejecución | Safe |
| [Safe 4337 Module v0.3.0](https://github.com/safe-global/safe-modules/tree/4337/v0.3.0/modules/4337) | Permite que el EntryPoint opere el Safe; también es su fallback handler | Safe |
| [SafeWebAuthnSharedSigner v0.2.1](https://github.com/safe-global/safe-modules/tree/passkey/v0.2.1/modules/passkey) | Verifica las firmas P-256 de la primera llave | Safe |
| [SafeWebAuthnSignerFactory v0.2.1](https://github.com/safe-global/safe-modules/tree/passkey/v0.2.1/modules/passkey) y los firmantes que crea | Un contrato firmante pequeño por cada llave adicional | Safe |
| [ERC-4337 EntryPoint v0.7](https://github.com/eth-infinitism/account-abstraction/releases/tag/v0.7.0) | Ejecuta tu operación firmada | Autores de ERC-4337 |

Los contratos propios de Vela no están en esta lista: el **registro de llaves
públicas**, que anota las llaves de cada wallet para que un dispositivo nuevo pueda
encontrarla ([recuperación](/es-MX/docs/recovery)), el registro de dominios que lo
acompaña y el índice anterior al que reemplazaron. No guardan fondos ni tienen
ningún papel en tu Safe.

El repositorio de la wallet no contiene nada de Solidity; lo puedes comprobar con un
solo comando:

```bash
git clone https://github.com/mondaylabsltd/vela-wallet
find vela-wallet -name '*.sol'   # no imprime nada
```

Vela no tiene ningún rol privilegiado en tu cuenta: ni llave de administrador, ni
ruta de actualización, ni un módulo que pueda agregar. Solo tus llaves pueden cambiar
tu Safe.

## Por qué «sin modificar» es la palabra que importa

Muchas wallets están construidas sobre «un Safe», un fork de Safe o una cuenta
inspirada en Safe. La diferencia importa de tres maneras.

**Las auditorías aplican a lo que de verdad estás usando.** Las auditorías de Safe
cubren estas versiones, o versiones anteriores de las que solo difieren por cambios
pequeños y documentados (la [página de auditorías](/es-MX/docs/security-audits) tiene
los detalles). Las auditorías de un fork cubren el código antes del fork; la
modificación es la parte que nadie auditó.

**El ecosistema trata tu cuenta como un Safe, porque lo es.** Los exploradores de
bloques la decodifican, y las herramientas de Safe pueden leerla y armar
transacciones para ella. Pero para *firmar* esas transacciones, un programa tiene
que poder pedirle a tu llave una firma para `getvela.app`, el dominio al que
pertenecen tus passkeys; por eso la propia app web de Safe, servida desde otro
dominio, no puede firmar por ti. La
[guía de autoalojamiento](/es-MX/docs/self-hosting#if-getvela-app-disappears)
enumera lo que sí puede.

**La superficie de ataque es una que muchos otros también vigilan.** Un contrato de
cuenta hecho a la medida lo vigila sobre todo su autor. Los contratos centrales de
Safe los vigila todo el que tiene dinero en un Safe; los módulos 4337 y de passkey
tienen un público más pequeño, pero real.

## Lo que cuesta

Ser estándar no sale gratis:

- **Gas.** Tu firma se verifica on-chain y la transacción pasa por el EntryPoint. Un
  envío simple desde una wallet de Vela ya desplegada usó entre 140,000 y 170,000
  unidades de gas on-chain en nuestras mediciones en Gnosis (septiembre de 2026); una
  transferencia simple de ETH desde una cuenta común usa 21,000. Además del gas, el
  relay cobra su comisión; consulta [redes y comisiones](/es-MX/docs/networks-and-fees).
- **La cuenta tiene que estar desplegada.** Tu dirección se calcula con `CREATE2`
  antes de que exista nada on-chain, así que puedes recibir en ella de inmediato; tu
  primera transacción de salida en cada red paga el despliegue del contrato.
- **No todas las cadenas califican.** Las firmas de passkey se verifican con el
  precompilado **EIP-7951 / RIP-7212**, y su dirección forma parte de los datos de configuración
  de cada wallet, así que una red que no lo tenga no puede ejecutar Vela en absoluto.
- **El riesgo de Safe ahora es tu riesgo.** Confiar en un contrato muy usado sigue
  siendo confiar en un contrato. Vela no agregó un segundo contrato propio en el
  camino hacia tu dinero en el que tengas que confiar.

## Qué está auditado y qué no

Los contratos de Safe, sus módulos 4337 y de passkey, y el EntryPoint v0.7 tienen
auditorías de terceros publicadas. **El código propio de Vela (las apps, los
servicios de backend y el contrato de registro) no ha tenido una auditoría de
terceros, y no hay ninguna programada**; es una meta para cuando el proyecto pueda
pagarla, no un compromiso con fecha. Cada contrato, su informe de auditoría y los
problemas que damos seguimiento están en
[auditorías y problemas conocidos](/es-MX/docs/security-audits).

## Compruébalo tú mismo

Tu cuenta está on-chain. Abre tu dirección en un explorador de bloques una vez
desplegada: es un proxy de Safe cuya implementación es el despliegue canónico de
SafeL2 v1.4.1 de Safe, en todas las redes.

Sigue: [auditorías y problemas conocidos](/es-MX/docs/security-audits).
