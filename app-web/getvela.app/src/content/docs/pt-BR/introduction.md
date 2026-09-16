---
title: Introdução
description: O que é a Vela, para quem ela é e as ideias por trás de uma carteira inteligente autocustodiada sem frase-semente.
---

# Introdução

A Vela é uma **carteira inteligente autocustodiada** para redes EVM. As chaves são
suas, mas não existe frase-semente para anotar: você assina com uma passkey, usando
o rosto ou a digital.

Esta documentação cobre como começar, criar uma carteira, movimentar tokens e
entender o modelo de segurança por trás disso.

## A versão curta

- **Autocustódia.** Seus fundos são controlados por uma chave que só você consegue
  usar. A Vela (a empresa) não pode mover, congelar nem recuperar o seu dinheiro.
- **Sem frase-semente.** Sua chave de assinatura é uma passkey guardada no hardware
  seguro do seu aparelho. Não existem doze palavras para perder ou para alguém
  roubar por phishing.
- **Uma conta inteligente Safe.** Cada carteira é um contrato
  [Safe](https://github.com/safe-fndn/safe-smart-account), operado com abstração de
  contas ERC-4337 — é exatamente isso que deixa você assinar com uma passkey e ler
  cada transação antes de aprovar.
- **12 redes, um endereço só.** Ethereum, BNB Chain, Polygon, Arbitrum, Optimism,
  Base, Avalanche, Gnosis, Unichain, Tempo, Monad e World Chain — mais as redes que
  você adicionar — todas no mesmo endereço.
- **Assinatura legível.** Onde existe um descritor, a transação é traduzida em uma
  intenção legível (ERC-7730); onde não existe, a Vela cai numa decodificação de
  melhor esforço e avisa. As chamadas que ela não consegue ler são sinalizadas, não
  escondidas.
- **Código aberto.** A carteira e todos os seus serviços estão
  [públicos no GitHub](https://github.com/mondaylabsltd/vela-wallet), para qualquer
  um conferir o que eles fazem.
- **Software em alfa.** A Vela funciona e já guarda dinheiro de verdade, mas não
  tem anos de produção nas costas. Comece com valores pequenos. O
  [post sobre a alfa](/blog/vela-is-in-alpha) explica o que isso significa.

## Para quem é

A Vela é para quem quer autocustódia de verdade sem a armadilha de administrar uma
frase-semente — e para quem já se queimou com isso. Se você sabe desbloquear seu
celular, sabe usar a Vela.

## Para onde ir depois

- [Instalar a Vela](/pt-BR/docs/install) — roda no navegador, não há nada para
  baixar.
- [Crie sua carteira](/pt-BR/docs/create-wallet) — sua primeira carteira em cerca
  de um minuto.
- [Como as passkeys funcionam](/pt-BR/docs/passkeys) — o modelo de segurança,
  explicado direto.
- [Whitepaper](/pt-BR/docs/whitepaper) — a arquitetura completa e o modelo de
  confiança.

Se o *porquê* te interessa mais que o *como*, o [blog](/blog) conta como a Vela
está sendo construída.
