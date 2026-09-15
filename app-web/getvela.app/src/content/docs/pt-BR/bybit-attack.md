---
title: O ataque à Bybit
description: Em fevereiro de 2025 a Bybit perdeu cerca de US$ 1,5 bilhão. Os contratos Safe não quebraram — a interface quebrou. Esta página explica o caminho usado e o que o desenho da Vela fecha.
---

# O ataque à Bybit

Em 21 de fevereiro de 2025, a Bybit perdeu cerca de **US$ 1,5 bilhão** de uma
carteira fria multisig da Safe. É o maior roubo da história do setor, e vale ler com
calma, porque quase tudo ali estava *correto*, menos uma coisa.

## O que aconteceu

A versão curta, a partir dos post-mortems públicos:

1. Um atacante comprometeu a **máquina de um desenvolvedor da `Safe{Wallet}`** e
   injetou JavaScript malicioso no bucket AWS S3 que servia o front-end da
   `Safe{Wallet}`. O código entrou em 19 de fevereiro e foi acionado em 21, mirando
   o Safe específico da Bybit.
2. Os signatários da Bybit abriram a interface e revisaram uma transação que parecia
   comum.
3. O que de fato chegou às **carteiras de hardware** deles não era aquela
   transação. Era um `delegatecall` que sobrescrevia a `masterCopy` do proxy do Safe
   — o slot 0 — trocando toda a implementação da conta pela do atacante.
4. Os signatários aprovaram. As assinaturas eram válidas. O contrato fez exatamente
   o que foi mandado.

A atribuição pública apontou atividade ligada à Coreia do Norte (o FBI citou o grupo
TraderTraitor).

## O que *não* quebrou

- **Não foram os contratos Safe.** Eles executaram uma instrução assinada de forma
  válida. Nenhum bug da Safe foi explorado.
- **Não foi a criptografia.** Todas as assinaturas eram genuínas.
- **Não foram as carteiras de hardware.** Havia dispositivos Ledger no caminho e eles
  assinaram mesmo assim — porque uma carteira de hardware mostra o que recebe, e o
  que recebeu foi a carga maliciosa. Um aparelho que não consegue traduzir um
  `delegatecall` em algo que um humano avalie protege a *chave*, não a *decisão*.

O que quebrou foi a suposição embaixo de toda interface de carteira: **que a tela
descrevendo uma transação e os bytes sendo assinados são a mesma coisa.**

## Por que esse é o caso geral, não um evento esquisito

Toda assinatura que você já produziu numa carteira web se apoiou nessa suposição. A
interface monta a carga, a interface desenha o resumo, e nada independente confere
se um bate com o outro. Se o código que serve essa interface for substituído — uma
pipeline de build comprometida, um CDN sequestrado, uma dependência maliciosa, uma
credencial de deploy roubada — o resumo vira o que o atacante quiser, e a sua
assinatura é real.

É esse o risco que o desenho de assinatura da Vela mira. Não phishing. Não uma chave
vazada. **Uma tela de assinatura que está mentindo para você.**

## O que a Vela faz a respeito

**Assinatura legível, até a calldata.** Toda transação é traduzida em intenção
legível antes de você aprovar — valor, destinatário, o que a chamada realmente faz
([ERC-7730](/pt-BR/docs/clear-signing)). Uma chamada que não conseguimos decodificar
é **sinalizada como indecodificável**, não desenhada em silêncio como se estivesse
tudo bem. A carga da Bybit era um `delegatecall` que trocava um endereço de
implementação; é exatamente o tipo de coisa que deveria travar um signatário na
hora, e escondê-la atrás de um resumo simpático é a razão de não ter travado.

**Um caminho independente capaz de conferir a interface.** A Vela está construindo
uma página de assinatura sem build e sem dependências, que desenha a intenção e faz
a assinatura WebAuthn por conta própria — uma única pasta de arquivos estáticos que
você pode ler de ponta a ponta, servir você mesmo ou rodar como extensão do
navegador. O propósito inteiro dela é ser uma segunda opinião que não compartilha a
cadeia de suprimentos do app principal. *Status: construída e testada, ainda não
publicada.* Quando sair será opcional, e esta página vai dizer isso com todas as
letras quando mudar.

**Nenhum contrato que a gente possa atualizar.** A carga da Bybit funcionou
substituindo a implementação da conta. As contas da Vela são
[Safe v1.4.1 sem modificações](/pt-BR/docs/account-contract) e a Vela não tem
nenhum papel privilegiado nelas: sem chave de administrador, sem caminho de upgrade
que possam nos obrigar ou comprometer a usar.

**Uma checagem biométrica nova a cada assinatura.** Não existe chave de sessão de
vida longa, então não existe janela em que algo assine no seu lugar sem você.

**Auto-hospedagem como rede de proteção.** O app e todos os serviços de backend são
código aberto. Se você não quer confiar na nossa pipeline de build, rode a sua — é a
única resposta a essa classe de ataque que não exige confiar em alguém.

## O que a Vela não afirma

O front-end da Vela poderia ser comprometido do mesmo jeito que o da
`Safe{Wallet}`. Nosso código não é auditado. Dizer outra coisa seria exatamente o
tipo de garantia que esse incidente deveria ter encerrado.

O que o desenho tenta fazer é estreitar o caminho: tornar a carga legível em vez de
opaca, remover a primitiva de upgrade de que o ataque dependeu, e te dar um jeito de
conferir com algo que não somos nós. O resumo honesto é que **essa classe de ataque
é mitigada por design, não eliminada** — e as partes que a deixariam mais dura estão
listadas, inacabadas, em
[auditorias e problemas conhecidos](/pt-BR/docs/security-audits).

## Fontes

- [SlowMist — Bybit's $1.5 billion theft unveiled: `Safe{Wallet}` front-end code tampered](https://slowmist.medium.com/bybits-1-5-billion-theft-unveiled-safe-wallet-front-end-code-tampered-84b78f0fa9c2)
- [NCC Group — Bybit hack: in-depth technical analysis](https://www.nccgroup.com/research/in-depth-technical-analysis-of-the-bybit-hack/)
- [Sygnia — Investigation into the Bybit hack](https://www.sygnia.co/blog/sygnia-investigation-bybit-hack/)
- [BlockSec — A web2 breach enables the largest crypto hack in history](https://blocksec.com/blog/bybit-incident-a-web2-breach-enables-the-largest-crypto-hack-in-history)
