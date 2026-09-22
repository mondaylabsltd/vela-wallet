---
title: Recuperación e inicio de sesión
description: "Cómo vuelves a entrar a tu wallet en un dispositivo nuevo con cualquiera de tus llaves, dónde se busca la wallet y los límites honestos de recuperar sin frase semilla."
source: 77cf3f24c7c7
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Recuperación e inicio de sesión

Sin frase semilla, la recuperación descansa en dos cosas: **una llave que todavía
tengas** y **un registro público de qué llaves pertenecen a tu wallet**.

## Qué queda registrado cuando creas una wallet

La dirección de tu wallet se calcula a partir de todas las llaves con las que la
creas. Para que cualquiera de esas llaves pueda volver a encontrar la wallet más
adelante, crear una wallet escribe un registro en un **contrato de registro**
público en Gnosis Chain: la llave pública de cada llave, la dirección de la wallet,
su nombre y los datos de registro firmados. El contrato de registro no tiene dueño,
y lo que contiene no se puede editar ni borrar. (La lista completa de lo que es
público está en [crea tu wallet](/es-MX/docs/create-wallet#what-is-public).)

El servicio de índice de llaves públicas de Vela envía ese registro y paga su gas;
el registro en sí vive on-chain, y cualquier app puede leerlo directamente.

## Iniciar sesión en un dispositivo nuevo

1. Abre Vela y elige iniciar sesión.
2. Usa **cualquiera** de tus llaves: una passkey que se sincronizó con este
   dispositivo, un celular cercano (escanea el código QR) o tu llave de seguridad.
3. Vela obtiene la llave pública de esa llave a partir de la firma, la busca
   (primero en el índice de Vela y, si el índice no responde, en el contrato de
   registro en Gnosis y luego en Ethereum) y reconstruye la wallet. Antes de
   mostrártela, verifica que las llaves que encontró de verdad dan como resultado la
   dirección registrada.

Las listas de cuentas no se sincronizan entre dispositivos; iniciar sesión las
reconstruye.

<Callout type="info" title="Si ningún índice ni registro puede responder">
Una wallet con <strong>una sola llave</strong> se puede reconstruir en el
dispositivo sin ningún servidor de por medio: dos firmas de esa llave bastan para
recuperar su llave pública y volver a calcular la dirección. Una wallet con varias
llaves necesita el registro, porque una llave no le puede decir a la app cuáles eran
las demás.
</Callout>

## Copias del registro

El registro en Gnosis es el que las apps leen primero. Desde **Ajustes** también
puedes copiar el registro de tu wallet al mismo contrato de registro en
**Ethereum**, pagando tú el gas, para que el registro exista en una segunda cadena.
Cualquiera puede hacer esa copia; no contiene nada que pueda mover fondos.

## Los límites honestos

<Callout type="warning" title="Una llave perdida está perdida">
Si todas las llaves con las que creaste la wallet desaparecieron (las passkeys
sincronizadas, los celulares, las llaves de seguridad), nadie puede recuperar la
wallet: ni Vela, ni Apple o Google, ni nadie. No hay frase semilla, ni
restablecimiento por soporte, ni puerta trasera.
</Callout>

Lo que hace que eso sea poco probable es tener más de una forma de entrar:

- **Mantén activada la sincronización de passkeys** si usas la passkey de este
  dispositivo. Es lo que lleva la llave a un celular o una computadora nuevos.
- **Protege la cuenta que está detrás.** Quien controle tu cuenta de Apple o de
  Google podría usar una passkey sincronizada; ponle una contraseña robusta y sus
  propias opciones de recuperación.
- **Crea la wallet con más de una llave**, por ejemplo la passkey de tu celular y
  una llave de seguridad física guardada en un lugar seguro. Las llaves solo se
  pueden agregar al crear la wallet ([por qué](/es-MX/docs/signers)). Recuerda que
  cualquier llave puede firmar por sí sola, y que no se puede quitar: si alguna
  llega a quedar comprometida, pasa tus fondos a una wallet nueva
  ([qué hacer](/es-MX/docs/signers)).

## Qué puede y qué no puede hacer Vela

- **Puede:** mantener el índice funcionando, para que tu wallet se encuentre rápido
  en un dispositivo nuevo.
- **No puede:** mover tus fondos, congelar tu wallet, agregar o quitar llaves, ni
  recuperar una llave que perdiste. Vela nunca tiene tus llaves.

Sigue: [firma legible](/es-MX/docs/clear-signing).
