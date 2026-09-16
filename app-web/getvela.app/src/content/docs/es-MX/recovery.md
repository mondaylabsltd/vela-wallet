---
title: Recuperación e inicio de sesión
description: Cómo Vela te deja recuperar tu wallet en un dispositivo nuevo sin frase semilla — y los límites honestos de ese modelo.
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Recuperación e inicio de sesión

Lo más difícil de una wallet sin frase semilla es la recuperación: si no hay doce
palabras, ¿cómo vuelves a entrar desde un teléfono nuevo? Aquí está exactamente
cómo lo resuelve Vela.

## Cómo funciona

Cuando creas una wallet, dos cosas se publican en el **índice de passkeys** de
Vela:

- La **llave pública** de tu passkey (nunca la privada).
- El **nombre** que elegiste para la wallet.

La llave pública se guarda en la blockchain Gnosis por medio de un contrato, así
que es legible públicamente y no depende de que los servidores de Vela sigan en
línea.

Tu llave **privada**, mientras tanto, es una passkey que sincroniza el llavero de
tu plataforma: **Llavero de iCloud** en dispositivos Apple, **Gestor de contraseñas
de Google** en Android.

Para iniciar sesión en un dispositivo nuevo:

1. Inicia sesión en la misma cuenta de iCloud o Google, con la sincronización del
   llavero activada.
2. Abre Vela y elige iniciar sesión.
3. Autentícate con tu passkey. La plataforma aporta la passkey sincronizada; el
   índice aporta la cuenta que le corresponde. Tu wallet está de vuelta.

El índice es un caché, no un punto único de falla. Si alguna vez no se pudiera
alcanzar y tu cuenta no estuviera en el almacenamiento local, Vela puede
reconstruir tu llave pública en el dispositivo a partir de dos firmas de passkey y
volver a derivar de ahí la dirección de tu wallet, sin servidor de por medio.

<Callout type="info" title="Por qué se separa así">
La llave pública en el índice on-chain permite que cualquiera (incluso una
instalación recién hecha) encuentre tu cuenta. La llave privada, sincronizada por
el llavero de la plataforma en la que confías, es la que de verdad autoriza
transacciones. Todo lo que está en el índice es información pública, y nada de eso
puede mover tus fondos: solo las firmas de tu passkey pueden.
</Callout>

## Los límites, sin adornos

Autocustodia significa que la responsabilidad es real. Esto es lo que hay que
entender.

<Callout type="warning" title="Tu recuperación depende del llavero de tu plataforma">
El inicio de sesión entre dispositivos de Vela se apoya en que tu passkey se
sincronice por el Llavero de iCloud o el Gestor de contraseñas de Google. Mantén
esa cuenta segura y sus opciones de recuperación al día. Si pierdes
<strong>tanto</strong> tus dispositivos <strong>como</strong> el llavero de la
cuenta de la plataforma, Vela no puede regenerar tu llave privada: por diseño,
nunca la tuvimos.
</Callout>

Consejos prácticos:

- **Deja la sincronización del llavero prendida.** Es lo que lleva tu passkey de un
  dispositivo a otro.
- **Asegura tu cuenta de Apple / Google** con una contraseña fuerte y sus propios
  métodos de recuperación. Esa cuenta ya es parte de la seguridad de tu wallet.
- **Ten más de un dispositivo con sesión iniciada** donde puedas, para que perder
  un teléfono sea una molestia y no una crisis.

## Qué puede y qué no puede hacer Vela

- **Puede:** ayudarte a encontrar tu cuenta de nuevo por el índice público.
- **No puede:** mover tus fondos, congelar tu wallet ni recuperar una llave
  privada. Vela nunca la tuvo. Ese es todo el punto de la autocustodia — y el trato
  que haces a cambio.
