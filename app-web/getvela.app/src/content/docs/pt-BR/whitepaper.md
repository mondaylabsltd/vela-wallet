---
title: Whitepaper
description: "Como a Vela funciona e em que você precisa — e não precisa — confiar para usá-la: a conta, as chaves, a taxa, o modelo de ameaças, a recuperação e o que acontece se a Vela deixar de existir."
source: 662b69510225
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Whitepaper

<Callout type="info" title="Situação: alfa · última revisão em setembro de 2026">
Esta página descreve como a Vela funciona hoje e em que você precisa e não precisa
confiar para usá-la. A Vela está em <a href="/blog/vela-is-in-alpha">alfa</a> — comece
com valores pequenos. A Vela não tem token. Tudo aqui pode ser conferido no código
aberto; onde o código e esta página discordarem, o código está certo e esta página
tem um bug.
</Callout>

## Resumo

A Vela é uma **carteira de contrato inteligente de autocustódia** para Ethereum e
outras redes EVM. Cada carteira é uma conta **Safe v1.4.1** sem modificações,
operada pelo **ERC-4337** e controlada por até sete **passkeys** — chaves WebAuthn
P-256 guardadas nos seus aparelhos, no seu gerenciador de senhas ou em chaves de
segurança físicas. Não existe frase de recuperação.

A Vela, a empresa, nunca guarda as suas chaves e não tem nenhum papel no seu Safe,
então **não consegue mover, congelar nem confiscar os seus fundos** por conta
própria. Mas é ela que escreve e distribui o software que pede às suas chaves para
assinar — e é por isso que o modelo de ameaças abaixo importa. Os apps, o relay que
envia as transações e os serviços de apoio são de código aberto, e você pode rodar a
sua própria cópia de cada um. Em que você confia, em resumo: nos contratos, nos
autenticadores que guardam as suas chaves, no código do app com que você assina,
no domínio ao qual as suas passkeys pertencem e nos serviços para os quais você
aponta o app.

## Por que a Vela existe

- **Carteiras com frase de recuperação** colocam um segredo de 12 a 24 palavras na
  frente de cada usuário: um ponto único de falha e um alvo permanente de phishing.
- **Carteiras custodiais** eliminam a frase de recuperação tomando a custódia dos
  fundos.
- **Carteiras com passkey** que dependem dos servidores e do código fechado de uma
  única empresa eliminam a frase de recuperação, mas deixam você na mão se a empresa
  sumir.
- A **assinatura às cegas** — aprovar dados opacos que você não consegue ler — ainda
  é comum, e é uma das formas pelas quais carteiras são esvaziadas.

A Vela busca a praticidade de uma passkey sem nenhuma dessas dependências: uma conta
padrão, código aberto, serviços substituíveis e transações que você consegue ler
antes de assinar.

## Princípios de design

1. **Autocustódia, sem exceções.** As chaves são criadas e guardadas pelos seus
   autenticadores. Os serviços da Vela nunca as veem; o que eles veem está listado em
   Privacidade.
2. **Contratos padrão, sem modificações.** Nenhum contrato no caminho até os seus
   fundos foi escrito pela Vela.
3. **Verifique, não confie.** Os apps e os serviços são públicos; os serviços podem
   ser hospedados por conta própria.
4. **Decodificar antes de assinar.** O que não pode ser decodificado vem com um
   aviso explícito de assinatura às cegas.
5. **Fazer menos.** A carteira envia, recebe e assina para os dApps que você
   escolhe.

## Arquitetura

```text
Apps da Vela — web, extensão de navegador, desktop (macOS/Windows/Linux), iOS, Android
  um único núcleo em Rust compartilhado (regras, criptografia, ABI, assinatura legível) + uma camada nativa em cada um
  • monta a UserOperation e mostra o que ela faz
  • pede à sua chave uma asserção WebAuthn
        │  UserOperation assinada (taxa incluída)
        ▼
Relay (vela-relay, auto-hospedável)
  • cota a taxa, adianta o gas, envia handleOps
  • não consegue alterar a operação
        ▼
Rede EVM
  EntryPoint v0.7 → o seu Safe v1.4.1 → módulo 4337 da Safe
  O módulo de passkey da Safe verifica P-256 pelo pré-compilado RIP-7212
```

