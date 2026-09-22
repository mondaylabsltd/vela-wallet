---
title: O ataque à Bybit e o caminho que ele usou
description: "Em fevereiro de 2025, a Bybit perdeu cerca de US$ 1,5 bilhão. Os contratos Safe não foram quebrados — a interface foi. Esta página explica o caminho do ataque e o que no design da Vela o fecha."
source: ac56b16b531f
---

# O ataque à Bybit e o caminho que ele usou

Em 21 de fevereiro de 2025, a Bybit perdeu cerca de **US$ 1,5 bilhão** de uma
carteira fria multisig Safe. É o maior roubo da história do setor, e vale a pena
ler com atenção, porque quase tudo nele estava *correto*, exceto uma coisa.

## O que aconteceu

A versão curta, a partir das análises públicas pós-incidente:

1. Um atacante comprometeu a **máquina de um desenvolvedor da `Safe{Wallet}`** e
   injetou JavaScript malicioso no bucket AWS S3 que servia o front-end da
   `Safe{Wallet}`. O código entrou em 19 de fevereiro e foi acionado em 21 de
   fevereiro, direcionado ao Safe específico da Bybit.
2. Os signatários da Bybit abriram a interface e revisaram uma transação que
   parecia comum.
3. O payload que de fato foi enviado para as **carteiras de hardware** deles não era
   essa transação. Era um `delegatecall` que sobrescrevia o `masterCopy` do proxy do
   Safe — o slot 0 —, trocando toda a implementação da conta pela do atacante.
4. Os signatários aprovaram. As assinaturas eram válidas. O contrato fez
   exatamente o que mandaram.

A autoria foi atribuída publicamente a atividades ligadas à Coreia do Norte (o FBI
apontou o grupo TraderTraitor).

## O que *não* foi quebrado

- **Não os contratos Safe.** Eles executaram uma instrução assinada de forma
  válida. Nenhum bug do Safe foi explorado.
- **Não a criptografia.** Todas as assinaturas eram autênticas.
- **Não as carteiras de hardware.** Havia aparelhos Ledger no processo, e eles
  assinaram mesmo assim — porque uma carteira de hardware mostra o que recebe, e o
  que ela recebeu foi o payload malicioso. Um aparelho que não consegue decodificar
  um `delegatecall` em algo que uma pessoa consiga avaliar protege a *chave*, não a
  *decisão*.

O que foi quebrado é a premissa por trás de toda interface de carteira: **a de que
a tela que descreve uma transação e os bytes que estão sendo assinados são a mesma
coisa.**

## Por que esse é o caso geral, e não um acidente isolado

A maioria das assinaturas feitas numa carteira web se apoia nessa premissa. A
interface monta o payload, a interface exibe o resumo, e nada independente confere
se um corresponde ao outro. Se o código que serve essa interface for trocado — por
um pipeline de build comprometido, uma CDN sequestrada, uma dependência maliciosa,
uma credencial de deploy roubada —, o resumo passa a ser o que o atacante quiser, e
a sua assinatura é real.

Esse é o risco para o qual o design de assinatura da Vela aponta. Não phishing. Não
uma chave vazada. **Uma tela de assinatura que está mentindo para você.**

## O que a Vela faz a respeito

**Assinatura legível, até a calldata.** Cada transação é decodificada numa intenção
legível antes de você aprovar — valor, destinatário, o que a chamada faz de fato
([ERC-7730](/pt-BR/docs/clear-signing)). Uma chamada que não conseguimos
decodificar é **sinalizada como não decodificável**, e não exibida discretamente
como se estivesse tudo bem. O payload da Bybit era um `delegatecall` que trocava um
endereço de implementação; é exatamente o tipo de coisa que deveria fazer um
signatário parar na hora, e escondê-lo atrás de um resumo amigável foi o que
impediu isso.

