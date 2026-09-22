---
title: Guia de auto-hospedagem
description: "Tudo o que a Vela roda para você, o que cada peça faz e como substituí-la pela sua — o relay, o índice de chaves públicas, os dados de chain, as cotações e os apps —, além da única coisa que você não pode substituir e de como viver sem o getvela.app."
source: a093c30db3fb
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Guia de auto-hospedagem

O seu dinheiro está num contrato Safe on-chain, controlado pelas suas chaves. Nada
do que a Vela roda consegue movê-lo. O que a Vela roda é a engrenagem que torna a
carteira prática: um relay que envia as suas transações, um índice que ajuda um
aparelho novo a encontrar a sua carteira, um diretório de dados de chain, uma fonte
de cotações e os próprios apps.

Esta página lista cada uma dessas peças, o que deixa de funcionar sem ela e como
rodar a sua. Ela também trata da única peça que você não pode substituir — o domínio
ao qual as suas passkeys pertencem — e do que fazer se o getvela.app deixar de
existir.

<Callout type="info" title="Para quem é esta página">
Você deve se sentir à vontade com um terminal, com Docker ou Cloudflare Workers e
com a tarefa de abastecer um endereço numa rede. Nada daqui é necessário para usar a
Vela no dia a dia.
</Callout>

## O mapa