Serviços de apoio, todos de código aberto: um **índice de chaves públicas** que
registra carteiras novas num registro on-chain e responde a consultas, um diretório
de **dados de chain** e uma fonte de **cotações**. Veja o
[guia de auto-hospedagem](/pt-BR/docs/self-hosting).

### Conta

A sua carteira é um proxy **Safe v1.4.1** (singleton SafeL2), com o **módulo 4337
v0.3.0** da Safe habilitado como módulo e como fallback handler, operado pelo
**EntryPoint v0.7**. Os proprietários dela são signatários de passkey do **módulo
de passkey v0.2.1** da Safe: a primeira chave é verificada pelo signatário
compartilhado, e cada chave adicional, pelo seu próprio contrato signatário criado
pela fábrica da Safe. O limite é **1**.

O endereço é **determinístico e contrafactual**: ele é calculado com `CREATE2` a
partir dos dados de configuração do Safe, que incluem todas as chaves fundadoras,
antes de qualquer coisa ser implantada. Ele é o mesmo em todas as redes. Você pode
receber nele na hora; a sua primeira transação em cada rede implanta a carteira e
paga por isso dentro da taxa dessa transação.

### Chaves

Uma carteira tem **de uma a sete chaves**, definidas quando você a cria. Qualquer uma
delas pode assinar sozinha (1-of-n). Uma chave pode ser:

- uma passkey no aparelho que você está usando — sincronizada pelas Chaves do
  iCloud, pelo Gerenciador de senhas do Google ou por outro gerenciador de senhas, se
  você permitir;
- outro celular, conectado escaneando um QR code (o transporte híbrido do WebAuthn);
- uma chave de segurança física por USB ou NFC, que não é sincronizada em lugar
  nenhum.

Toda assinatura exige a verificação de usuário do próprio autenticador — biometria
ou o PIN do aparelho, ou o PIN e um toque numa chave de segurança. Não existe chave
de sessão. As chaves não podem ser adicionadas, removidas nem trocadas depois: em
cada rede onde a carteira ainda não foi implantada, o endereço continua
representando o conjunto fundador, então trocar proprietários numa rede faria a conta
ficar diferente de uma rede para outra.

As passkeys pertencem a uma relying party (a parte confiável, no vocabulário do
WebAuthn) — as da Vela são criadas para **`getvela.app`**. Os navegadores só as
oferecem a páginas do getvela.app ou dos seus subdomínios, e é isso que as torna
resistentes a phishing; é também uma dependência à qual este documento volta mais
adiante.

### Fluxo de assinatura

1. **Montar** uma UserOperation para o seu Safe — incluindo uma transferência que
   paga o relay — e simulá-la.
2. **Decodificar** a operação numa intenção legível e mostrá-la a você.
3. **Assinar**: o seu autenticador verifica você e produz uma asserção WebAuthn
   sobre o hash da operação.
4. **Codificar** a asserção como a assinatura do Safe que o módulo de passkey espera.
5. **Enviar** a operação assinada ao relay, que chama o EntryPoint.
6. **Verificar on-chain**: o módulo de passkey confere a assinatura P-256 com o
   pré-compilado RIP-7212 antes de o Safe executar qualquer coisa. Não existe
   verificador alternativo; uma rede sem o pré-compilado não pode ser adicionada.

### Taxas

- O relay é pago **dentro da própria operação**: a operação declara zero de taxas do
  EntryPoint e inclui uma transferência do seu Safe para o endereço do relay. O valor
  e o destinatário fazem parte do que você assina, então você paga exatamente o que a
  tela de confirmação mostrou.