Dois limites, para sermos exatos. Um dApp não consegue pedir à Vela um
`delegatecall` diretamente — as solicitações que uma página pode fazer geram
chamadas comuns —, então o próprio payload da Bybit não poderia chegar por esse
caminho. Mas uma página *pode* pedir uma chamada do seu Safe para ele mesmo:
`enableModule`, `addOwnerWithThreshold`, `setFallbackHandler`, `setGuard`. Qualquer
uma delas, assinada uma única vez, entrega a conta tão completamente quanto o
payload da Bybit entregou — um módulo habilitado pode, depois, executar um
`delegatecall` por conta própria. A Vela decodifica essas chamadas, mas ainda não as
bloqueia; **rejeite qualquer solicitação cujo destino seja o endereço da sua própria
carteira.** E, se o próprio código da Vela fosse trocado, como aconteceu com o da
`Safe{Wallet}`, a decodificação também seria a do atacante — e é para isso que serve
o próximo ponto.

**Um caminho independente que pode conferir a interface.** A Vela construiu uma
[página de assinatura](/pt-BR/docs/clear-signing-self-host) sem build e sem
dependências que decodifica a solicitação e faz a assinatura WebAuthn por conta
própria — uma única pasta de arquivos estáticos que você pode ler do começo ao fim,
servir você mesmo ou carregar como extensão de navegador. O objetivo dela é ser uma
segunda opinião que não compartilha a cadeia de suprimentos do app principal.
*Situação: pronta e testada; não publicada, e nenhum app da Vela envia solicitações
para ela ainda.* Esta página vai dizer isso com todas as letras quando mudar.

**Nenhum papel de administrador para perdermos.** As contas da Vela são
[Safe v1.4.1 sem modificações](/pt-BR/docs/account-contract), e a Vela não tem
nenhum papel privilegiado nelas — nenhuma chave de administrador e nenhum caminho
de atualização nosso que pudéssemos ser coagidos ou comprometidos a usar. Mas fique
claro o que isso *não* elimina: a primitiva que os atacantes da Bybit usaram — um
`delegatecall` assinado por um proprietário que reescreve a implementação da conta —
continua existindo em todo Safe, inclusive nos da Vela (as transações em lote da
própria Vela usam `delegatecall` para o MultiSend da Safe). Ela exige uma assinatura
válida de uma das suas chaves. As defesas contra ser convencido a dar essa
assinatura são a decodificação acima e a conferência independente.

**Uma confirmação nova a cada assinatura.** Toda assinatura exige a confirmação da
própria chave — Face ID, digital, PIN, ou um toque e o PIN numa chave de segurança.
Não existe chave de sessão de longa duração, então não existe uma janela em que
algo possa assinar em seu nome sem você presente.

**Auto-hospedagem como última linha de defesa.** Os apps e os serviços de backend
são de código aberto. Se você não quer confiar de jeito nenhum no nosso pipeline de
build, compile a extensão ou um app você mesmo e rode os serviços de que precisar —
o [guia de auto-hospedagem](/pt-BR/docs/self-hosting) mostra como. Isso tira o nosso
pipeline de build da cadeia de confiança; você continua confiando no código que
compila, então leia esse código.

## O que a Vela não afirma

O front-end da Vela poderia ser comprometido do mesmo jeito que o da
`Safe{Wallet}` foi. O nosso código não é auditado. Dizer o contrário seria
exatamente o tipo de garantia que este incidente deveria ter enterrado.

O que o design tenta fazer é estreitar o caminho: tornar o payload legível em vez
de opaco, não ter nenhum papel de administrador que possa ser usado contra você e
dar a você uma forma de conferir com algo que não somos nós. O resumo honesto é que
**esse tipo de ataque é mitigado pelo design, não eliminado**, e as partes que o
endureceriam ainda mais estão listadas, inacabadas, em
[auditorias e problemas conhecidos](/pt-BR/docs/security-audits).

## Fontes

- [SlowMist — Bybit's $1.5 billion theft unveiled: `Safe{Wallet}` front-end code tampered](https://slowmist.medium.com/bybits-1-5-billion-theft-unveiled-safe-wallet-front-end-code-tampered-84b78f0fa9c2)
- [NCC Group — Bybit hack: in-depth technical analysis](https://www.nccgroup.com/research/in-depth-technical-analysis-of-the-bybit-hack/)
- [Sygnia — Investigation into the Bybit hack](https://www.sygnia.co/blog/sygnia-investigation-bybit-hack/)
- [BlockSec — A web2 breach enables the largest crypto hack in history](https://blocksec.com/blog/bybit-incident-a-web2-breach-enables-the-largest-crypto-hack-in-history)
