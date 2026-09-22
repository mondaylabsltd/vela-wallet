---
title: Hospedar a página de assinatura
description: "A página sem dependências e a extensão do Chrome que decodificam uma transação por conta própria e a assinam com a sua passkey — como rodar a sua própria cópia, e qual cópia consegue assinar pela sua carteira."
source: c81389ef7aa2
---

# Hospedar a página de assinatura

A Vela decodifica cada transação antes de você aprová-la, e essa decodificação é um
trabalho honesto — mas é um trabalho feito pelo mesmo app que montou a transação. Se
o app, ou o caminho pelo qual ele chega até você, for adulterado, ele pode mostrar
uma coisa e assinar outra. Foi exatamente o que aconteceu com a
[Bybit](/pt-BR/docs/bybit-attack).

A página de assinatura existe para dividir isso em dois: a transação vem de um lugar,
e a conferência e a assinatura acontecem num lugar que você controla.

**Situação:** pronta e testada localmente; **não publicada**, e **nenhum app da Vela
envia solicitações para ela ainda**. Hoje, ela é algo para ler, rodar e experimentar
com os solicitantes de demonstração da pasta `samples/`. Usá-la para assinaturas de
verdade exige que os apps encaminhem suas solicitações para ela, e isso ainda precisa
ser construído.

## O que é

Uma pasta — `app-web/clearsigning` no repositório — que é ao mesmo tempo uma página
web e uma extensão do Chrome. HTML, CSS e JavaScript puros: nenhum framework, nenhum
bundler, nenhuma etapa de build, nenhuma dependência e nenhum dado buscado de um
servidor — a única requisição que ela faz é pelos logos decorativos dos tokens.

Diante de uma solicitação de assinatura, ela não confia no resumo que veio junto. Ela
decodifica a calldata crua por conta própria, calcula o próprio digest, mostra o que
a assinatura vai autorizar de fato e só então chama a sua passkey.

Como não há etapa de build, os arquivos que você lê são os arquivos que rodam. Você
pode comparar a pasta com o repositório e saber exatamente o que está servindo.

## Qual cópia consegue assinar pela sua carteira

Uma passkey fica vinculada ao domínio em que foi criada. As suas chaves da Vela são
registradas sob `getvela.app`, e um navegador só as oferece a uma página cuja
relying party (a parte confiável, no vocabulário do WebAuthn) seja `getvela.app`.
Essa única regra decide qual forma de rodar a sua própria cópia é útil para você.

**Como extensão do Chrome — a forma de usá-la com as chaves que você já tem.** A
relying party da extensão é `getvela.app`, não importa de onde a pasta veio, então as
suas chaves existentes conseguem assinar nela, enquanto o código é a pasta que você
carregou e inspecionou.

1. Obtenha a pasta: `git clone https://github.com/mondaylabsltd/vela-wallet`
   (ela está em `app-web/clearsigning`).
2. Abra `chrome://extensions` e ative o **Modo do desenvolvedor**.
3. Clique em **Carregar sem compactação** e escolha a pasta `app-web/clearsigning`.
4. O ícone na barra de ferramentas abre a página numa aba.

**Como página no seu próprio domínio, ou no localhost.** Servida por HTTPS (ou a
partir do localhost), a relying party da página é o próprio hostname dela — então
ela consegue assinar com chaves registradas sob _aquele_ hostname, não com chaves
registradas sob `getvela.app`. Isso a torna a forma certa de experimentar toda a
cerimônia de ponta a ponta, de rodar o fluxo de desktop e de assinar por uma carteira
cuja chave foi criada no seu próprio domínio. Não é uma forma de assinar por uma
carteira existente do `getvela.app`.

```sh
cd app-web/clearsigning
python3 -m http.server 8080   # → http://localhost:8080
```

Todos os caminhos no app são relativos, então um subdiretório num host existente
também funciona, e abrir o `index.html` direto do disco (`file://`) serve para dar
uma olhada — sem origem, não há relying party e nada pode ser assinado.

## O que ela faz antes de assinar

- **Ela mesma decodifica a transação.** O que a chamada faz, para quem e de quanto,
  a partir da calldata — inclusive chamadas aninhadas dentro de um lote.
- **Ela só assina um digest que ela mesma calculou.** Os digests EIP-191, EIP-712,
  SafeOp e SafeMessage são calculados na página e conferidos com o `vela-core`, o
  mesmo código que a carteira usa. Um digest que ela não consegue calcular é uma
  recusa, não uma assinatura.
- **Ela confere se a transação é a que foi solicitada.** A chamada que o site pediu
  precisa estar de fato dentro da operação que está sendo assinada.
- **Ela recusa uma aprovação no nível “ilimitado”.** Não é um aviso — é uma recusa,
  com uma indicação do que fazer no lugar.
- **Ela diz quando não consegue ler algo,** em vez de mostrar um resumo amigável que
  não pode sustentar.
- **Ela mostra o endereço e o identicon da conta,** e não mostra um nome de
  destinatário fornecido por quem pediu a assinatura. Tudo o que o solicitante
  controla é descartado ou identificado como vindo dele.

## O que ela deliberadamente não tem

- **Nenhum editor.** A solicitação fica fixa quando chega: você assina ou não
  assina. Um seletor de taxa ou um editor de limite reescreveriam a calldata, que é
  justamente a doença que esta página existe para evitar.
- **Nenhuma criação de chave.** A página de assinatura não consegue criar uma
  passkey. Criar uma seria criar outra conta.
- **Nenhum dado da rede.** Nada do que ela mostra ou assina é buscado. A única coisa
  que ela carrega são os logos dos tokens, como imagens, do servidor de dados de chain
  da Vela; se falharem, uma letra ocupa o lugar deles.

## Como uma solicitação chega até ela

| Solicitante                                   | Canal                                                                      |
| --------------------------------------------- | -------------------------------------------------------------------------- |
| Uma página no mesmo navegador                 | `postMessage`                                                              |
| Uma página no mesmo navegador, para a extensão | Porta da extensão                                                         |
| Um app de desktop na mesma máquina            | Fragmento de URL + callback de loopback (demonstração em `samples/`; o app de desktop da Vela ainda não usa isso) |
| Um celular ou outro computador                | Bluetooth LE (protocolo implementado; o rádio ainda não foi testado em hardware real) |

O formato das mensagens, os digests e uma tabela de onde vem cada item da tela estão
em `PROTOCOL.md`, ao lado do código.

## Onde ela se encaixa

Quando os apps puderem entregar suas solicitações a ela, o uso previsto é simples: a
partir do dia em que a conta guardar um dinheiro que você não gostaria de perder,
toda assinatura passa por uma página cujo código você mesmo carregou. Não só para
valores altos — uma aprovação pequena pode entregar o suficiente para esvaziar uma
conta. Até lá, a página é uma forma de ler e testar exatamente como essa segunda
opinião vai funcionar.