- A taxa é **o triplo do gas que a carteira reserva para a operação** (as estimativas
  simuladas aumentadas em metade, com mínimos), **precificado pelo maior entre a
  leitura de preço de gas da própria carteira e o preço do relay para a velocidade
  escolhida**, com mínimo de cerca de US$ 0,01. Na Tempo, o multiplicador é dois. A
  folga na reserva e no preço deixa a taxa acima do custo real da operação on-chain,
  ainda mais na primeira transação numa rede; o relay fica com a diferença. O valor
  exato aparece na tela de confirmação antes de você assinar.
- A taxa vai para o relay configurado na carteira: o da Vela, por padrão, ou
  qualquer implantação do vela-relay, inclusive uma que você mesmo rode.
- A taxa é paga na moeda da rede ou numa stablecoin em dólar que o relay aceite
  (pathUSD na Tempo, que não tem moeda nativa). **Não há paymaster**: ninguém
  patrocina o gas, e ninguém pode filtrar transações por uma política de patrocínio.
- Se a tesouraria de gas do próprio relay numa rede estiver vazia, a carteira avisa
  antes de você assinar. Não existe depósito por usuário.

Detalhes: [redes e taxas](/pt-BR/docs/networks-and-fees).

### Assinatura legível

Chamadas e mensagens EIP-712 são decodificadas com descritores **ERC-7730** —
integrados ao app para contratos comuns, buscados no serviço de dados de chain ou
correspondidos a formatos padrão de tokens — e, como último recurso, com um banco de
dados público de seletores, marcado como melhor esforço. O que sobrar recebe um aviso
explícito de assinatura às cegas. Os descritores buscados não são autenticados
criptograficamente. Uma aprovação on-chain no nível “ilimitado” (2^200 ou mais) não
pode ser enviada até que você a reduza; uma aprovação finita alta e permits
assinados aparecem com um alerta, mas não são bloqueados. Detalhes:
[assinatura legível](/pt-BR/docs/clear-signing).

### Redes

A Vela tem 24 redes integradas — Ethereum, BNB Chain, Polygon, Arbitrum, Optimism,
Base, Avalanche, Gnosis, Unichain, Tempo, Monad, World Chain, Arc, X Layer, Stable,
Soneium, MegaETH, Robinhood Chain, Mantle, Kaia, Celo, Ink, Plume e XRPL EVM — e
aceita qualquer rede EVM que tenha os onze contratos que ela verifica e o
pré-compilado RIP-7212. (As chaves dois a sete também precisam da fábrica de
signatários de passkey da Safe nessa rede, o que a verificação ainda não cobre.)

## Modelo de segurança

**O que a Vela não consegue fazer**

- Mover, gastar ou congelar os seus fundos por conta própria — só as suas chaves
  autorizam o seu Safe, e a Vela não tem nenhum papel nele. (O que a Vela consegue
  fazer é distribuir um software que pede a sua assinatura; veja as ameaças abaixo.)
- Alterar uma transação depois que você a assina — qualquer mudança invalida a
  assinatura.
- Ler as suas chaves privadas — elas ficam nos seus autenticadores.
- Adicionar uma chave à sua carteira, ou remover uma.

**O que “não consegue congelar” não cobre: o token.** USDC, USDT e a maioria dos
tokens lastreados em moeda fiduciária permitem que o emissor coloque qualquer
endereço numa lista de bloqueio, inclusive o seu. Esse poder pertence ao emissor e
existe qualquer que seja a carteira que você usa. O que a autocustódia garante é que
a Vela não é uma segunda parte capaz de fazer isso.

**Em que você confia**

- Nos **contratos**: Safe, os módulos 4337 e de passkey dele, EntryPoint v0.7 e o
  pré-compilado RIP-7212 da rede.
- No **domínio**: qualquer página servida pelo getvela.app ou por um dos seus
  subdomínios pode pedir uma assinatura às suas chaves.
- Nos **autenticadores** que guardam as suas chaves e — para passkeys sincronizadas
  — na conta Apple, Google ou do gerenciador de senhas por trás delas.
- **No código do app com que você assina.** Ele monta a transação e mostra o que ela
  faz. Um app comprometido pode mostrar uma coisa e pedir que você assine outra; o
  aviso do autenticador não vai mostrar a diferença.
