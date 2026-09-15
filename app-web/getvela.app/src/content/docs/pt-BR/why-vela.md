---
title: Por que criamos a Vela
description: A versão longa — onde é que você deveria guardar doze palavras, o que as passkeys mudaram, o que não dava para aceitar nas carteiras que a gente já usava, e o preço que escolhemos em troca.
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Por que criamos a Vela

A gente não saiu por aí querendo fazer mais uma carteira. Começamos com uma
pergunta que nunca conseguimos responder direito:

> Onde é que você deveria guardar doze palavras?

## A resposta honesta é um print

Coloque no app de Notas e você está a um celular roubado do problema. Escreva no
papel e agora você pensa em incêndio, água, mudança, colegas de apartamento, saco de
lixo — e se o você do futuro vai lembrar onde era «o lugar seguro».

Para muita gente, a resposta honesta é um print no rolo da câmera. Todo mundo sabe
que é errado. Todo mundo faz assim mesmo — porque a resposta «certa» é pesada demais
para se conviver com ela.

Uma frase-semente é um segredo que precisa sobreviver a décadas de vida comum sem
nunca ser copiado, fotografado, digitado no campo errado ou lido em voz alta para
alguém prestativo no telefone. Não é um problema difícil para uma pessoa
cuidadosíssima. É um problema difícil para uma pessoa.

## Aí as passkeys mudaram a sensação de usar uma carteira

A gente usava [Base Account](https://account.base.app) todo dia, e assinar com Face
ID parecia óbvio de um jeito que a frase-semente nunca foi — menos manusear material
perigoso, mais usar o resto da internet.

Mas quanto mais usávamos, mais batíamos em limites impossíveis de ignorar:

- uma **chave de recuperação gerada dentro do navegador**, em que só dava para
  confiar,
- **nenhuma rede personalizada**,
- **nenhum jeito de hospedar por conta própria**,
- e o problema silencioso, que era o maior: **se o serviço sumir, a carteira some
  junto.**

Então construímos a versão da qual a gente queria depender.

## O que a Vela é de verdade

A Vela é **uma carteira com passkey que pode ser inteiramente sua.**

Sua passkey fica onde o seu aparelho já a protege — Chaveiro do iCloud, Gerenciador
de senhas do Google ou uma chave de segurança de hardware na sua mão. Quando você
assina uma transação, a Vela manda um desafio para o aparelho; o aparelho assina e
devolve só a assinatura. A Vela nunca vê a chave.

A maioria das carteiras ainda tem um momento perigoso, mesmo que breve: palavras na
tela, uma frase-semente na memória, uma chave de recuperação parada numa aba. A Vela
é projetada para que esse momento não exista.

<Callout type="info" title="Não é promessa — é arquitetura">
A gente não consegue acessar suas chaves. Não é «prometemos não fazer isso»: não
existe caminho de código na Vela capaz disso. A carteira é uma
<a href="/pt-BR/docs/security-audits">conta inteligente Safe</a> operada por uma
assinatura que o seu aparelho produz e que nós apenas recebemos.
</Callout>

Deixamos a Vela **de código aberto** para você conferir isso por conta própria, e
**auto-hospedável** para que sua carteira nunca dependa de a nossa empresa continuar
no ar. E construímos sobre
[contratos Safe](https://github.com/safe-fndn/safe-smart-account/tree/release/v1.4.1)
sem modificações, porque o caminho chato e testado em campo é o certo quando o
dinheiro é dos outros — os mesmos contratos que já guardam bilhões on-chain.

## O preço que escolhemos

Ainda existe um preço, e escondê-lo seria desonesto.

Com a Vela, sua conta Apple ou Google importa, porque é ali que mora uma passkey
sincronizada. Perca essa conta, ou apague a passkey, e não há frase-semente, nem
reset pelo suporte, nem porta dos fundos.

Mas toda carteira autocustodiada pede que você escolha com qual risco prefere
conviver. Uma frase-semente pode ser copiada, fotografada, phishada ou digitada no
site errado à uma da manhã. Uma passkey é diferente: não há palavras a revelar, nem
segredo para colar, nem site falso capaz de te fazer entregá-la. Seu aparelho assina
para o domínio verdadeiro, ou não assina.

E a escolha não é binária. Uma carteira pode ser criada com **até sete assinantes**,
cada um capaz de assinar sozinho — passkeys em aparelhos diferentes, um celular por
perto que você escaneia, ou uma chave de segurança USB/NFC. Se você prefere que sua
carteira não dependa de conta de plataforma nenhuma, dá para fazer da primeiríssima
chave uma chave de segurança de hardware. A única condição é o momento: seu endereço
é derivado do conjunto inteiro de chaves, então elas são escolhidas quando você cria
a carteira.

<Callout type="warning" title="O que isso não compra">
Assinantes extras são um caminho de volta, não uma segunda tranca. Como qualquer
chave sozinha assina, acrescentar uma chave de hardware protege você de
<em>perder</em> o acesso — não impede quem já tomou uma das suas chaves. É essa a
forma honesta do 1-of-n.
</Callout>

## É por isso que a Vela existe

Uma carteira sem frase-semente para esconder, sem chave de recuperação para
confiar, e sem empresa que você precise torcer para continuar existindo.

Se você quer conferir as afirmações em vez de aceitá-las: o
[whitepaper](/pt-BR/docs/whitepaper) tem a arquitetura,
[auditorias e problemas conhecidos](/pt-BR/docs/security-audits) lista cada contrato
de que dependemos e o que foi auditado ou não, e todo o código está
[no GitHub](https://github.com/mondaylabsltd/vela-wallet).

A seguir: [instalar a Vela](/pt-BR/docs/install).
