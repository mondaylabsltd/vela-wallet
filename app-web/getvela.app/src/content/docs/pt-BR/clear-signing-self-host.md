---
title: Hospede você a página de assinatura
description: A página sem dependências e a extensão do Chrome que decodificam a transação por conta própria e a assinam com sua passkey — como rodar a sua cópia e qual cópia consegue assinar pela sua carteira.
---

# Hospede você a página de assinatura

A Vela decodifica cada transação antes de você aprovar, e esse trabalho é trabalho
de verdade — mas quem o faz é o mesmo app que montou a transação. Se o app, ou o
caminho por onde ele chega até você, for adulterado, ele pode mostrar uma coisa e
assinar outra. Foi exatamente isso que aconteceu com a
[Bybit](/pt-BR/docs/bybit-attack).

A página de assinatura existe para partir isso em dois: a transação vem de um lugar,
e a conferência e a assinatura acontecem em outro, que você controla.

## O que ela é

Uma pasta — `app-web/clearsigning` no repositório — que é ao mesmo tempo página web
e extensão do Chrome. HTML, CSS e JavaScript puros: sem framework, sem bundler, sem
etapa de build, sem dependências e sem requisições de rede próprias.

Ao receber um pedido de assinatura, ela não confia no resumo que veio junto. Ela
decodifica a calldata crua por conta própria, calcula o próprio digest, mostra o que
a assinatura vai realmente autorizar e só então pede a sua passkey.

Como não há etapa de build, os arquivos que você lê são os que rodam. Dá para
comparar a pasta com o repositório e saber o que você está servindo.

## Qual cópia consegue assinar pela sua carteira

Uma passkey fica presa ao domínio em que foi criada. Suas chaves da Vela estão
registradas sob `getvela.app`, e o navegador só as oferece a uma página cuja relying
party seja `getvela.app`. Essa única regra decide qual jeito de rodar a sua cópia te
serve.

**Como extensão do Chrome — é esta que se usa com a carteira que você já tem.** A
relying party da extensão é `getvela.app` independentemente de onde a pasta veio,
então suas chaves atuais conseguem assinar ali, enquanto o código é a pasta que você
carregou e inspecionou.

1. Abra `chrome://extensions` e ligue o **Modo do desenvolvedor**.
2. **Carregar sem compactação** e escolha a pasta `app-web/clearsigning`.
3. O ícone na barra abre a página em uma aba.

**Como página no seu próprio domínio, ou em localhost.** Servida por HTTP(S), a
relying party da página é o próprio hostname dela — então ela assina com chaves
registradas sob *aquele* hostname, não com chaves registradas sob `getvela.app`. É o
jeito certo de experimentar a cerimônia inteira de ponta a ponta, rodar o fluxo de
desktop e assinar por uma carteira cuja chave foi criada no seu próprio domínio. Não
é um jeito de assinar por uma carteira `getvela.app` existente.

```sh
cd app-web/clearsigning
python3 -m http.server 8080   # → http://localhost:8080
```

Todos os caminhos do app são relativos, então um subdiretório num host existente
também funciona; abrir `index.html` direto do disco (`file://`) serve para dar uma
olhada — sem origem não há relying party e nada pode ser assinado.

## O que ela faz antes de assinar

- **Decodifica a transação por conta própria.** O que a chamada faz, para quem e de
  quanto, a partir da calldata — inclusive chamadas aninhadas dentro de um lote.
- **Só assina um digest que ela mesma calculou.** Os digests EIP-191, EIP-712,
  SafeOp e SafeMessage são calculados na página e conferidos contra o `vela-core`, o
  mesmo código que a carteira usa. Um digest que ela não consegue calcular é uma
  recusa, não uma assinatura.
- **Confere que a transação é a que foi pedida.** A chamada que o site pediu precisa
  estar de fato dentro da operação que será assinada.
- **Recusa uma aprovação ilimitada.** Não é aviso — é recusa, com uma indicação do
  que fazer no lugar.
- **Diz quando não consegue ler algo,** em vez de mostrar um resumo simpático pelo
  qual não pode responder.
- **Mostra o endereço e o identicon da conta,** e não mostra nome de destinatário
  fornecido por quem pediu a assinatura. Tudo o que o solicitante controla ou é
  removido ou é rotulado como sendo dele.

## O que ela deliberadamente não tem

- **Nenhum editor.** O pedido fica fixo ao chegar: você assina ou não. Um seletor de
  taxas ou um editor de limite reescreveria a calldata, que é justamente a doença
  que esta página existe para evitar.
- **Nenhuma criação de chave.** A página de assinatura não cria passkey. Criar uma
  seria criar outra conta.
- **Nenhuma requisição de rede.** Se não há nada a buscar, não há nada a
  interceptar.

## Como um pedido chega até ela

| Quem pede | Canal |
| -------------------------------------------- | -------------------------------------------------------------------------- |
| Uma página no mesmo navegador | `postMessage` |
| Uma página do mesmo navegador, para a extensão | Porta da extensão |
| Um app de desktop na mesma máquina | Fragmento de URL + callback em loopback |
| Um celular ou outro computador | Bluetooth LE (protocolo implementado; o rádio ainda não foi testado em hardware real) |

O formato de troca, os digests e uma tabela de onde vem cada item da tela estão no
`PROTOCOL.md`, ao lado do código.

## Quando usar

A partir do dia em que a conta guarda dinheiro cuja perda te incomodaria — e daí em
diante, em toda assinatura. Não só para valores altos: uma aprovação pequena pode
entregar o suficiente para esvaziar uma conta. Um hábito de assinatura guardado para
ocasiões especiais não está de pé no dia em que faz falta.
