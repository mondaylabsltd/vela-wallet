---
title: O contrato da conta
description: "A sua carteira Vela é um Safe v1.4.1 sem modificações. Nenhum contrato no caminho até o seu dinheiro foi escrito pela Vela — aqui estão exatamente quais contratos são, o que isso garante e quanto custa."
source: 588a6ba6e672
---

# O contrato da conta

A sua carteira não é uma estrutura de dados privada de um app. Ela é uma conta
inteligente **Safe v1.4.1** — o contrato que muitas grandes tesourarias on-chain
usam —, implantada exatamente como a Safe publica, sem nenhuma modificação.

## Nada no caminho é nosso

Todo contrato que pode tocar no seu dinheiro foi escrito pela Safe ou pelos autores
do ERC-4337:

| Contrato | Papel na sua carteira | Escrito por |
| --- | --- | --- |
| [Safe v1.4.1](https://github.com/safe-fndn/safe-smart-account/tree/v1.4.1) (SafeL2, por meio de um proxy) | A própria conta; proprietários, limite, execução | Safe |
| [Módulo 4337 da Safe v0.3.0](https://github.com/safe-global/safe-modules/tree/4337/v0.3.0/modules/4337) | Permite que o EntryPoint opere o Safe; também é o fallback handler dele | Safe |
| [SafeWebAuthnSharedSigner v0.2.1](https://github.com/safe-global/safe-modules/tree/passkey/v0.2.1/modules/passkey) | Verifica as assinaturas P-256 da primeira chave | Safe |
| [SafeWebAuthnSignerFactory v0.2.1](https://github.com/safe-global/safe-modules/tree/passkey/v0.2.1/modules/passkey) e os signatários que ela cria | Um pequeno contrato signatário para cada chave adicional | Safe |
| [ERC-4337 EntryPoint v0.7](https://github.com/eth-infinitism/account-abstraction/releases/tag/v0.7.0) | Executa a sua operação assinada | Autores do ERC-4337 |

Os contratos da própria Vela não estão nesta lista: o **registro de chaves
públicas**, que grava as chaves de cada carteira para que um aparelho novo consiga
encontrá-la ([recuperação](/pt-BR/docs/recovery)), o registro de domínios que o
acompanha e o índice anterior que eles substituíram. Eles não guardam fundos e não
têm nenhum papel no seu Safe.

O repositório da carteira não contém nenhuma linha de Solidity — dá para conferir
com um comando:

```bash
git clone https://github.com/mondaylabsltd/vela-wallet
find vela-wallet -name '*.sol'   # não imprime nada
```

A Vela não tem nenhum papel privilegiado na sua conta: nenhuma chave de
administrador, nenhum caminho de atualização, nenhum módulo que ela possa
adicionar. Só as suas chaves podem alterar o seu Safe.

## Por que “sem modificações” é a expressão que importa

Muitas carteiras são construídas sobre “um Safe”, um fork do Safe ou uma conta
inspirada no Safe. A diferença importa de três formas.

**As auditorias valem para o que você está usando de fato.** As auditorias da Safe
cobrem essas versões, ou versões anteriores das quais elas diferem só por pequenas
mudanças documentadas (a [página de auditorias](/pt-BR/docs/security-audits) traz
os detalhes). As auditorias de um fork cobrem o código antes do fork; a modificação
é a parte que ninguém auditou.

**O ecossistema trata a sua conta como um Safe, porque ela é um.** Exploradores de
blocos a decodificam, e as ferramentas da Safe conseguem lê-la e montar transações
para ela. Para *assinar* essas transações, porém, um programa precisa conseguir
pedir à sua chave uma assinatura para `getvela.app`, o domínio ao qual as suas
passkeys pertencem — então o próprio app web da Safe, servido de outro domínio, não
consegue assinar por você. O
[guia de auto-hospedagem](/pt-BR/docs/self-hosting#if-getvela-app-disappears) lista
o que consegue.

**A superfície de ataque é uma que muitos outros também estão vigiando.** Um
contrato de conta feito sob medida é vigiado quase só pelo autor. Os contratos
principais do Safe são vigiados por todo mundo que guarda dinheiro num Safe; os
módulos 4337 e de passkey têm um público menor, mas real.

## Quanto custa

Ser padrão tem um preço:

- **Gas.** A sua assinatura é verificada on-chain e a transação passa pelo
  EntryPoint. Um envio simples de uma carteira Vela já implantada usou cerca de
  140.000–170.000 unidades de gas on-chain nas nossas medições na Gnosis (setembro de 2026);
  uma transferência simples de ETH de uma conta comum usa 21.000. Além do gas, o
  relay cobra a taxa dele — veja [redes e taxas](/pt-BR/docs/networks-and-fees).
- **A conta precisa ser implantada.** O seu endereço é calculado com `CREATE2`
  antes de existir qualquer coisa on-chain, então você pode receber nele na hora; a
  sua primeira transação de saída em cada rede paga a implantação do contrato.
- **Nem toda rede se qualifica.** As assinaturas de passkey são verificadas com o
  pré-compilado **RIP-7212**, e o endereço dele faz parte dos dados de configuração
  de toda carteira, então uma rede sem ele simplesmente não consegue rodar a Vela.
- **O risco da Safe passa a ser o seu risco.** Confiar num contrato amplamente usado
  continua sendo confiar num contrato. A Vela não acrescentou um segundo contrato
  próprio no caminho até o seu dinheiro para você ter que confiar.

## O que é e o que não é auditado

Os contratos da Safe, os módulos 4337 e de passkey dela e o EntryPoint v0.7 têm
auditorias de terceiros publicadas. **O código da própria Vela — os apps, os
serviços de backend e o contrato de registro — não passou por auditoria de
terceiros, e nenhuma está agendada**; é um objetivo para quando o projeto puder
pagar por uma, não um compromisso com data. Cada contrato, o relatório de auditoria
dele e os problemas que acompanhamos estão em
[auditorias e problemas conhecidos](/pt-BR/docs/security-audits).

## Confira você mesmo

A sua conta está on-chain. Abra o seu endereço num explorador de blocos depois que
ela for implantada: é um proxy Safe cuja implementação é a implantação canônica do
SafeL2 v1.4.1 da Safe, em todas as redes.

A seguir: [auditorias e problemas conhecidos](/pt-BR/docs/security-audits).
