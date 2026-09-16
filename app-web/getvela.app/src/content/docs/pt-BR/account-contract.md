---
title: O contrato da conta
description: Sua carteira Vela é um Safe v1.4.1 sem modificações. Nada no caminho dos contratos foi escrito por nós — aqui está o que isso te dá e o que custa.
---

# O contrato da conta

Sua carteira não é a estrutura de dados privada de um app. É uma conta inteligente
**Safe v1.4.1** — o mesmo contrato usado para guardar tesourarias muito maiores do
que qualquer coisa que a Vela vá ver — implantada exatamente como a Safe publica,
sem nenhuma modificação.

A frase é curta e as consequências não são, então esta página detalha.

## Nada nesse caminho é nosso

Entre você e o seu dinheiro estão quatro contratos. A Vela não escreveu nenhum:

| Contrato | Quem escreveu |
| --- | --- |
| [Safe v1.4.1](https://github.com/safe-fndn/safe-smart-account/tree/release/v1.4.1) (a própria conta, um proxy) | Safe |
| [Safe 4337 Module](https://github.com/safe-global/safe-modules/tree/main/modules/4337) | Safe |
| [SafeWebAuthnSharedSigner](https://github.com/safe-global/safe-modules/tree/main/modules/passkey) (verifica sua chave P-256) | Safe |
| [ERC-4337 EntryPoint v0.7](https://eips.ethereum.org/EIPS/eip-4337) | Os autores do ERC-4337 |

Não existe um contrato da Vela. O repositório não contém nada de Solidity — dá para
conferir com um comando:

```bash
git clone https://github.com/mondaylabsltd/vela-wallet
find vela-wallet -name '*.sol'   # não imprime nada
```

Quando a Vela adiciona uma rede, ela implanta **aqueles** contratos, nos endereços
canônicos deles. Não implanta um contrato de desenho próprio e não tem nenhum papel
privilegiado no seu: sem chave de administrador, sem caminho de upgrade, sem módulo
que a gente possa acrescentar.

## Por que «sem modificações» é a palavra que importa

Muitas carteiras dizem ser construídas sobre «um Safe», «um fork do Safe» ou «uma
conta inspirada no Safe». Um fork é um contrato novo com reputação velha. Na
prática, as diferenças:

**As auditorias valem para o que você está usando de fato.** Os relatórios da Safe
cobrem o bytecode exatamente destas versões. As auditorias de um fork cobrem o
código anterior ao fork. Se uma carteira modificou o contrato da conta, toda
auditoria que ela cita é auditoria de outra coisa — e a modificação é justamente a
parte que ninguém olhou.

**O ecossistema trata sua conta como um Safe, porque ela é um.** Exploradores de
blocos a decodificam. As próprias ferramentas de transação da Safe a entendem. Se a
Vela sumir amanhã, sua carteira não vira um formato órfão: é a conta inteligente com
mais ferramentas em volta no Ethereum, e qualquer interface compatível com Safe
consegue operá-la. É isso que faz de
[«se a Vela sumir, sua carteira não»](/pt-BR/docs/why-vela) uma afirmação sobre
contratos, e não sobre nossas intenções.

**A superfície de ataque é uma que todo mundo também está vigiando.** Um contrato de
conta feito sob medida é vigiado só por quem o escreveu. Este aqui é vigiado por
todo mundo que guarda dinheiro em um Safe.

## O que custa

Ser padrão não sai de graça, e os trade-offs são reais:

- **Gas.** Uma conta inteligente verifica uma assinatura on-chain. Conte com algo em
  torno de 1,5 a 3 vezes o gas de uma transferência EOA simples, dependendo da
  chain. Veja [redes e taxas](/pt-BR/docs/networks-and-fees).
- **A conta precisa ser implantada.** Seu endereço é calculado com `CREATE2` antes
  de existir qualquer coisa on-chain, então você recebe na hora, mas a primeira
  transação de saída paga a implantação do contrato.
- **Nem toda chain se qualifica.** O assinante WebAuthn verifica uma assinatura
  P-256 on-chain, o que exige o precompilado **RIP-7212**. A Vela se recusa a
  habilitar uma rede sem ele em vez de cair para um verificador mais fraco.
- **O risco da Safe agora é seu risco.** Confiar num contrato amplamente usado
  continua sendo confiar num contrato. O que a Vela pode dizer é que não acrescentou
  por cima uma segunda coisa em que confiar.

## O que é auditado e o que não é

Os contratos da Safe e o módulo assinante WebAuthn são auditados por terceiros, e
esses relatórios são públicos. **O código do app da Vela não passou por auditoria
independente**, e não há nenhuma agendada — é uma meta para quando o projeto puder
bancar uma, não um compromisso com data. Cada contrato de que a Vela depende, seu
relatório de auditoria e os problemas que acompanhamos estão em
[auditorias e problemas conhecidos](/pt-BR/docs/security-audits).

## Confira você mesmo

Sua conta está on-chain. Abra em um explorador de blocos e leia o endereço da
implementação: será o deployment canônico da Safe na v1.4.1, byte a byte, em todas
as redes que a Vela suporta.
