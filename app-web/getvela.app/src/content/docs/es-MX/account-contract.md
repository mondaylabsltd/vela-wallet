---
title: El contrato de la cuenta
description: "Tu wallet Vela es un Safe v1.4.1 sin modificar. Nada en la ruta de contratos lo escribimos nosotros: aquí está qué te da eso y qué cuesta."
---

# El contrato de la cuenta

Tu wallet no es la estructura de datos privada de una app. Es una cuenta
inteligente **Safe v1.4.1** —el mismo contrato que resguarda tesorerías mucho más
grandes que cualquier cosa que Vela vaya a ver— desplegado exactamente como Safe lo
publica, sin modificación alguna.

La frase es corta y las consecuencias no, así que esta página las desglosa.

## Nada en la ruta es nuestro

Entre tú y tu dinero hay cuatro contratos. Vela no escribió ninguno:

| Contrato | Quién lo escribió |
| --- | --- |
| [Safe v1.4.1](https://github.com/safe-fndn/safe-smart-account/tree/release/v1.4.1) (la cuenta misma, un proxy) | Safe |
| [Safe 4337 Module](https://github.com/safe-global/safe-modules/tree/main/modules/4337) | Safe |
| [SafeWebAuthnSharedSigner](https://github.com/safe-global/safe-modules/tree/main/modules/passkey) (verifica tu llave P-256) | Safe |
| [ERC-4337 EntryPoint v0.7](https://eips.ethereum.org/EIPS/eip-4337) | Los autores de ERC-4337 |

No existe un contrato de Vela. El repositorio no contiene nada de Solidity, y lo
puedes comprobar con un comando:

```bash
git clone https://github.com/mondaylabsltd/vela-wallet
find vela-wallet -name '*.sol'   # no imprime nada
```

Cuando Vela agrega una red, despliega **esos** contratos en sus direcciones
canónicas. No despliega un contrato de diseño propio ni tiene ningún rol
privilegiado en el tuyo: sin llave de administrador, sin ruta de actualización, sin
módulo que podamos agregar.

## Por qué «sin modificar» es la palabra que importa

Muchas wallets dicen estar construidas sobre «un Safe», «un fork de Safe» o «una
cuenta inspirada en Safe». Un fork es un contrato nuevo con reputación vieja. En la
práctica, las diferencias:

**Las auditorías aplican a lo que de verdad estás usando.** Los reportes de Safe
cubren el bytecode de estas versiones exactas. Las auditorías de un fork cubren el
código anterior al fork. Si una wallet modificó el contrato de la cuenta, cada
auditoría que cita es la auditoría de otra cosa — y la modificación es justo la
parte que nadie revisó.

**El ecosistema trata tu cuenta como un Safe, porque lo es.** Los exploradores la
decodifican. Las propias herramientas de transacciones de Safe la entienden. Si Vela
desaparece mañana, tu wallet no queda en un formato huérfano: es la cuenta
inteligente con más herramientas alrededor en Ethereum, y cualquier interfaz
compatible con Safe puede manejarla. Eso es lo que hace que
[«si Vela desaparece, tu wallet no»](/es-MX/docs/why-vela) sea una afirmación sobre
contratos y no sobre nuestras intenciones.

**La superficie de ataque es una que todos los demás también vigilan.** Un contrato
de cuenta a la medida solo lo vigila su autor. A este lo vigila todo el que guarda
dinero en un Safe.

## Qué cuesta

Ser estándar no sale gratis, y los trade-offs son reales:

- **Gas.** Una cuenta inteligente verifica una firma on-chain. Cuenta con cerca de
  1.5 a 3 veces el gas de una transferencia EOA simple, según la cadena. Ve
  [redes y comisiones](/es-MX/docs/networks-and-fees).
- **La cuenta debe desplegarse.** Tu dirección se calcula con `CREATE2` antes de
  que exista algo on-chain, así que puedes recibir de inmediato, pero la primera
  transacción de salida paga el despliegue del contrato.
- **No toda cadena califica.** El firmante WebAuthn verifica una firma P-256
  on-chain, lo que requiere el precompilado **RIP-7212**. Vela se niega a habilitar
  una red que no lo tenga en vez de caer a un verificador más débil.
- **El riesgo de Safe ahora es tu riesgo.** Confiar en un contrato ampliamente
  usado sigue siendo confiar en un contrato. Lo que Vela puede decir es que no
  agregó encima una segunda cosa en la que confiar.

## Qué está auditado y qué no

Los contratos de Safe y el módulo firmante WebAuthn están auditados por terceros, y
esos reportes son públicos. **El código de la app de Vela no ha tenido una auditoría
independiente**, y no hay ninguna programada: es una meta para cuando el proyecto
pueda pagarla, no un compromiso con fecha. Cada contrato del que Vela depende, su
reporte de auditoría y los problemas que seguimos están en
[auditorías y problemas conocidos](/es-MX/docs/security-audits).

## Compruébalo tú

Tu cuenta está on-chain. Ábrela en un explorador de bloques y lee la dirección de
implementación: será el despliegue canónico de Safe en v1.4.1, byte por byte, en
cada red que Vela soporta.
