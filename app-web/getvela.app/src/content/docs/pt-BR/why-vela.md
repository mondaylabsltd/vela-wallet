---
title: Por que criamos a Vela
description: "A versão longa — onde é que você deveria guardar doze palavras, o que as passkeys mudaram, o que não conseguimos aceitar nas carteiras que já usávamos e a contrapartida que escolhemos no lugar."
source: 06307425f631
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Por que criamos a Vela

Não saímos por aí querendo fazer mais uma carteira. Começamos com uma pergunta que
nunca conseguimos responder direito:

> Onde é que você deveria guardar doze palavras?

## A resposta honesta é um print

Coloque no app de Notas e você está a um celular roubado de ter problemas. Escreva
no papel e agora você tem que pensar em incêndio, água, mudança, colegas de
apartamento, saco de lixo — e se o você do futuro vai lembrar onde ficava “o lugar
seguro”.

Para muita gente, a resposta honesta é um print na galeria de fotos. Todo mundo sabe
que é errado. Todo mundo faz assim mesmo — porque a resposta “certa” é difícil
demais de manter no dia a dia.

Uma frase de recuperação é um segredo que precisa sobreviver a décadas de vida
comum sem nunca ser copiado, fotografado, digitado no campo errado ou lido em voz
alta para alguém muito prestativo do outro lado da linha. Para uma pessoa
cuidadosa, isso não é um problema difícil. Para uma pessoa comum, é.

## Aí as passkeys mudaram o que uma carteira podia ser

Usávamos a [Base Account](https://account.base.app) todo dia, e assinar com Face ID
parecia óbvio de um jeito que a frase de recuperação nunca pareceu — menos como
manusear material perigoso, mais como usar o resto da internet.

Mas quanto mais usávamos, mais esbarrávamos em limites impossíveis de ignorar:

- uma **chave de recuperação gerada no navegador**, em que você simplesmente
  tinha que confiar;
- **nada de redes personalizadas**;
- **nenhum jeito de hospedar por conta própria**;
- e o problema silencioso, que era o maior de todos: **se o serviço sumisse, a
  carteira sumia junto.**

Então construímos a versão de que queríamos depender.

## O que a Vela é de verdade

A Vela é **uma carteira com passkeys que pode ser inteiramente sua.**

Sua passkey fica onde o seu aparelho já a protege — nas Chaves do iCloud, no
Gerenciador de senhas do Google ou numa chave de segurança física que fica com
você. Quando você assina uma transação, a Vela pede ao seu aparelho que assine; o
aparelho assina e devolve só a assinatura. A Vela nunca vê a chave em si.

A maioria das carteiras ainda tem um momento perigoso, mesmo que breve: palavras na
tela, uma frase de recuperação na memória, uma chave de recuperação parada numa aba
do navegador. A Vela foi projetada para que esse momento nunca exista.

<Callout type="info" title="Não é promessa — é arquitetura">
Não conseguimos acessar suas chaves. Não é “prometemos não fazer isso” — não existe
nenhum caminho no código da Vela capaz disso; o WebAuthn não permite. A carteira é
uma <a href="/pt-BR/docs/account-contract">conta inteligente Safe</a> operada por
uma assinatura que o seu aparelho produz e que nós apenas recebemos. O que o app com
que você assina decide, sim, é <em>o que</em> a sua chave é chamada a assinar — e é
por isso que o <a href="/pt-BR/docs/whitepaper">modelo de ameaças</a> dedica tanto
espaço a isso.
</Callout>

Deixamos a Vela **de código aberto** para que você possa conferir isso por conta
própria, e **auto-hospedável** para que uma carteira que já existe continue
funcionando sem os servidores da nossa empresa — com um limite, o domínio ao qual as
suas passkeys pertencem, que o [guia de auto-hospedagem](/pt-BR/docs/self-hosting)
explica junto com as formas de contorná-lo. E construímos sobre
[contratos Safe](https://github.com/safe-fndn/safe-smart-account/tree/release/v1.4.1)
sem modificações, porque, quando o dinheiro é dos outros, o caminho sem graça e
testado em campo é o certo — os mesmos contratos que já protegem bilhões on-chain.

## A contrapartida que escolhemos

Ainda existe uma contrapartida, e escondê-la seria desonesto.

Com a Vela, a sua conta Apple ou Google importa, porque é nela que fica uma passkey
sincronizada. Perca essa conta, ou apague a passkey, e não há frase de recuperação,
nem redefinição pelo suporte, nem porta dos fundos.

Mas toda carteira de autocustódia pede que você escolha com qual risco prefere
conviver. Uma frase de recuperação pode ser copiada, fotografada, roubada por
phishing ou digitada no site errado à uma da manhã. Uma passkey é diferente: não há
palavras para revelar, nem segredo para colar, nem site falso capaz de enganar você
para que a entregue. O navegador só oferece a passkey a páginas do domínio
verdadeiro.

E a escolha não é tudo ou nada. Uma carteira pode ser criada com **até sete
signatários**, e qualquer um deles pode assinar sozinho — passkeys em aparelhos
diferentes, um celular por perto que você conecta por QR code ou uma chave de
segurança USB/NFC. Se você prefere que a sua carteira não dependa de nenhuma conta
de plataforma, pode usar só chaves de segurança físicas — duas, porque uma carteira
não pode depender de uma única chave que não é sincronizada em lugar nenhum. A única
condição é o momento: o seu endereço é derivado do conjunto completo de chaves,
então elas são escolhidas quando você cria a carteira.

<Callout type="warning" title="O que isso não garante">
Signatários extras são um caminho de volta, não uma segunda tranca. Como qualquer
chave assina sozinha, adicionar uma chave física protege você de <em>perder</em> o
acesso — mas não impede quem já tomou uma das suas chaves. Essa é a forma honesta
do 1-of-n.
</Callout>

## É por isso que a Vela existe

Uma carteira sem frase de recuperação para esconder, sem chave de recuperação em
que confiar e sem uma empresa que você precise torcer para continuar existindo para
sempre.

Se você quer conferir as afirmações em vez de simplesmente aceitá-las: o
[whitepaper](/pt-BR/docs/whitepaper) traz a arquitetura,
[Auditorias e problemas conhecidos](/pt-BR/docs/security-audits) lista cada contrato
de que dependemos e o que foi ou não auditado, e todo o código está
[no GitHub](https://github.com/mondaylabsltd/vela-wallet).

A seguir: [instalar a Vela](/pt-BR/docs/install).
