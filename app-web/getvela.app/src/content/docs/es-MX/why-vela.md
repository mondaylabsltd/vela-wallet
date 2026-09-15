---
title: Por qué hicimos Vela
description: La versión larga — dónde se supone que guardas doce palabras, qué cambiaron las passkeys, qué no pudimos aceptar en las wallets que ya usábamos y el costo que elegimos a cambio.
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Por qué hicimos Vela

No salimos a hacer otra wallet. Empezamos con una pregunta que nunca pudimos
responder limpiamente:

> ¿Dónde se supone que guardas doce palabras?

## La respuesta honesta es una captura de pantalla

Si las metes en Notas, estás a un teléfono robado del problema. Si las escribes en
papel, ahora piensas en incendios, agua, mudanzas, roomies, bolsas de basura y en si
tu yo del futuro se acordará de dónde estaba «el lugar seguro».

Para mucha gente la respuesta honesta es una captura en el carrete. Todos saben que
está mal. Lo hacen de todos modos, porque la respuesta «correcta» es demasiado
pesada para vivir con ella.

Una frase semilla es un secreto que tiene que sobrevivir décadas de vida ordinaria
sin ser copiado, fotografiado, tecleado en la casilla equivocada ni leído en voz
alta a alguien servicial por teléfono. No es un problema difícil para una persona
cuidadosa. Es un problema difícil para una persona.

## Luego las passkeys cambiaron lo que una wallet puede sentirse

Usábamos [Base Account](https://account.base.app) todos los días, y firmar con Face
ID se sentía obvio de un modo en que las frases semilla nunca lo fueron: menos
manejar material peligroso, más usar el resto de internet.

Pero entre más la usábamos, más chocábamos con bordes que no se podían ignorar:

- una **llave de recuperación generada en un navegador** en la que simplemente
  tenías que confiar,
- **nada de redes personalizadas**,
- **ninguna forma de hospedarla nosotros mismos**,
- y el problema callado que era el más grande: **si el servicio desaparece, la
  wallet se va con él.**

Así que construimos la versión de la que sí queríamos depender.

## Qué es Vela en realidad

Vela es **una wallet con passkey que puedes tener por completo.**

Tu passkey se queda donde tu dispositivo ya la protege: Llavero de iCloud, Gestor de
contraseñas de Google o una llave de seguridad de hardware que traes tú. Cuando
firmas una transacción, Vela manda un reto a tu dispositivo; tu dispositivo lo firma
y devuelve solo la firma. Vela nunca ve la llave.

La mayoría de las wallets todavía tiene un momento peligroso, aunque sea breve:
palabras en una pantalla, una frase semilla en memoria, una llave de recuperación
sentada en una pestaña. Vela está diseñada para que ese momento no exista.

<Callout type="info" title="No es una promesa: es la arquitectura">
No podemos acceder a tus llaves. No es «prometemos no hacerlo»: no hay una ruta de
código en Vela que pudiera. La wallet es una
<a href="/es-MX/docs/security-audits">cuenta inteligente Safe</a> operada por una
firma que produce tu dispositivo y que nosotros solo recibimos.
</Callout>

Hicimos Vela **de código abierto** para que puedas verificarlo tú, y
**autohospedable** para que tu wallet nunca dependa de que nuestra empresa siga en
línea. Y la construimos sobre
[contratos Safe](https://github.com/safe-fndn/safe-smart-account/tree/release/v1.4.1)
sin modificar, porque el camino aburrido y probado es el correcto cuando está en
juego el dinero de la gente: los mismos contratos que ya resguardan miles de
millones on-chain.

## El costo que elegimos

Sigue habiendo un costo, y esconderlo sería deshonesto.

Con Vela, tu cuenta de Apple o Google importa, porque ahí vive una passkey
sincronizada. Pierde esa cuenta, o borra la passkey, y no hay frase semilla, ni
reinicio por soporte, ni puerta trasera.

Pero toda wallet de autocustodia te pide elegir con qué riesgo prefieres vivir. Una
frase semilla se puede copiar, capturar, robar con phishing o teclear en el sitio
equivocado a la una de la mañana. Una passkey es distinta: no hay palabras que
revelar, ni secreto que pegar, ni sitio falso capaz de hacer que la entregues. Tu
dispositivo firma para el dominio real, o no firma.

Y la decisión no es binaria. Una wallet se puede crear con **hasta siete
firmantes**, y cualquiera de ellos firma por su cuenta: passkeys en distintos
dispositivos, un teléfono cercano que escaneas o una llave de seguridad USB/NFC. Si
prefieres que tu wallet no dependa en absoluto de una cuenta de plataforma, puedes
hacer que la primerísima llave sea una llave de seguridad de hardware. La única
condición es el momento: tu dirección se deriva del conjunto completo de llaves, así
que se eligen al crear la wallet.

<Callout type="warning" title="Lo que esto no te compra">
Los firmantes extra son un camino de regreso, no un segundo candado. Como cualquier
llave sola puede firmar, agregar una llave de hardware te protege de
<em>perder</em> el acceso: no detiene a alguien que ya se apoderó de una de tus
llaves. Esa es la forma honesta del 1-of-n.
</Callout>

## Por eso existe Vela

Una wallet sin frase semilla que esconder, sin llave de recuperación en la que
confiar y sin una empresa de la que tengas que esperar que siga ahí para siempre.

Si quieres comprobar las afirmaciones en vez de creerlas: el
[whitepaper](/es-MX/docs/whitepaper) tiene la arquitectura,
[auditorías y problemas conocidos](/es-MX/docs/security-audits) lista cada contrato
del que dependemos y qué está auditado y qué no, y todo el código está
[en GitHub](https://github.com/mondaylabsltd/vela-wallet).

Sigue: [instalar Vela](/es-MX/docs/install).
