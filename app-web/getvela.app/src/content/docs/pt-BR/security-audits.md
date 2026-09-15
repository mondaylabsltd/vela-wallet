---
title: Auditorias e problemas conhecidos
description: Cada contrato on-chain de que a Vela depende, quem auditou, se a versão auditada bate com a que está implantada, e o que não é auditado de jeito nenhum.
---

«Auditado» é uma afirmação sobre uma versão específica de um código específico,
então esta página não fica acenando a palavra: ela cita os relatórios exatos, os
endereços exatos de implantação e as diferenças entre versão auditada e implantada.
Ela também lista o que _não_ é auditado, porque essa lista pesa tanto quanto a
primeira.

Última revisão: agosto de 2026. Se encontrar um erro aqui, avise que a gente
corrige.

## O caminho do dinheiro

Quatro camadas de contrato podem tocar no seu dinheiro. As quatro são contratos de
terceiros com auditorias publicadas, e em cada caso o endereço implantado é a
implantação canônica oficial.

### Safe v1.4.1 — a própria conta

Sua carteira é um proxy
[Safe](https://github.com/safe-global/safe-smart-account): singleton SafeL2, fábrica
de proxies, handler de compatibilidade e MultiSend para lotes.

[A Ackee Blockchain auditou o Safe v1.4.0](https://github.com/safe-global/safe-smart-account/blob/main/docs/audit_1_4_0.md)
(relatório final de março de 2023): 11 achados, nenhum crítico ou alto. O v1.4.1 que
implantamos difere do v1.4.0 auditado por uma correção de compatibilidade ERC-4337
de uma única linha
([PR #572](https://github.com/safe-global/safe-smart-account/pull/572)). A lógica do
MultiSend não mudou desde o
[v1.3.0 auditado pelo G0 Group](https://github.com/safe-global/safe-smart-account/tree/main/docs).
Todos os endereços batem com as implantações canônicas de
[safe-deployments](https://github.com/safe-global/safe-deployments), e os contratos
estão no escopo do
[bug bounty da Safe Foundation](https://docs.safefoundation.org/security/bug-bounty)
(até US$ 1.000.000 para achados críticos).

Uma coisa que auditoria não cobre: o incidente da Bybit em 2025. Aquele ataque
comprometeu a pipeline de build do front-end oficial da Safe, não os contratos — a
[conclusão forense oficial](https://safefoundation.org/blog/safe-ecosystem-foundation-statement)
não achou vulnerabilidade nos contratos da Safe. A gente lê isso como uma lição
sobre a camada web e de operação, que é exatamente a camada em que você também
deveria nos examinar.

### Safe4337Module v0.3.0 — o adaptador ERC-4337

Implantado em `0x75cf11467937ce3F2f357CE24ffc3DBF8fD5c226`, o endereço canônico do
v0.3.0 (correspondência exata no Sourcify — o bytecode on-chain é o código
auditado).
[Auditado pela Ackee Blockchain](https://github.com/safe-global/safe-modules/blob/main/modules/4337/docs/v0.3.0/audit.md)
(relatório final de março de 2024), sem achados em aberto acima do nível
informativo. A combinação v0.3.0 + EntryPoint v0.7 + Safe ≥ 1.4.1 que usamos é
exatamente a configuração que a auditoria e as notas de versão descrevem.

O histórico do módulo inclui um problema divulgado: o v0.1.0 (2023) não assinava
`initCode` nem `paymasterAndData`, um vetor de griefing de gas. Foi
[corrigido no v0.2.0](https://safefoundation.org/blog/strengthening-security-addressing-the-incident-of-the-canonical-4337-module),
e o v0.1.0 nunca saiu das testnets. Usamos o v0.3.0, que herda a correção.

### SafeWebAuthnSharedSigner v0.2.1 — o assinante das passkeys

Implantado em `0x94a4F6affBd8975951142c3999aEAB7ecee555c2`, o endereço canônico do
v0.2.1 (o mesmo em todas as chains, via a fábrica de singletons da Safe).

O que «shared» quer dizer — e o que não quer: o que é compartilhado é a _implantação
do contrato_, do mesmo jeito que o singleton da Safe é compartilhado. Sua chave não
é. Cada Safe chama `configure()` por delegatecall e guarda a própria chave pública
P-256 no próprio storage. Uma instância do assinante representa exatamente uma
passkey por Safe, e o Safe de ninguém mais consegue usar a sua.

Aqui a versão importa. A auditoria do v0.2.0
[dizia explicitamente](https://github.com/safe-global/safe-modules/blob/main/modules/passkey/docs/v0.2.0/audit.md)
que o assinante compartilhado estava fora do escopo — o contrato ainda nem existia.
As auditorias que cobrem o que implantamos são as do v0.2.1: uma
[competição de auditoria da Hats Finance](https://github.com/safe-global/safe-modules/blob/main/modules/passkey/docs/v0.2.1/audit-competition-report-hats.md)
(junho–julho de 2024: zero altos, zero médios, três achados baixos — todos
corrigidos) mais uma
[revisão da Certora sobre o commit de release](https://github.com/safe-global/safe-modules/blob/main/modules/passkey/docs/v0.2.1/audit.md)
sem achados novos. Desde o lançamento não foi divulgada nenhuma vulnerabilidade em
nível de contrato; os contratos de passkey estão no escopo do bounty da Safe
Foundation.

A própria documentação da Safe recomenda combinar a propriedade por passkey com um
caminho de recuperação, em vez de tratar uma única credencial como a chave exclusiva
da conta. Como a Vela lida com isso está documentado em
[recuperação e login](/pt-BR/docs/recovery).

A verificação P-256 on-chain usa direto o precompilado RIP-7212, sem verificador de
reserva em Solidity. Antes de habilitar qualquer rede, o app testa o precompilado com
uma assinatura real e recusa a rede se a verificação falhar. Duas ressalvas
honestas: a especificação original do RIP-7212 tem falhas em casos extremos que o
[EIP-7951](https://eips.ethereum.org/EIPS/eip-7951) foi escrito para corrigir (elas
não afetam assinaturas WebAuthn bem formadas), e um teste não consegue pegar todas as
formas em que a implementação de uma chain poderia divergir em contextos de execução
incomuns.

### EntryPoint v0.7 — o ponto de entrada do ERC-4337

Implantado em `0x0000000071727De22E5E9d8BAf0edAc6f37da032`, a
[implantação canônica do v0.7.0](https://github.com/eth-infinitism/account-abstraction/releases/tag/v0.7.0).
[Auditado pela OpenZeppelin](https://www.openzeppelin.com/news/erc-4337-account-abstraction-incremental-audit)
(a pedido da Ethereum Foundation, janeiro de 2024): zero críticos, zero altos, cinco
achados médios, todos resolvidos — e o commit auditado é a versão implantada. O
EntryPoint v0.7.0 está no escopo do
[bug bounty do ERC-4337](https://docs.erc4337.io/community/bug-bounty) da Ethereum
Foundation (até US$ 250.000).

## Problemas conhecidos que acompanhamos

### O vetor de griefing do EntryPoint

Em fevereiro de 2026, pesquisadores de segurança da Trust Security
[divulgaram](https://erc4337.substack.com/p/improving-useroperation-execution)
um vetor de griefing e censura que afeta todo EntryPoint anterior ao v0.9, incluindo
o v0.7 que usamos. Quem interceptar uma UserOperation assinada antes de ela ser
minerada pode executá-la dentro de um frame de chamada que controla e forçar a
execução interna a reverter — a operação falha, mas o gas é cobrado assim mesmo. A
Ethereum Foundation pagou aos pesquisadores um bounty de US$ 50.000; classificou o
problema como vetor de censura/griefing, não de roubo de fundos, e ele nunca foi
explorado.

O que ele consegue: desperdiçar uma taxa e atrasar uma transação. O que ele não
consegue: roubar fundos ou forjar uma assinatura. A exposição da Vela é estreita
porque as UserOperations vão direto a um relay em vez de passar por um mempool
público — então há pouca oportunidade de interceptar uma — e o pior caso é limitado
pela taxa que você já aceitou. A correção existe só no EntryPoint v0.9 (novembro de
2025); o v0.7 em si não dá para corrigir. Esperamos migrar conforme a pilha ao redor
— em especial a linha do módulo 4337 da Safe — passar a suportar o v0.9, e vamos
anotar aqui quando isso acontecer.

## O que não é auditado

- **Os contratos da própria Vela.** Dois contratos pequenos que escrevemos,
  implantados na Gnosis: o
  [índice de chaves públicas de passkeys](https://github.com/atshelchin/webauthnp256-publickey-index.biubiu.tools)
  (um registro somente-acréscimo que ajuda seus aparelhos a achar sua chave pública)
  e o auxiliar de lotes dele. Não são auditados. Por construção, não guardam fundos,
  não têm dono e não podem ser atualizados — são uma camada de descoberta, não de
  autorização. O poder de gastar vem sempre da passkey configurada dentro do seu
  Safe. A pior falha realista é griefing (alguém ocupar uma entrada do índice), o que
  pode deixar a recuperação menos confortável mas não move dinheiro. Um contrato
  divisor para liquidação de gas, de um desenho de taxas anterior, não faz mais parte
  do fluxo de transação.
- **Multicall3.** O README dele
  [diz com todas as letras](https://github.com/mds1/multicall3): «This contract is
  unaudited.» Nós o usamos exatamente do jeito que os autores descrevem como seguro:
  chamadas de leitura em lote para saldos, metadados de tokens e cotações. A Vela
  nunca dá aprovações a ele e ele nunca guarda fundos. O pior caso de um bug é uma
  leitura incorreta.
- **O deployer CREATE2.** O
  [proxy de implantação determinística da Arachnid](https://github.com/Arachnid/deterministic-deployment-proxy)
  é o deployer sem estado padrão do ecossistema; não tem auditoria formal. Nossas
  checagens de rede falham pelo lado seguro se ele estiver faltando ou alterado numa
  chain.
- **Tempo e pathUSD.** A Tempo, uma das nossas doze redes integradas, não tem moeda
  nativa; lá o gas é liquidado na stablecoin pathUSD. Em agosto de 2026, nem o
  protocolo central da Tempo nem a pathUSD têm auditoria de segurança publicada ou
  bug bounty, e uma
  [avaliação independente de colateral da DefiLlama](https://artifacts.llama.fi/md-exports/pathusd-collateral-assessment-april2026-1776332825042.md)
  (abril de 2026) classificou a pathUSD como de alto risco. É risco em nível de
  chain, que nenhuma carteira consegue mitigar: os fundos que você tiver na Tempo, e
  a liquidação de gas lá, herdam isso. Trate a Tempo como a chain mais nova e menos
  testada da lista e dimensione seus saldos de acordo. Vamos atualizar esta seção
  conforme auditorias forem publicadas.
- **A própria Vela.** Nosso app e serviços de backend não passaram por auditoria de
  terceiros. Essa é a maior ressalva desta página, dizemos isso no cabeçalho do site,
  e os detalhes honestos estão em
  [A Vela está em alfa](/blog/vela-is-in-alpha). Comece com valores pequenos. Leia o
  código.

## Confira você mesmo

Todos os endereços acima são implantações públicas e canônicas que você pode
conferir nos registros oficiais —
[safe-deployments](https://github.com/safe-global/safe-deployments),
[safe-modules-deployments](https://github.com/safe-global/safe-modules-deployments)
e as
[notas de versão do EntryPoint](https://github.com/eth-infinitism/account-abstraction/releases/tag/v0.7.0):

| Contrato | Endereço |
| ----------------------------------- | -------------------------------------------- |
| SafeL2 singleton v1.4.1 | `0x29fcB43b46531BcA003ddC8FCB67FFE91900C762` |
| SafeProxyFactory v1.4.1 | `0x4e1DCf7AD4e460CfD30791CCC4F9c8a4f820ec67` |
| CompatibilityFallbackHandler v1.4.1 | `0xfd0732Dc9E303f09fCEf3a7388Ad10A83459Ec99` |
| MultiSend v1.4.1 | `0x38869bf66a61cF6bDB996A6aE40D5853Fd43B526` |
| SafeModuleSetup v0.3.0 | `0x2dd68b007B46fBe91B9A7c3EDa5A7a1063cB5b47` |
| Safe4337Module v0.3.0 | `0x75cf11467937ce3F2f357CE24ffc3DBF8fD5c226` |
| SafeWebAuthnSharedSigner v0.2.1 | `0x94a4F6affBd8975951142c3999aEAB7ecee555c2` |
| EntryPoint v0.7 | `0x0000000071727De22E5E9d8BAf0edAc6f37da032` |
| Multicall3 | `0xcA11bde05977b3631167028862bE2a173976CA11` |
| Índice de chaves públicas de passkeys (Gnosis) | `0xdd93420BD49baaBdFF4A363DdD300622Ae87E9c3` |