- Nos **endpoints RPC** de que você lê: um nó mentiroso pode mostrar saldos errados
  ou uma prévia de simulação errada. Você pode definir os seus próprios.
- Nos **serviços de dados de chain e de cotações**: eles fornecem listas de tokens,
  descritores, a lista de tokens aceitos para a taxa e as cotações usadas para
  transformar um valor em moeda fiduciária num valor em tokens.
- No **relay**: ele não consegue mudar o que você assinou, mas pode atrasar ou
  recusar a operação, escolher quando ela entra na rede (então poderia se antecipar a
  um swap seu, dentro da sua tolerância de slippage) e definir o preço de gas em que a
  sua taxa se baseia, até o triplo da leitura da própria carteira.

**Ameaças consideradas**

- **Aparelho perdido ou roubado** — um ladrão ainda precisa passar pela verificação
  do autenticador; outra chave devolve o acesso. Mas uma chave não pode ser removida:
  se uma delas pode estar nas mãos de outra pessoa, transfira os seus fundos para uma
  carteira nova, porque o endereço antigo continua podendo ser movimentado por aquela
  chave em todas as redes.
- **Phishing** — uma passkey não pode ser digitada num site falso, e os navegadores
  só a oferecem a páginas do getvela.app e dos seus subdomínios.
- **dApp malicioso** — tratado pela assinatura legível e pela proteção de aprovações,
  com uma lacuna séria: um dApp pode pedir uma chamada do seu Safe para ele mesmo —
  `enableModule`, `addOwnerWithThreshold`, `setFallbackHandler`, `setGuard` — e
  qualquer uma delas, assinada uma única vez, entrega a conta tão completamente quanto
  o payload da Bybit. A Vela decodifica essas chamadas, mas ainda não as bloqueia.
  Rejeite qualquer solicitação cujo destino seja o endereço da sua própria carteira.
- **Serviço de backend comprometido** (relay, índice, dados de chain, cotações) —
  nenhum poder de assinatura, mas influência real: recusar o serviço, descritores ou
  listas de tokens enganosos, cotações erradas que mudam quanto um valor em moeda
  fiduciária envia e (no caso do relay) o momento e o preço de gas descritos acima. Os
  descritores buscados não são tratados como autenticados, e cada serviço pode ser
  substituído.
- **Distribuição do app comprometida** — uma implantação web adulterada, uma
  atualização de extensão ou uma versão de app poderia apresentar uma transação
  maliciosa para você assinar. Esse é o tipo de ataque da
  [Bybit](/pt-BR/docs/bybit-attack). As mitigações hoje são limitadas: a decodificação
  e a proteção de aprovações no próprio app, versões de macOS notarizadas e a
  possibilidade de compilar a extensão ou os apps você mesmo a partir do código-fonte
  (os pacotes de release trazem checksums SHA-256, não assinaturas). Uma página de
  assinatura independente, que não compartilha o código do app, está pronta, mas
  ainda não conectada.
- **Qualquer coisa servida pelo domínio** — qualquer página do getvela.app ou dos
  seus subdomínios, inclusive um script que ela carregue, poderia pedir assinaturas às
  passkeys da Vela, e o aviso mostra só “getvela.app”. Por isso, o site proíbe as
  próprias páginas de usar passkeys e mantém o script de análise fora da página que
  guarda uma chave. Se o domínio trocasse de dono, o novo dono também controlaria
  quais apps podem usar as passkeys. A extensão e os apps compilados por você trazem o
  próprio código, embora, por padrão, ainda busquem descritores e usem serviços sob o
  getvela.app.

## Recuperação

Criar uma carteira publica as chaves públicas e o endereço dela num **contrato de
registro** público na Gnosis (que pode ser copiado para o Ethereum). Num aparelho
novo, você faz login com **qualquer uma** das chaves; o app encontra a carteira pelo
índice ou, se ele falhar, direto no registro, e confere se as chaves recalculam o
endereço registrado. Uma carteira de chave única também pode ser reconstruída a
partir de duas assinaturas, sem registro nenhum.

