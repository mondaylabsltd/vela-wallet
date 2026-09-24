---
title: Auditorias e problemas conhecidos
description: "Cada contrato de que a Vela depende, quem auditou qual versão, se a versão auditada é a que está implantada, os achados em aberto que acompanhamos e o que não foi auditado de forma alguma."
source: d0bb95c016da
---

“Auditado” é uma afirmação sobre um código específico numa versão específica, então
esta página cita os relatórios, os commits e os endereços implantados — e lista o
que **não** foi auditado, o que importa tanto quanto.

Última revisão: 22 de setembro de 2026. Se você encontrar um erro, avise e nós
corrigimos.

## O caminho dos fundos

Todo contrato que pode tocar no seu dinheiro é uma implantação canônica de código de
terceiros com revisões publicadas.

### Safe v1.4.1 — a própria conta

A sua carteira é um proxy [Safe](https://github.com/safe-fndn/safe-smart-account/tree/v1.4.1)
que usa o singleton SafeL2 e a SafeProxyFactory. Os lotes passam pelo MultiSend.

A [Ackee Blockchain auditou o Safe v1.4.0](https://github.com/safe-global/safe-smart-account/blob/main/docs/audit_1_4_0.md)
(relatório final em 16 de março de 2023, revisão das correções em 28 de março): 11
achados, nenhum crítico ou alto; os dois achados médios foram reconhecidos, e não
alterados. O escopo foi SafeL2, SafeProxyFactory, CompatibilityFallbackHandler,
MultiSendCallOnly e SignMessageLib. A v1.4.1 difere da v1.4.0 em uma linha
funcional, uma correção de compatibilidade com o ERC-4337 na configuração de módulos
([PR #572](https://github.com/safe-global/safe-smart-account/pull/572)); a Safe
consultou a Ackee e concluiu que não era necessária uma nova auditoria. A lógica do
MultiSend não mudou desde a v1.3.0, que a
[G0 Group auditou](https://github.com/safe-global/safe-smart-account/tree/main/docs).
Todos os endereços batem com o
[safe-deployments](https://github.com/safe-global/safe-deployments). Os contratos
principais estão no escopo do
[bug bounty da Safe Foundation](https://docs.safefoundation.org/security/bug-bounty),
cuja faixa mais alta paga até US$ 1.000.000.

O incidente da Bybit em 2025 não é um achado de contrato: os atacantes adulteraram o
JavaScript servido pela interface web da Safe, e a
[declaração forense](https://safefoundation.org/blog/safe-ecosystem-foundation-statement)
da Safe não encontrou nenhuma vulnerabilidade nos contratos.
[A nossa página sobre o caso](/pt-BR/docs/bybit-attack) explica por que o mesmo tipo
de ataque diz respeito a toda interface de carteira, inclusive a nossa.

### Safe4337Module v0.3.0 — o adaptador ERC-4337

Implantado em `0x75cf11467937ce3F2f357CE24ffc3DBF8fD5c226` (correspondência exata no
Sourcify) e também definido como o fallback handler do seu Safe. Revisado três vezes
— [relatórios aqui](https://github.com/safe-global/safe-modules/blob/main/modules/4337/docs/v0.3.0/audit.md):

- **Ackee Blockchain**, relatório final em março de 2024: um aviso (uso do otimizador
  do compilador) reconhecido, nada acima disso em aberto.
- **Certora**, agosto de 2026: um achado **médio**, reconhecido e **não corrigido**
  na v0.3.0 — *mudanças de autorização não invalidam UserOperations posteriores já
  validadas no mesmo bundle*. Veja “Problemas conhecidos” abaixo.
- **Nethermind**, agosto de 2026: nenhum achado.

O SafeModuleSetup v0.3.0 (`0x2dd6…5b47`), que habilita o módulo quando uma carteira
é implantada, foi coberto pelas revisões da Certora e da Nethermind.

O histórico do módulo tem um problema divulgado: a v0.1.0 não assinava `initCode` e
`paymasterAndData`, um vetor de gas griefing
[corrigido na v0.2.0](https://safefoundation.org/blog/strengthening-security-addressing-the-incident-of-the-canonical-4337-module);
a Safe informa que a v0.1.0 não foi usada fora de testnets. A Vela usa a v0.3.0 com
o EntryPoint v0.7 e o Safe 1.4.1, a configuração que a release do módulo descreve.

### Módulo de passkey da Safe v0.2.1 — os signatários

A sua primeira chave é verificada pelo **SafeWebAuthnSharedSigner** em
`0x94a4F6affBd8975951142c3999aEAB7ecee555c2`. “Compartilhado” quer dizer que a
implantação do contrato é compartilhada, como a do singleton do Safe; a sua chave,
não. Cada Safe guarda a própria chave pública P-256 no próprio storage.

Cada chave adicional tem o seu próprio contrato signatário, criado pela
**SafeWebAuthnSignerFactory** em `0x1d31F259eE307358a26dFb23EB365939E8641195`
como um proxy para o **singleton SafeWebAuthnSigner** em
`0x4E27b51350e6c2083EE19011120F50DAfEc5CA50`.

As revisões que cobrem esses contratos na v0.2.1
([relatórios](https://github.com/safe-global/safe-modules/blob/main/modules/passkey/docs/v0.2.1/audit.md)):

- Uma [competição de auditoria da Hats Finance](https://github.com/safe-global/safe-modules/blob/main/modules/passkey/docs/v0.2.1/audit-competition-report-hats.md)
  (junho–julho de 2024): nenhum achado alto ou médio; três baixos, todos corrigidos.
- A revisão da **Certora** sobre o commit da release: nenhum achado novo. (A
  auditoria anterior, da v0.2.0, registra que o signatário compartilhado ainda não
  tinha sido auditado — ele foi adicionado depois dessa auditoria.)
- **Nethermind**, agosto de 2026: nenhum achado.

Nenhuma vulnerabilidade de contrato foi divulgada desde a release, e os contratos de
passkey estão no escopo do bug bounty da Safe Foundation.

As assinaturas de passkey são verificadas pelo pré-compilado **EIP-7951 / RIP-7212** da rede,
sem verificador alternativo. Antes de habilitar uma rede, o app testa o
pré-compilado com uma assinatura real. Duas ressalvas: a especificação original do
RIP-7212 tem falhas em casos extremos que o
[EIP-7951](https://eips.ethereum.org/EIPS/eip-7951) corrige (elas só afetam
entradas que deveriam falhar de qualquer jeito, não assinaturas WebAuthn bem
formadas), e um teste não consegue detectar todas as formas pelas quais a
implementação de uma rede pode divergir.

### EntryPoint v0.7 — executa a sua operação

Implantado em `0x0000000071727De22E5E9d8BAf0edAc6f37da032`, a
[release canônica v0.7.0](https://github.com/eth-infinitism/account-abstraction/releases/tag/v0.7.0).
[Auditado pela OpenZeppelin](https://www.openzeppelin.com/news/erc-4337-account-abstraction-incremental-audit)
para a Ethereum Foundation (janeiro de 2024): nenhum achado crítico ou alto, cinco
médios, todos os 24 achados resolvidos; o commit da revisão das correções bate com a
release. Ele está no escopo do
[bug bounty do ERC-4337](https://docs.erc4337.io/community/bug-bounty) da Ethereum
Foundation (até US$ 250.000).

## Problemas conhecidos que acompanhamos

### Mudanças de autorização dentro de um bundle (Safe4337Module, Certora M-01)

O EntryPoint valida todas as operações de um bundle antes de executar qualquer uma
delas. Então, se uma operação remove um proprietário, uma operação assinada por esse
proprietário e colocada depois no mesmo bundle continua passando na validação e é
executada. A Safe reconheceu isso e não alterou a v0.3.0.

Os apps da Vela nunca montam trocas de proprietário, e um dApp que peça uma é
recusado de imediato, então a própria Vela nunca aciona esse problema. Ele ainda
importa para quem remove uma chave comprometida por outras ferramentas da Safe: essa
pessoa não poderia contar com o corte dela dentro do mesmo bundle.

### Interceptação de uma operação assinada (EntryPoint anterior à v0.9)

Em fevereiro de 2026, pesquisadores
[divulgaram](https://erc4337.substack.com/p/improving-useroperation-execution) um
vetor de griefing e censura que afeta todos os EntryPoints anteriores à v0.9,
inclusive a v0.7. Alguém que obtenha uma operação assinada antes de ela ser minerada
pode executá-la dentro de uma chamada que controla e forçar a execução interna a
reverter: a operação falha e precisa ser assinada de novo. (Com a taxa embutida da
Vela, a transferência da taxa reverte junto, então quem absorve o gas é o relay, e
não você.) Isso afeta operações que chamam contratos com proteção contra
reentrância ou que podem ser levadas a reverter por um estado temporário;
transferências simples não são afetadas. Usado repetidamente contra fluxos de saque,
poderia deixar fundos indisponíveis por um tempo. Não permite falsificar uma
assinatura nem redirecionar fundos.

O relay da Vela envia as operações diretamente, e não por uma mempool
compartilhada, mas uma transação `handleOps` pendente continua visível na mempool
pública, então isso reduz a exposição em vez de eliminá-la. A correção só existe no
EntryPoint v0.9 (novembro de 2025); a v0.7 não tem como ser corrigida. A migração
depende de o módulo 4337 da Safe suportar a v0.9, e esta página vai avisar quando
isso acontecer.

### Lacunas nas defesas da própria Vela

Não são achados de contrato, mas pontos em que a carteira protege você menos do que
você talvez imagine. Cada um está sendo acompanhado para correção:

- **A proteção de aprovações só barra valores “ilimitados”** (2^200 ou mais; 2^152
  no Permit2). Uma aprovação finita alta, um permit assinado ou um
  `setApprovalForAll` de NFT recebem um alerta, não um bloqueio.
- **A página de assinatura independente não está conectada** a nenhum app ainda.
- **O site carrega um script de análise de terceiros** no mesmo domínio das
  passkeys. O site proíbe que as suas páginas usem passkeys (um cabeçalho
  Permissions-Policy) e mantém o script fora da página que guarda uma chave.

## O que não é auditado

- **Os contratos da própria Vela.** O
  [registro de chaves públicas](https://github.com/mondaylabsltd/p256-index/tree/main/contracts)
  em `0x94fD1A891EB6c5F340622Baf2F3A0cb70A941EA9` (Gnosis; o mesmo endereço no
  Ethereum e na Base), a implantação original do registro em
  `0x5266DfF591B9F9EecfEdb8E7EfEf6c687854edaf` (o endereço dela faz parte do domínio
  de assinatura de todo registro) e o índice anterior que eles substituíram
  (`0xdd93420BD49baaBdFF4A363DdD300622Ae87E9c3`, histórico somente leitura). Eles não
  são auditados. Não guardam fundos, não têm dono e não podem ser atualizados; são
  uma camada de descoberta, não uma camada de autorização. O poder de gasto vem
  apenas das chaves configuradas no seu Safe. A pior falha realista é uma carteira
  ficar mais difícil de encontrar num aparelho novo, não dinheiro se mover.
- **Multicall3.** O README dele
  [diz](https://github.com/mds1/multicall3) “This contract is unaudited.” (“Este
  contrato não é auditado.”) A Vela o usa só para leituras em lote — saldos, dados de
  tokens, cotações de preço —, nunca com aprovações ou fundos.
- **Os implantadores determinísticos** (o proxy CREATE2 da Arachnid e a singleton
  factory da Safe) — padrão do ecossistema e sem estado, sem auditorias formais. A
  verificação de rede da Vela falha de forma segura se eles estiverem ausentes; ela
  confere se existe código no endereço, não se ele corresponde byte a byte.
- **Tempo.** Uma das 24 redes integradas, sem moeda nativa; ali, a Vela paga o gas
  na stablecoin pathUSD. Em setembro de 2026, a
  [política de segurança](https://github.com/tempoxyz/.github/blob/main/SECURITY.md)
  da Tempo diz que o protocolo ainda está passando por auditoria e não tem bug bounty
  ativo. Fundos mantidos na Tempo, e o gas pago ali, carregam esse risco de rede;
  trate-a como a rede mais nova e menos comprovada da lista.
- **A própria Vela.** Os apps, os serviços de backend e os contratos acima não
  passaram por auditoria de terceiros, e nenhuma está agendada. Essa é a maior
  ressalva desta página. Os detalhes estão em
  [Vela is in alpha](/blog/vela-is-in-alpha). Comece com valores pequenos e leia o
  código.

## Confira você mesmo

Todos os endereços abaixo são implantações públicas canônicas. Confira-os com o
[safe-deployments](https://github.com/safe-global/safe-deployments), o
[safe-modules-deployments](https://github.com/safe-global/safe-modules-deployments)
e a [release do EntryPoint](https://github.com/eth-infinitism/account-abstraction/releases/tag/v0.7.0):

| Contrato                                         | Endereço                                     |
| ------------------------------------------------ | -------------------------------------------- |
| Singleton SafeL2 v1.4.1                          | `0x29fcB43b46531BcA003ddC8FCB67FFE91900C762` |
| SafeProxyFactory v1.4.1                          | `0x4e1DCf7AD4e460CfD30791CCC4F9c8a4f820ec67` |
| MultiSend v1.4.1                                 | `0x38869bf66a61cF6bDB996A6aE40D5853Fd43B526` |
| CompatibilityFallbackHandler v1.4.1 ¹            | `0xfd0732Dc9E303f09fCEf3a7388Ad10A83459Ec99` |
| SafeModuleSetup v0.3.0                           | `0x2dd68b007B46fBe91B9A7c3EDa5A7a1063cB5b47` |
| Safe4337Module v0.3.0                            | `0x75cf11467937ce3F2f357CE24ffc3DBF8fD5c226` |
| SafeWebAuthnSharedSigner v0.2.1                  | `0x94a4F6affBd8975951142c3999aEAB7ecee555c2` |
| SafeWebAuthnSignerFactory v0.2.1                 | `0x1d31F259eE307358a26dFb23EB365939E8641195` |
| Singleton SafeWebAuthnSigner v0.2.1              | `0x4E27b51350e6c2083EE19011120F50DAfEc5CA50` |
| EntryPoint v0.7                                  | `0x0000000071727De22E5E9d8BAf0edAc6f37da032` |
| Multicall3                                       | `0xcA11bde05977b3631167028862bE2a173976CA11` |
| Registro de chaves públicas (Vela, não auditado) | `0x94fD1A891EB6c5F340622Baf2F3A0cb70A941EA9` |

¹ Verificado quando uma rede é adicionada; o seu Safe usa o módulo 4337 como
fallback handler no lugar dele.