| Peça | O que faz | Padrão da Vela | Dá para substituir? | Sem ela |
| --- | --- | --- | --- | --- |
| **Relay** | Recebe a sua operação assinada, paga o gas, envia a operação e recolhe a taxa que você assinou | `vela-relay-cf.getvela.app` | Sim — rode o [vela-relay](#relay) e aponte a carteira para ele | Você não consegue enviar |
| **Índice de chaves públicas** | Registra on-chain as chaves de uma carteira nova; responde “de qual carteira esta chave faz parte?” | `p256-index-v2.getvela.app` | Sim — rode o [p256-index](#index) | Não dá para criar carteiras novas; o login passa a ler direto da rede |
| **Contrato de registro** | O registro público e permanente das chaves de cada carteira | `0x94fD…1EA9` na Gnosis | Não é preciso — ninguém é dono dele; a carteira o lê diretamente | — |
| **Dados de chain** | Detalhes das redes, listas de tokens, logos, descritores de assinatura legível | `ethereum-data.getvela.app` | Sim — rode o [ethereum-data](#chain-data) | Sem listas de tokens nem logos; menos transações decodificadas; adicionar redes falha |
| **Cotações** | Valores em moeda fiduciária na sua moeda de exibição | `vela-currency.getvela.app` | Sim — rode o [vela-currency](#exchange-rates) ou qualquer fonte compatível com Frankfurter | Os apps recorrem às cotações on-chain da Chainlink quando podem (o desktop mostra USD) |
| **Nós RPC** | Ler saldos, simular transações | Endpoints públicos por rede | Sim — por rede, em Configurações → Redes | A Vela alterna entre endpoints |
| **Os apps** | A própria carteira | wallet.getvela.app, versões publicadas | Sim — [compile-os](#web-app) | — |
| **getvela.app** | O domínio ao qual as suas passkeys pertencem | — | **Não** — veja [abaixo](#if-getvela-app-disappears) | — |

Alguns serviços de terceiros também são contatados e não pertencem à Vela: os
bancos de dados públicos de seletores de função (sourcify, openchain, 4byte), usados
como último recurso para decodificar uma transação; o diretório de autenticadores que
dá nome ao modelo da sua chave de segurança; e os servidores de túnel da Apple e do
Google quando você assina com um celular escaneando um QR code.

## A única coisa que você não pode substituir: o domínio da passkey

<span id="if-getvela-app-disappears"></span>

Uma passkey pertence ao site para o qual foi criada. As chaves da Vela são criadas
para `getvela.app`. Os navegadores só as oferecem a páginas do getvela.app ou dos
seus subdomínios (ou a origens que o getvela.app declara como relacionadas), e as
passkeys integradas de um celular só funcionam em apps que o getvela.app autoriza.
Fora do navegador, a regra é mais flexível: o Chrome deixa uma extensão com
permissão para o getvela.app usá-las, e um programa no seu computador pode pedir
diretamente a uma chave de segurança ou a um celular uma assinatura para o
getvela.app — é assim que os apps compilados por você funcionam, e é por isso que o
software que você roda importa. Disso decorrem duas coisas.

**Uma cópia da carteira web no seu próprio domínio é outra carteira.** Servido a
partir de `wallet.example.com`, o mesmo código cria passkeys para
`wallet.example.com` — chaves novas e, portanto, um endereço novo. Ela não consegue
assinar por uma carteira criada em wallet.getvela.app. Essa cópia continua útil:
para uma carteira que você criar nela, ou para rodar toda a estrutura por conta
própria, do zero.

**Para uma carteira existente, estes caminhos continuam funcionando se o
getvela.app estiver fora do ar ou deixar de existir:**

| Caminho | Chaves que pode usar | Onde conseguir |
| --- | --- | --- |
| A **extensão da Vela para navegador** (navegadores Chromium: Chrome, Edge, Brave) | Qualquer chave que o navegador alcance: a passkey deste aparelho, uma chave de segurança USB (NFC onde o computador suportar), um celular por QR | Um zip de release no [GitHub](https://github.com/mondaylabsltd/vela-wallet/releases), ou [compile-a](#web-app) |
| Um **app de desktop ou de celular compilado por você** | Um celular por QR e chaves de segurança USB | [Compile-o](#web-app) |
| Os **apps das lojas e os apps de desktop notarizados** | Um celular por QR e chaves de segurança, sempre; passkeys de “este aparelho” só enquanto o sistema operacional ainda conseguir verificar o app junto ao getvela.app | Releases do GitHub (lojas mais adiante) |

A extensão consegue usar chaves do `getvela.app` porque o Chrome deixa uma extensão
com permissão para um site usar as passkeys desse site. O navegador confere essa
permissão localmente; medimos que funciona, mas ainda não com o domínio realmente
fora do ar. Um app compilado por você consegue usar um celular ou uma chave de
segurança porque a Vela conversa diretamente com eles; a passkey do próprio celular
(“este aparelho”) exige que o app seja assinado pela Vela, e o seu não é.

A [página de assinatura](/pt-BR/docs/clear-signing-self-host) não é um caminho por
si só: ela assina solicitações que outro programa envia, e nenhum app da Vela envia
essas solicitações ainda.

<Callout type="warning" title="Quem controla o domínio pode pedir uma assinatura">
Qualquer página servida pelo getvela.app ou por um dos seus subdomínios — ou por
quem vier a controlar o domínio no futuro — pode pedir uma assinatura às suas
chaves, e o aviso do sistema mostra “getvela.app”, não a transação. As passkeys
funcionam assim em todo lugar. É por isso que o site da Vela proíbe as próprias
páginas de usar passkeys. E é também por isso que a extensão e os apps compilados por
você importam: eles trazem o próprio código, embora, por padrão, ainda busquem
descritores e usem serviços sob o getvela.app.
</Callout>

## Aponte a carteira para os seus serviços

Cada app tem quatro campos em **Configurações → Avançado → Endpoints de serviço**
(no desktop, **Configurações → Endpoints de serviço**): índice de dados da chain,
índice de passkey, Vela Relay e cotações fiat. Cada campo mostra o padrão da Vela
até você mudá-lo; **Restaurar padrões** restaura os quatro. Para o relay, o índice e
os dados de chain, a carteira chama `/api/health` e mostra um selo, verde só quando o
endpoint informa o serviço certo e retorna `status: "ok"`. Ela salva o que você
digitar de qualquer forma — espere ficar verde.

| Serviço | `service` em `/api/health` |
| --- | --- |
| Relay | `vela-relay` |
| Índice de chaves públicas | `webauthn-p256-publickey-registry` |
| Dados de chain | `ethereum-data` |
| Cotações | não é conferido pelo nome — precisa retornar uma lista de cotações com base em USD |

Até que ponto cada app respeita essas configurações hoje:

| App | Endpoints de serviço | RPC por rede |
| --- | --- | --- |
| Web e extensão | Dados de chain, relay e cotações fiat. O índice de passkey é usado para buscar nomes, mas criar uma carteira e fazer login ainda usam o índice da Vela | Sim |
| Desktop | Os quatro; um novo índice de passkey passa a valer depois que você reinicia ou sai da conta | Sim |
| Android | Os quatro, exceto que a busca de nomes para endereços ainda consulta o índice da Vela | Sim |
| iOS | **Ainda não**: a página mostra valores de exemplo e não salva. O índice de passkey pode ser alterado na tela de login quando o padrão está inacessível | Somente leitura |

Essas lacunas são bugs, e estão sendo acompanhadas.

## Rode o seu próprio relay

<span id="relay"></span>

O relay é o [vela-relay](https://github.com/mondaylabsltd/vela-relay) (Rust, MIT).
Uma única implantação atende todas as redes: a carteira chama
`https://your-relay/<chainId>`. Precisa ser o vela-relay — a carteira pede a
cotação da taxa com um método específico da Vela que bundlers ERC-4337 genéricos
não implementam.

**Do que você precisa**

- Ou Docker, mais um Redis e um servidor [Iggy](https://iggy.apache.org) que você
  já rode, ou uma conta Cloudflare no plano **Workers Paid**, com Node.js e uma
  toolchain Rust (com o target `wasm32-unknown-unknown`) na sua máquina.
- Um `OPERATOR_SECRET` (hexadecimal, com pelo menos 32 bytes). Ele deriva um
  endereço de tesouraria e um conjunto de endereços de relayer, iguais em todas as
  redes. Mantenha-o em segredo: ele controla os fundos do relay.
- Gas em cada rede que você quer atender: envie a moeda da rede (pathUSD na Tempo)
  para o seu endereço de tesouraria. A tesouraria abastece os relayers.

**Docker**

```sh
git clone https://github.com/mondaylabsltd/vela-relay
cd vela-relay
cp .env.example .env
# no .env: VELA_RELAY_IGGY_URL, VELA_RELAY_REDIS_URL, OPERATOR_SECRET,
# VELA_RELAY_CHAIN_DIRECTORY_URL se você roda os seus próprios dados de chain,
# e VELA_RELAY_IMAGE apontando para uma imagem de release em que você confia (veja docs/docker.md)
docker compose pull relay
docker compose up -d --no-build
curl --fail http://127.0.0.1:4567/readyz
```

Prefira a imagem publicada: compilar a partir do código-fonte com `docker compose up
--build` pode falhar com o Dockerfile atual. Sem Docker,
`cargo run --release --bin vela-relay` roda o relay diretamente.

**Cloudflare Workers**

```sh
cd vela-relay/vela-relay-cf
npx wrangler queues create vela-relay-ops
npx wrangler queues create vela-relay-dlq
npx wrangler secret put OPERATOR_SECRET
# dados de chain próprios: adicione "VELA_RELAY_CHAIN_DIRECTORY_URL" em "vars" no wrangler.jsonc
npx wrangler deploy
```

**Confira**

```sh
curl https://your-relay/api/health        # {"service":"vela-relay","status":"ok",…}
curl https://your-relay/v1/treasury/100   # seu endereço de tesouraria na Gnosis, e se ele precisa de gas
```

Depois, coloque `https://your-relay` no campo **Vela Relay**.

**Saiba que**

- A taxa que a carteira paga vai para a sua tesouraria. A carteira a calcula do
  mesmo jeito, qualquer que seja o relay que você usa (veja
  [redes e taxas](/pt-BR/docs/networks-and-fees)).
- Uma rede personalizada que você adicionou antes de trocar o relay mantém o
  endereço de relay com que foi adicionada.
- O relay lê os detalhes de cada rede e as stablecoins que aceita num diretório de
  redes: `ethereum-data.getvela.app`, a menos que você defina
  `VELA_RELAY_CHAIN_DIRECTORY_URL` com [o seu](#chain-data). Essa variável existe a partir do vela-relay v0.9.6; versões mais antigas sempre leem a cópia da Vela.

## Rode o seu próprio índice de chaves públicas

<span id="index"></span>

O índice é o [p256-index](https://github.com/mondaylabsltd/p256-index) (Rust, MIT).
Quando uma carteira é criada, ele confere a prova de cada chave, depois grava o
grupo no **contrato de registro** na Gnosis e paga o gas. Continue usando o registro
existente em `0x94fD1A891EB6c5F340622Baf2F3A0cb70A941EA9`: ele não tem dono,
qualquer endereço com saldo pode gravar nele e todos os apps da Vela o leem
diretamente. Um registro só seu seria invisível para eles.

**Do que você precisa**

- Docker com Redis e Iggy (o servidor), ou uma conta Cloudflare (a versão Worker,
  cujo próprio README avisa que a gravação on-chain ainda não foi testada de ponta a
  ponta).
- Uma chave privada da Gnosis com xDAI. Registrar uma carteira custa cerca de 1,1
  milhão de gas com uma chave e cerca de 3,6 milhões com sete.
- Estas configurações:

```dotenv
P256_INDEX_IGGY_URL=iggy+tcp://user:password@iggy.example:5100
P256_INDEX_REDIS_URL=redis://redis.example:6379/0
P256_INDEX_CONTRACT_ADDRESS=0x94fD1A891EB6c5F340622Baf2F3A0cb70A941EA9
P256_INDEX_DOMAIN_REGISTRY=0x5266DfF591B9F9EecfEdb8E7EfEf6c687854edaf
PRIVATE_KEY=0x…
```

O `P256_INDEX_DOMAIN_REGISTRY` é importante, mesmo que o arquivo de exemplo do
servidor o deixe de fora: sem ele, o servidor distribui desafios que o contrato
rejeita, e todo registro falha.

**Rode e confira**

```sh
git clone https://github.com/mondaylabsltd/p256-index
cd p256-index
cargo run --release -p p256-index-server
curl https://your-index/api/health   # "service":"webauthn-p256-publickey-registry","status":"ok"
```

O servidor escuta em HTTP simples (porta 11256 por padrão); coloque um proxy TLS na
frente dele, já que a carteira só aceita endpoints `https://`. No momento em que
escrevemos isto, o Dockerfile do código-fonte pode não compilar; compilar com o
Cargo funciona.

**Se nenhum índice responder**, as carteiras existentes continuam funcionando: no
login, o app lê o contrato de registro na Gnosis (e depois no Ethereum) pelos seus
nós RPC. Uma carteira com uma única chave pode até ser reconstruída a partir de duas
assinaturas, sem envolver o registro. Criar uma carteira nova precisa, sim, de um
índice, porque alguém tem que pagar pelo registro.

## Rode os seus próprios dados de chain

<span id="chain-data"></span>

Os dados de chain são o [ethereum-data](https://github.com/atshelchin/ethereum-data)
(MIT): JSON estático e imagens de cerca de 2.600 redes e dos seus tokens, além dos
descritores ERC-7730 que a Vela usa para explicar transações.

```sh
docker run -d --name ethereum-data -p 3000:3000 --restart unless-stopped \
  ghcr.io/atshelchin/ethereum-data:latest
curl http://localhost:3000/api/health   # "service":"ethereum-data","status":"ok"
```

O README dele também explica como compilar a partir do código-fonte e implantar na
Cloudflare. Sirva-o por HTTPS e coloque o endereço no campo **Índice de dados da
chain**.

O relay também lê esses arquivos, inclusive um campo específico da Vela (a lista
`stables` decide quais stablecoins podem pagar taxas). Aponte-o para a sua cópia com
`VELA_RELAY_CHAIN_DIRECTORY_URL=https://your-chain-data`; ele guarda em cache a
entrada de cada rede por uma hora.

## Rode as suas próprias cotações

<span id="exchange-rates"></span>

O [vela-currency](https://github.com/mondaylabsltd/vela-currency) (MIT) republica as
cotações diárias do Banco Central Europeu. Ele não precisa de nenhuma chave.

```sh
docker run -d -p 8080:8080 -v rates-data:/data ghcr.io/mondaylabsltd/vela-currency:latest
curl "http://localhost:8080/v2/rates?base=USD"
```

Coloque `https://your-host/v2/rates?base=USD` no campo **Cotações fiat**. Qualquer
serviço compatível com Frankfurter também funciona. Mantenha o `?base=USD`: todas as
conversões partem dele.

## Compile os apps você mesmo

<span id="web-app"></span>

Todos os apps estão em [um único repositório](https://github.com/mondaylabsltd/vela-wallet)
(MIT). O README lista os passos de compilação de cada app; a versão curta:

| App | Compilação | Assina pela sua carteira existente do getvela.app? |
| --- | --- | --- |
| Extensão de navegador | `cd app-web/vela-wallet && pnpm install && pnpm build:extension`, depois carregue `extension/dist` sem compactação em `chrome://extensions` | Sim, com qualquer chave |
| Carteira web | `cd app-web/vela-wallet && pnpm install && pnpm build`; é implantada como um Cloudflare Worker | Não — no seu domínio, ela é outra carteira (veja acima) |
| Desktop | `cd app-desktop/vela-wallet && cargo run` (scripts de empacotamento no README dele) | Sim, com um celular por QR ou uma chave de segurança USB |
| Android | Gere os bindings do núcleo e depois rode `./gradlew :app:installDebug` | Sim, com um celular por QR ou uma chave de segurança USB |
| iOS | `./rust/scripts/build-ios-xcframework.sh` e depois compile no Xcode com o seu próprio time | Sim, com um celular por QR ou uma YubiKey USB-C / Lightning (firmware 5.8 ou mais recente) |

A passkey de “este aparelho” num app compilado por você não funciona para carteiras
do getvela.app: a Apple e o Google só deixam apps assinados pela Vela usar passkeys
do `getvela.app`.

## Adicione uma rede que a Vela não traz

A Vela roda em qualquer rede EVM que tenha o pré-compilado P-256 e os contratos
padrão que ela verifica. A [configuração de rede](/pt-BR/chain-setup) diz o que
falta numa rede e implanta o que qualquer pessoa pode implantar;
[redes e taxas](/pt-BR/docs/networks-and-fees) explica os requisitos. Uma lacuna:
uma carteira com mais de uma chave também precisa da fábrica de signatários de
passkey da Safe nessa rede, e a verificação ainda não procura por ela — sem ela, só a
primeira chave consegue assinar ali.

## O que ainda aponta para a Vela depois de tudo isso

Se você substituir tudo o que está acima, sobra isto:

- **O diretório de autenticadores** que dá nome aos modelos de chave de segurança —
  só estético; os apps recorrem a um nome genérico.
- **Os arquivos de associação do getvela.app**, de que os apps das lojas precisam
  para as passkeys de “este aparelho”. Um celular ou uma chave de segurança não
  precisa deles.

E estes não são da Vela: os bancos de dados públicos de seletores, os túneis da
Apple e do Google para login pelo celular e os provedores de RPC que você escolher.

A seguir: [a página de assinatura que você mesmo pode rodar](/pt-BR/docs/clear-signing-self-host).