<Callout type="warning" title="As suas chaves são a sua recuperação">
Não há frase de recuperação, nem recuperação social, nem guardiões — nada que a Vela
pudesse perder, vazar ou ser obrigada a usar. Se todas as chaves fundadoras forem
perdidas, a carteira não pode ser recuperada. Crie a carteira com mais de uma chave,
mantenha a sincronização de passkeys ativada se depender dela e proteja a conta por
trás dela.
</Callout>

Detalhes: [recuperação e login](/pt-BR/docs/recovery).

## Se a Vela deixar de existir

Os seus fundos continuam no seu Safe, on-chain. Os contratos não dependem da Vela, e
todos os serviços que a Vela opera são de código aberto, para que outra pessoa possa
rodá-los. A única coisa que não pode mudar de lugar é a relying party das
passkeys, `getvela.app`: uma cópia da carteira web em outro domínio cria outra
carteira. Para carteiras existentes, a extensão da Vela para navegador (que pode usar
passkeys do `getvela.app` por permissão) e os apps que você mesmo compilar (com um
celular ou uma chave de segurança) continuam funcionando sem o getvela.app. O
[guia de auto-hospedagem](/pt-BR/docs/self-hosting#if-getvela-app-disappears)
detalha cada caminho e os seus limites. O acesso independente a uma rede também
exige que essa rede suporte o RIP-7212.

## Privacidade

Sem cadastro, sem e-mail, sem KYC. O que se torna público é gravado no registro
quando você cria uma carteira: a chave pública e o ID de credencial de cada chave, o
modelo do autenticador, o nome da carteira e os nomes das chaves, o endereço e os
dados de registro assinados. O índice da Vela vê esse registro antes de enviá-lo, e
também os endereços cujos nomes você procura; o relay da Vela vê o seu endereço, as
operações que você envia e o endpoint RPC que o seu app usa (inclusive qualquer chave
de API na URL dele), e guarda as operações por um tempo limitado para tentar de novo
e diagnosticar problemas. Todos os serviços veem o seu endereço IP. O site usa
análise sem cookies. A [política de privacidade](/privacy) é a lista oficial.

## Código aberto

A carteira (todos os apps e o núcleo), o relay e o serviço de cotações têm licença
MIT; o diretório de dados de chain também é MIT. O índice de chaves públicas é
público, mas ainda não tem arquivo de licença. Código:
[github.com/mondaylabsltd](https://github.com/orgs/mondaylabsltd/repositories).

## Sem token

A Vela não tem token nem planos de ter um. Não há nada para comprar, farmar ou
especular. As taxas são pagas na moeda de cada rede ou numa stablecoin.

## Situação das auditorias e limitações

Os contratos da Safe, os módulos 4337 e de passkey dela e o EntryPoint v0.7 são
auditados de forma independente e amplamente usados. **O código da própria Vela —
os apps, os serviços de backend e o contrato de registro — não passou por uma
auditoria independente de terceiros, e nenhuma está agendada**; uma auditoria
profissional é um objetivo para quando o projeto puder pagar por uma, não um
compromisso com data. Até lá, a revisão é informal: o código é aberto, membros
capacitados da comunidade o leem e ele é revisado com ferramentas de IA. Isso ajuda,
mas não equivale a uma auditoria profissional. Trate a Vela como software em alfa.
Detalhes: [auditorias e problemas conhecidos](/pt-BR/docs/security-audits).

## Referências

- ERC-4337 — Abstração de contas pelo EntryPoint
- EIP-1271 — Validação de assinaturas para contratos
- ERC-7730 — Descritores de assinatura legível
- EIP-5792 — Agrupamento de chamadas na carteira (`wallet_sendCalls`)
- RIP-7212 / EIP-7951 — Pré-compilado de verificação de assinaturas P-256
- WebAuthn / FIDO2 — Passkeys
- [Safe smart account v1.4.1](https://github.com/safe-fndn/safe-smart-account/tree/v1.4.1)
