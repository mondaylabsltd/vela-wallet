---
title: Whitepaper
description: Como a Vela funciona e o que você precisa — e não precisa — aceitar como verdade para usá-la. Arquitetura, modelo de segurança, recuperação e como conferir tudo por conta própria.
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Whitepaper

<Callout type="info" title="Status: alfa · v0.1">
Esta página descreve como a Vela funciona hoje e o que você precisa ou não precisa
aceitar como verdade para usá-la. Ela prefere honestidade a marketing. A Vela está
em <a href="/blog/vela-is-in-alpha">alfa</a> — comece com valores pequenos. A Vela
não tem token. Tudo aqui é verificável contra o código aberto.
</Callout>

## Resumo

A Vela é uma **carteira de contrato inteligente autocustodiada** para redes EVM.
Cada carteira é uma conta inteligente
[Safe](https://github.com/safe-fndn/safe-smart-account) controlada por uma
**passkey** — uma credencial WebAuthn (P-256) guardada pelo sistema operacional do
seu aparelho, criptografada de ponta a ponta e destravada com Face ID, Touch ID ou
digital. Não há frases-semente nem chaves privadas para você copiar, guardar ou
perder.

A Vela, a empresa, nunca fica com suas chaves nem com seus fundos e **não pode
movê-los, congelá-los ou confiscá-los**. O app, o relay de transações e os serviços
de apoio são todos código aberto e auto-hospedáveis. O que você precisa confiar se
resume a contratos auditados, ao cofre de passkeys do seu sistema operacional e —
só para disponibilidade — a um relay que você pode trocar ou rodar você mesmo.

## Por que a Vela existe

A maioria das carteiras impõe uma troca:

- **Carteiras com frase-semente** colocam um segredo de 12 a 24 palavras na frente
  de cada usuário. É o ponto único de falha e um alvo constante de phishing.
- **Carteiras custodiais** tiram a frase-semente mas assumem a custódia dos seus
  fundos, trazendo de volta o risco de contraparte que a cripto deveria eliminar.
- **Assinatura às cegas** — aprovar hexadecimal que você não lê — virou norma no
  ecossistema inteiro e está por trás de boa parte das carteiras esvaziadas.

A Vela quer ser tão fácil quanto um app custodial mantendo você em autocustódia
completa: sem frase-semente, sem custódia alheia e sem transação que você não consiga
ler antes de assinar.

## Princípios de projeto

1. **Autocustódia, sem exceção.** As chaves nascem no seu aparelho e ficam com o
   serviço de passkeys do seu sistema, criptografadas de ponta a ponta. Os
   servidores da Vela só veem dados públicos.
2. **Verifique, não confie.** A pilha inteira — app e os quatro serviços de backend
   — é código aberto sob licença MIT.
3. **Nada de assinatura às cegas.** As transações são traduzidas em intenção legível
   onde existe descritor; chamadas desconhecidas são sinalizadas, não escondidas.
4. **Fazer menos.** A carteira guarda ETH e ERC-20 e se conecta às dApps que você
   escolher. Menos código em que confiar, superfície de ataque menor.

## Arquitetura

```text
App Vela (iOS / Android / Web, uma base de código só)
  • Passkey (WebAuthn P-256, serviço de passkeys do sistema)
  • Montagem e assinatura da UserOperation
  • Interface de assinatura legível (ERC-7730)
        │  UserOperation assinada
        ▼
Relay Vela (ERC-4337, auto-hospedável)
  • envia handleOps ao EntryPoint
  • não consegue alterar nem forjar sua transação
        ▼
Chain EVM
  EntryPoint v0.7 → conta inteligente Safe
  O assinante WebAuthn verifica P-256 on-chain
```

### Modelo de conta

Sua carteira é uma conta inteligente **Safe v1.4.1** (um contrato proxy) operada por
abstração de contas **ERC-4337** (EntryPoint v0.7), com o **Safe 4337 Module** e um
**assinante WebAuthn** como dono da conta.

O endereço é **determinístico** e **contrafactual**: é calculado a partir da chave
pública da sua passkey via `CREATE2` antes de qualquer transação ser enviada, então
você recebe fundos nele antes de ele ser implantado. A conta se implanta sozinha,
pagando do próprio saldo, na sua primeira transação.

### Chaves e autenticação

A autenticação usa **passkeys WebAuthn** na curva **P-256**. A chave privada é
gerada no seu aparelho e fica, criptografada de ponta a ponta, com o serviço de
passkeys do sistema (Chaveiro do iCloud ou Gerenciador de senhas do Google), que a
sincroniza entre seus aparelhos. **Os servidores da Vela só veem a sua chave
pública.** Assinar exige uma verificação biométrica nova a cada vez — não há chave de
sessão de vida longa. Todos os detalhes em
[como as passkeys funcionam](/pt-BR/docs/passkeys).

### Assinatura e fluxo da transação

1. **Montar** uma `UserOperation` ERC-4337 para o seu Safe e estimar o gas.
2. **Decodificar** a chamada em intenção legível e mostrar para conferência.
3. **Assinar** — depois da verificação biométrica, seu aparelho produz uma asserção
   WebAuthn sobre o hash da operação.
4. **Codificar** a asserção como assinatura de contrato **EIP-1271**.
5. **Repassar** a operação assinada ao relay, que a envia ao EntryPoint.
6. **Verificar on-chain** — o Safe confere a assinatura P-256 on-chain pelo
   precompilado RIP-7212 antes de executar. O precompilado é requisito duro: não há
   verificador de reserva, e a Vela se recusa a habilitar uma rede sem ele.

O relay recebe uma operação **já assinada**. Ele não pode mudar destinatário, valor
ou qualquer outro campo sem invalidar a assinatura.

### Relay e modelo de gas

- O gas é pago **do saldo da sua própria carteira** — no token nativo da rede por
  padrão, ou numa stablecoin suportada onde o relay oferecer. A Tempo, que não tem
  moeda nativa, sempre liquida gas em stablecoins em dólar. **Não há paymaster** nem
  terceiro patrocinando — ou barrando — suas transações.
- **O relay é a única fonte de verdade para o preço do gas.** Ele cota a partir das
  condições reais da chain; a carteira mostra essa cotação e assina exatamente o que
  mostra.
- A cobrança do relay da Vela é deliberadamente simples: o total é o **custo de rede
  mais a taxa de serviço do relay**, com uma taxa mínima pequena em transações muito
  baratas. Uma parte vai para os validadores da chain; o resto paga o relay que
  mantém a infraestrutura e mantém sua conta de gas com saldo.
- A carteira **mostra a taxa estimada antes de você confirmar** — no ativo da taxa e
  na sua moeda de exibição — e o valor cotado, junto com o destinatário, faz parte do
  que você assina, então o relay recebe exatamente o que foi mostrado. Sem margem
  escondida.
- Cada Safe tem uma **conta de relay dedicada** (conta de gas) por chain, ativada
  por um depósito **não reembolsável**. Ela pode se esgotar com o tempo e precisar de
  **nova ativação** depois — ou seja, não é exatamente um depósito único.

O relay é uma dependência de **disponibilidade**, não de **custódia**: ele pode
atrasar ou recusar, mas nunca alterar, forjar ou roubar. É código aberto e você pode
rodar o seu — e, como o preço é **cotado e mostrado** em vez de escondido, até a taxa
de um relay próprio ou de terceiros fica sempre visível antes de você assinar. Veja
[redes e taxas](/pt-BR/docs/networks-and-fees).

### Assinatura legível (ERC-7730)

A Vela decodifica calldata e dados tipados EIP-712 com descritores **ERC-7730** e
mostra a **intenção** (Trocar, Enviar, Aprovar…), a **substância** (valores,
endereços) e, sob demanda, os **detalhes** (nonce, prazo, calldata crua), com cores
por risco. Quando nenhum descritor casa, a Vela mostra um aviso explícito de
assinatura às cegas em vez de fingir que entendeu a chamada.

### Redes

A Vela suporta 12 redes EVM — Ethereum, BNB Chain, Polygon, Arbitrum, Optimism,
Base, Avalanche, Gnosis, Unichain, Tempo, Monad e World Chain — mais redes
personalizadas. Uma rede personalizada só pode ser adicionada se já hospedar os
contratos de que a Vela depende (o EntryPoint, os contratos Safe, o assinante
WebAuthn) e o precompilado P-256 RIP-7212; a Vela confere antes de habilitar.

## Modelo de segurança

**O que a Vela não pode fazer:**

- Mover, gastar ou transferir seus fundos — só a sua passkey pode autorizar o Safe.
- Congelar ou confiscar sua conta — o Safe é o seu contrato on-chain; a Vela não tem
  papel privilegiado nele.
- Assinar no seu lugar — cada transação exige uma asserção biométrica nova.
- Ver sua chave privada — ela nunca chega à Vela; só o seu aparelho consegue usá-la
  para assinar.
- Alterar uma transação depois que você assina — qualquer mudança invalida a
  assinatura.

**O que «não pode congelar» não cobre: o *token*.** Uma stablecoin com permissões —
USDC, USDT e a maioria dos tokens lastreados em moeda — traz uma função de blacklist
que o emissor pode acionar contra qualquer endereço, inclusive o seu. Esse poder é do
emissor e existe em qualquer carteira onde você guarde o token; nenhuma carteira
autocustodiada, Vela inclusive, pode tirá-lo. O que a autocustódia te dá é que
**nós** não somos uma segunda parte que possa fazer isso.

**No que você confia:**

- Nos **contratos Safe** (auditados, muito usados) e no assinante WebAuthn que
  verifica sua chave P-256.
- No **serviço de passkeys do seu sistema** (Apple / Google) para proteger e
  sincronizar sua credencial.
- Nos **provedores de RPC** que você consulta (a Vela usa um conjunto de várias
  fontes com failover; você pode configurar os seus).
- No **relay**, só para disponibilidade — e você pode auto-hospedá-lo.

**Ameaças consideradas:**

- **Aparelho perdido ou roubado** — quem estiver com ele ainda precisa da sua
  biometria ou do seu PIN para assinar.
- **Phishing / dApp maliciosa** — tratado pela assinatura legível.
- **Servidor da Vela comprometido** — não dá capacidade de assinatura; o raio do
  estrago é serviço degradado, não perda de fundos.
- **Risco de cadeia de suprimentos** — mitigado por código aberto e auto-hospedagem.

## Recuperação

Sua passkey é copiada pelo serviço do seu sistema operacional; em um aparelho novo,
entrar com a mesma conta Apple ou Google a restaura, e sua carteira reaparece.

<Callout type="warning" title="O backup de passkeys da sua plataforma é a sua recuperação">
A recuperação da Vela é a sua passkey, sincronizada pelo Chaveiro do iCloud ou pelo
Gerenciador de senhas do Google. Por projeto, não há frase-semente, recuperação
social nem guardiões — nada que a Vela possa perder, vazar ou ser obrigada a usar. O
outro lado é real: se você perder <strong>tanto</strong> o aparelho
<strong>quanto</strong> a passkey sincronizada na nuvem, sem nenhuma outra cópia, a
conta não pode ser recuperada. Mantenha o backup de passkeys da sua plataforma
ligado e proteja essa conta.
</Callout>

O modelo completo de recuperação, com seus limites honestos, está em
[recuperação e login](/pt-BR/docs/recovery).

## Se a Vela sumir

Autocustódia significa que suas chaves e seus fundos não dependem de a Vela estar no
ar. Os fundos vivem no **seu contrato Safe on-chain**, e o relay é código aberto e
substituível.

Uma ressalva honesta: o WebAuthn prende uma passkey a um domínio de relying party
(`getvela.app`). Se esse domínio fosse perdido em definitivo, as passkeys presas a
ele precisariam de ajuda para funcionar em outro lugar — uma ferramenta capaz de
apresentar ao autenticador a relying party original. A Vela distribuía antes uma
extensão de navegador de nível desenvolvedor para esse caso e a retirou em setembro
de 2026; um caminho de recuperação para perda de domínio adequado ao público geral
continua sendo trabalho em aberto, e dizemos isso em vez de dar a entender que já
existe. O acesso on-chain independente também depende do suporte a P-256 (RIP-7212)
da chain de destino, que vem melhorando em várias chains.

## Privacidade

Sem contas, sem e-mail, sem KYC, sem frase-semente para coletar. Os servidores
guardam só a sua **chave pública** e um nome de conta escolhido por você (para a
recuperação entre aparelhos), publicados on-chain por projeto. O conteúdo das
transações não é registrado. O site usa analytics auto-hospedada e sem cookies. Veja
a [política de privacidade](/privacy).

## Verificabilidade e código aberto

Tudo é **licenciado MIT e de código aberto** — o app e os quatro serviços de backend
(dados de chain, índice de passkeys, relay, taxas de câmbio), que você pode
**auto-hospedar** (Configurações → Avançado → Endpoints de serviço). Leia o código em
[github.com/mondaylabsltd/vela-wallet](https://github.com/mondaylabsltd/vela-wallet).

## Sem token

A Vela **não tem token** e não pretende ter. Não há nada para comprar, farmar ou
especular. O gas é pago no ativo nativo de cada rede.

## Status de auditoria e limitações

Os **contratos Safe** no centro de toda conta Vela são auditados de forma
independente e testados na prática. A **integração da própria Vela** em volta deles
**não passou por auditoria independente de terceiros**, e nenhuma está agendada por
enquanto — uma auditoria profissional é uma meta para quando o projeto puder bancar,
não um compromisso com data. Até lá a revisão é informal: o código é aberto e conta
com membros capazes e interessados da comunidade lendo, além de revisão assistida por
IA. Ajuda, mas não equivale a uma auditoria profissional. Trate a Vela como software
em alfa e use valores que você se sinta confortável em colocar em algo tão novo.

## Referências

- ERC-4337 — abstração de contas via EntryPoint
- EIP-1271 — padrão de validação de assinatura para contratos
- ERC-7730 — assinatura legível / descritores de dados estruturados
- EIP-5792 — agrupamento de chamadas da carteira
- RIP-7212 — precompilado para verificação de assinatura secp256r1 (P-256)
- WebAuthn / FIDO2 — autenticação por passkey
- [Conta inteligente Safe v1.4.1](https://github.com/safe-fndn/safe-smart-account/tree/release/v1.4.1)
