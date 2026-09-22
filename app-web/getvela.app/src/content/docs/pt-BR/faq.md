---
title: Perguntas frequentes
description: "Respostas curtas sobre custódia, chaves, recuperação, redes, taxas, o que a Vela consegue ver, código aberto e o que acontece se a Vela deixar de existir."
source: 2d38e6a0b6d9
---

# Perguntas frequentes

## A Vela é de autocustódia?

Sim. A sua carteira é uma conta inteligente Safe controlada apenas pelas suas
chaves, que ficam nos seus aparelhos, no seu gerenciador de senhas ou nas suas
chaves de segurança. A Vela não tem nenhuma chave nem papel nela, então não consegue
mover, congelar nem recuperar os seus fundos por conta própria. Mas é ela que escreve
o software que pede às suas chaves para assinar — veja o
[modelo de ameaças](/pt-BR/docs/whitepaper).

## Não existe mesmo frase de recuperação?

Não existe mesmo. As suas chaves são passkeys, e uma passkey não tem nenhum segredo
que dê para anotar ou digitar. Veja [como as passkeys funcionam](/pt-BR/docs/passkeys).

## Do que eu preciso para criar uma carteira?

Um aparelho compatível com passkeys (um celular ou computador recente com Face ID,
digital ou Windows Hello), ou duas chaves de segurança físicas. Sem e-mail, sem
cadastro, sem saldo inicial. Você pode criar a carteira com até sete chaves; não dá
para adicionar outras depois. Veja [criar sua carteira](/pt-BR/docs/create-wallet).

## O que acontece se eu perder o celular?

Faça login num aparelho novo com qualquer outra chave: a mesma passkey sincronizada
pelas Chaves do iCloud ou pelo Gerenciador de senhas do Google, outro celular ou a
sua chave de segurança. Se o celular tinha a sua única chave e ela não estava
sincronizada, a carteira não pode ser recuperada. Veja
[recuperação e login](/pt-BR/docs/recovery).

## Quais redes e tokens são compatíveis?

24 redes EVM integradas, entre elas Ethereum, Base, Arbitrum, Optimism, Polygon, BNB
Chain, Gnosis e Avalanche, além de qualquer rede EVM que você adicionar e que atenda
aos requisitos. Moedas nativas e tokens ERC-20. O endereço é o mesmo em todas as
redes. Veja [redes e taxas](/pt-BR/docs/networks-and-fees).

## Quanto custa?

- **Os apps:** a carteira web, a extensão de navegador e os apps de desktop são
  gratuitos. Os apps de iOS e Android vão ser uma compra única nas lojas; você também
  pode compilar qualquer app a partir do código-fonte, de graça.
- **Cada transação:** uma taxa paga da sua carteira ao relay que a envia — o da
  Vela, a menos que você aponte a carteira para outro relay ou rode o seu. Ela cobre
  o gas mais a margem do relay, com mínimo de cerca de US$ 0,01. O valor exato
  aparece na tela de confirmação antes de você assinar e faz parte do que você
  assina. Não há depósito nem assinatura mensal.
  [Como a taxa é calculada](/pt-BR/docs/networks-and-fees#fee).
- **Nenhum token.** A Vela não tem nem planeja ter.

## Dá para usar a Vela com dApps?

Sim, pela extensão da Vela para navegador (Chrome, Edge, Brave) e pelo navegador
embutido nos apps de desktop (macOS, Windows), iOS e Android. A carteira web em
wallet.getvela.app não se conecta a dApps. Veja [instalar](/pt-BR/docs/install#dapps).

## O que a Vela consegue ver ou fazer?

A Vela não consegue ler as suas chaves nem mover os seus fundos por conta própria.
Os serviços dela veem o seu endereço IP e o que o app pede a eles: o índice vê as
suas chaves públicas e o nome da carteira quando registra uma carteira nova, e os
endereços que você consulta; o relay vê o seu endereço, as operações que você envia e
o endpoint RPC que o seu app usa; o serviço de dados de chain vê sobre quais tokens e
contratos o seu app pergunta. O que se torna público on-chain está listado em
[criar sua carteira](/pt-BR/docs/create-wallet#what-is-public). A
[política de privacidade](/privacy) é a versão completa e oficial.

## A Vela é de código aberto?

Os apps da carteira, o relay e o serviço de cotações têm licença MIT e estão no
[GitHub](https://github.com/orgs/mondaylabsltd/repositories); o diretório de dados
de chain também é MIT. O índice de chaves públicas é público, mas ainda não tem
arquivo de licença. Você mesmo pode rodar cada serviço — veja o
[guia de auto-hospedagem](/pt-BR/docs/self-hosting).

## A Vela é auditada?

Os contratos onde o seu dinheiro fica — o Safe e os módulos dele, e o EntryPoint do
ERC-4337 — são auditados. O código da própria Vela não é, e nenhuma auditoria está
agendada. Veja [auditorias e problemas conhecidos](/pt-BR/docs/security-audits).

## E se a Vela fechar?

Os seus fundos continuam no seu Safe, on-chain. Para uma carteira existente, a
extensão da Vela para navegador e os apps que você mesmo compilar continuam
funcionando sem o getvela.app, e todos os serviços são de código aberto, para que
outra pessoa possa rodá-los. O
[guia de auto-hospedagem](/pt-BR/docs/self-hosting#if-getvela-app-disappears) lista
os caminhos e os seus limites.

## Tenho uma pergunta que não está aqui.

Abra uma issue no [GitHub](https://github.com/mondaylabsltd/vela-wallet/issues) ou
fale com a gente no [X](https://x.com/realvelawallet) ou no
[Telegram](https://t.me/velawallet).
