---
title: Por qué hicimos Vela
description: "La versión larga: dónde se supone que guardas doce palabras, qué cambiaron las passkeys, qué no pudimos aceptar en las wallets que ya usábamos y la concesión que elegimos a cambio."
source: 06307425f631
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Por qué hicimos Vela

No nos propusimos hacer otra wallet. Empezamos con una pregunta a la que nunca le
encontramos una respuesta limpia:

> ¿Dónde se supone que guardas doce palabras?

## La respuesta honesta es una captura de pantalla

Si las guardas en Notas, basta con que te roben el celular para tener un problema.
Si las escribes en papel, ahora tienes que pensar en incendios, en agua, en
mudanzas, en roomies, en bolsas de basura y en si tu yo del futuro se va a acordar
de dónde quedó «el lugar seguro».

Para mucha gente, la respuesta honesta es una captura de pantalla en la galería del
celular. Todo mundo sabe que está mal. Lo hacen de todos modos, porque la respuesta
«correcta» es demasiado difícil de sostener en la vida diaria.

Una frase semilla es un secreto que tiene que sobrevivir décadas de vida normal sin
que nadie lo copie, lo fotografíe, lo escriba en el campo equivocado o se lo lea en
voz alta a alguien muy amable al teléfono. Para una persona cuidadosa, no es un
problema difícil. Para una persona, sí.

## Luego las passkeys cambiaron cómo podía sentirse una wallet

Usábamos [Base Account](https://account.base.app) todos los días, y firmar con Face
ID se sentía natural como nunca se sintieron las frases semilla: menos como manejar
material peligroso y más como usar el resto de internet.

Pero entre más la usábamos, más nos topábamos con límites que no podíamos ignorar:

- una **llave de recuperación generada en un navegador** en la que no te quedaba
  más que confiar,
- **nada de redes personalizadas**,
- **ninguna forma de alojarla nosotros mismos**,
- y el problema silencioso que era el más grande: **si el servicio desaparecía, la
  wallet desaparecía con él.**

Así que construimos la versión de la que sí queríamos depender.

## Qué es Vela en realidad

Vela es **una wallet con passkeys que puede ser completamente tuya.**

Tu passkey se queda donde tu dispositivo ya la protege: el Llavero de iCloud, el
Administrador de contraseñas de Google o una llave de seguridad física que traes
contigo. Cuando firmas una transacción, Vela le pide a tu dispositivo que la firme;
tu dispositivo firma y le devuelve solo la firma. Vela nunca ve la llave.

La mayoría de las wallets todavía tiene un momento peligroso, aunque sea breve:
palabras en una pantalla, una frase semilla en la memoria, una llave de recuperación
abierta en una pestaña del navegador. Vela está diseñada para que ese momento no
exista.

<Callout type="info" title="No es una promesa, es la arquitectura">
No podemos acceder a tus llaves. No es que «prometamos no hacerlo»: en Vela no hay
ningún camino en el código que pudiera hacerlo, porque WebAuthn no lo permite. La
wallet es una <a href="/es-MX/docs/account-contract">cuenta inteligente Safe</a> que
opera con una firma que produce tu dispositivo y que nosotros solo recibimos. Lo que
sí decide la app con la que firmas es <em>qué</em> se le pide firmar a tu llave, y
por eso el <a href="/es-MX/docs/whitepaper">modelo de amenazas</a> le dedica tanto
espacio.
</Callout>

Hicimos Vela **de código abierto** para que puedas comprobarlo tú mismo, y
**autoalojable** para que una wallet existente siga funcionando sin los servidores
de nuestra empresa, con un límite: el dominio al que pertenecen tus passkeys, que la
[guía de autoalojamiento](/es-MX/docs/self-hosting) explica junto con las formas de
sortearlo. Y construimos sobre
[contratos de Safe](https://github.com/safe-fndn/safe-smart-account/tree/release/v1.4.1)
sin modificar porque, cuando hay dinero de por medio, el camino aburrido y probado
en batalla es el correcto: son los mismos contratos que ya protegen miles de
millones on-chain.

## La concesión que elegimos

Sigue habiendo una concesión, y sería deshonesto esconderla.

Con Vela, tu cuenta de Apple o de Google importa, porque ahí vive una passkey
sincronizada. Si pierdes esa cuenta o borras la passkey, no hay frase semilla, ni
restablecimiento por soporte, ni puerta trasera.

Pero toda wallet de autocustodia te pide elegir con qué riesgo prefieres vivir. Una
frase semilla se puede copiar, capturar en pantalla, robar con phishing o escribir
en el sitio equivocado a la una de la mañana. Una passkey es distinta: no hay
palabras que revelar, ni secreto que pegar, ni sitio falso que pueda engañarte para
que la entregues. Tu navegador solo la ofrece a páginas del dominio real.

Y la elección no es de todo o nada. Una wallet se puede crear con **hasta siete
firmantes**, cualquiera de los cuales puede firmar por sí solo: passkeys en
distintos dispositivos, un celular cercano que escaneas o una llave de seguridad
USB/NFC. Si prefieres que tu wallet no dependa para nada de una cuenta de
plataforma, puedes usar solo llaves de seguridad físicas; dos, porque una wallet no
puede descansar sobre una sola llave que no se sincroniza en ningún lado. La única
condición es el momento: tu dirección se deriva del conjunto completo de llaves, así
que se eligen al crear la wallet.

<Callout type="warning" title="Lo que esto no te da">
Los firmantes adicionales son una forma de volver a entrar, no un segundo candado.
Como cualquier llave puede firmar sola, agregar una llave física te protege de
<em>perder</em> el acceso, pero no detiene a alguien que ya tomó el control de una
de tus llaves. Esa es la forma honesta del 1-of-n.
</Callout>

## Por eso existe Vela

Una wallet sin frase semilla que esconder, sin llave de recuperación en la que
confiar a ciegas y sin una empresa que tengas que esperar que dure para siempre.

Si quieres comprobar lo que decimos en vez de creerlo: el
[whitepaper](/es-MX/docs/whitepaper) tiene la arquitectura,
[Auditorías y problemas conocidos](/es-MX/docs/security-audits) enumera cada
contrato del que dependemos y qué se ha auditado y qué no, y todo el código está
[en GitHub](https://github.com/mondaylabsltd/vela-wallet).

Sigue: [instala Vela](/es-MX/docs/install).
