---
title: Perguntas frequentes
description: Dúvidas comuns sobre a Vela — custódia, passkeys, contas inteligentes, recuperação, redes suportadas, taxas e privacidade.
---

# Perguntas frequentes

## A Vela é autocustodiada?

É. Sua carteira é uma conta inteligente controlada por uma chave que só você
consegue usar, guardada pelo sistema operacional do seu aparelho e que a Vela nunca
vê. A Vela não pode mover, congelar nem recuperar seus fundos.

## Minha carteira é uma conta normal ou um contrato?

É uma **conta inteligente Safe** (um contrato), operada com abstração de contas
ERC-4337. É isso que te permite assinar com uma passkey, ler cada transação antes de
aprovar e usar o mesmo endereço em todas as redes. A arquitetura está no
[whitepaper](/pt-BR/docs/whitepaper).

## Realmente não existe frase-semente?

Realmente. Sua chave de assinatura é uma passkey guardada pelo sistema operacional
do seu aparelho, e a Vela nunca a vê. Não há doze palavras para anotar, perder ou
ter roubadas por phishing. Por que isso é seguro:
[como as passkeys funcionam](/pt-BR/docs/passkeys).

## E se eu perder o celular?

Se a sua passkey é sincronizada pelo Chaveiro do iCloud ou pelo Gerenciador de
senhas do Google, você entra em um aparelho novo com a mesma conta e sua carteira
volta. O modelo completo e seus limites estão em
[recuperação e login](/pt-BR/docs/recovery).

## Quais redes e tokens são suportados?

A Vela vem com **12 redes EVM** — Ethereum, BNB Chain, Polygon, Arbitrum, Optimism,
Base, Avalanche, Gnosis, Unichain, Tempo, Monad e World Chain — mais redes
personalizadas, com tokens nativos e ERC-20. Seu endereço é o mesmo em todas. Veja
[redes e taxas](/pt-BR/docs/networks-and-fees).

## Quanto custa usar?

A carteira é gratuita e a Vela **não tem token**. Você paga o **gas** da rede do seu
próprio saldo, mais uma taxa do relay. O preço é cotado pelo relay e aparece **antes
de você assinar**, dividido em _taxa de rede / taxa do relay / total_: o custo exato
de cada transação está na tela de confirmação, e o valor cotado faz parte do que
você assina, então não muda depois. Transações muito baratas podem esbarrar numa
taxa mínima pequena. Na Tempo, que não tem moeda nativa, o gas é liquidado em
stablecoins em dólar. Cada rede também precisa de um pequeno **depósito não
reembolsável para ativar a conta de relay de gas** dela (a Vela cobre isso para
usuários novos quando dá); como essa conta pode se esgotar, talvez seja preciso
repor mais adiante — ou seja, não é exatamente um custo único. Detalhes em
[redes e taxas](/pt-BR/docs/networks-and-fees).

## O que a Vela (a empresa) consegue ver ou fazer?

A Vela guarda a chave **pública** da sua passkey e o **nome** que você escolheu,
para viabilizar o login entre aparelhos. Ela não vê sua chave privada, os saldos são
lidos de chains públicas e não há cadastro por e-mail. A versão que vale é a
[política de privacidade](/privacy).

## A Vela é de código aberto?

É — a carteira e seus quatro serviços de backend (dados de chain, índice de
passkeys, relay, taxas de câmbio) estão
[públicos no GitHub](https://github.com/mondaylabsltd/vela-wallet) sob licença MIT,
e você pode hospedá-los.

## Tenho uma pergunta que não está aqui.

Abra uma issue no [GitHub](https://github.com/mondaylabsltd/vela-wallet) ou fale com
a gente no [X](https://x.com/realvelawallet) ou no
[Telegram](https://t.me/velawallet).
