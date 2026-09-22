---
title: Signatários e chaves de segurança
description: "Uma carteira Vela pode ter até sete signatários — passkeys, um celular por perto ou uma chave de segurança do tipo YubiKey — e qualquer um deles assina sozinho. Eles são escolhidos na criação da carteira; esta página explica por quê, e o que fazer se uma chave for comprometida."
source: 072891bd4768
---

# Signatários e chaves de segurança

Uma carteira Vela é um Safe, e um Safe tem proprietários. O seu pode ter **até
sete**, com limite de **um**: qualquer signatário sozinho pode autorizar uma
transação. Isso se escreve `1-of-n`.

## O que pode ser um signatário

Três tipos, e você pode combiná-los à vontade:

| Método | O que é | Exemplo típico |
| --- | --- | --- |
| **Plataforma** | O autenticador integrado ao aparelho que você está usando | Face ID / Touch ID neste celular ou notebook, sincronizado pelas Chaves do iCloud ou pelo Gerenciador de senhas do Google |
| **Aparelho por perto** | Outro aparelho que você conecta escaneando um código | O seu celular assinando para o seu computador, pelo transporte híbrido do WebAuthn |
| **Chave de segurança** | Um autenticador removível, por USB ou NFC | YubiKey e outras chaves FIDO2 |

Os três são credenciais WebAuthn na curva **P-256**. Para o Safe, eles são
indistinguíveis: cada um é um proprietário cuja assinatura o módulo de passkey da
Safe confere on-chain do mesmo jeito. (A primeira chave é verificada pelo
signatário compartilhado da Safe; cada chave adicional, pelo seu próprio pequeno
contrato signatário, criado pela fábrica da Safe na primeira vez que a carteira é
implantada numa rede.)

Quais tipos cada app aceita:

| App | Este aparelho | Celular por perto (QR) | Chave de segurança |
| --- | --- | --- | --- |
| Carteira web, extensão de navegador | Sim | Sim | USB ou NFC, pelo navegador |
| Desktop (macOS, Windows, Linux) | macOS e Windows | Sim | USB |
| Android | Sim (com os serviços do Google Play) | Sim | USB |
| iOS | Sim | Sim | YubiKey USB-C ou Lightning, firmware 5.8 ou mais recente |

A opção “este aparelho” do app de desktop (Touch ID, Windows Hello) e o suporte ao
Windows em geral são recentes e menos testados que os outros caminhos; hoje, ali, um
celular ou uma chave de segurança é a escolha mais confiável.

Uma chave de segurança pode ser o seu **primeiro** signatário, não só um backup. Se
você prefere que a sua carteira nunca dependa de uma conta Apple ou Google, crie-a
com **duas** chaves de segurança e guarde uma num lugar seguro. (Não dá para criar
uma carteira cuja única chave não esteja sincronizada em lugar nenhum: o app pede uma
segunda chave, porque perder aquele único aparelho seria perder a carteira.)

## Por que eles são escolhidos na criação

Esta é a parte que surpreende as pessoas, então aqui vai o mecanismo, e não um
pedido de desculpas.

O endereço da sua carteira é **derivado** do conjunto de proprietários. A Vela o
calcula com `CREATE2` a partir dos dados de configuração do Safe — que incluem a
chave pública de cada signatário — antes de qualquer coisa ser implantada on-chain.
É isso que permite receber fundos num endereço que ainda não existe.

Para o endereço, a consequência é aritmética: **um conjunto diferente de chaves é
um endereço diferente**. Adicionar outro signatário depois não ampliaria a sua
carteira; calcularia uma carteira nova, num endereço novo, sem nada do seu dinheiro.

Por isso, a pergunta “posso adicionar uma chave depois?” tem duas respostas
honestas:

- **Antes de colocar fundos**: sim — o endereço ainda não está comprometido com
  nada, então crie a carteira de novo com as chaves que você quer.
- **Depois de colocar fundos**: o endereço é onde está o seu dinheiro. O próprio
  Safe consegue trocar proprietários numa rede em que a sua carteira já está
  implantada — mas, em cada rede onde ela ainda não foi implantada, o mesmo
  endereço continua representando as chaves originais, então os conjuntos de
  proprietários iriam divergindo de uma rede para outra. Mantê-los sincronizados
  entre redes é possível — algumas carteiras inteligentes fazem isso —, mas a Vela
  não construiu esse recurso, então não oferece troca de proprietários. Planeje o
  conjunto de chaves na criação.

## Do que isso realmente protege você

**Perder um aparelho.** Com mais de um signatário, um celular perdido é um
transtorno: outra chave assina. Com um único signatário e a sincronização do
sistema desativada, um celular perdido é uma carteira perdida — e é por isso que “a
sua passkey sincroniza automaticamente” descreve uma configuração que você controla,
não uma garantia que possamos dar por você.

**Uma conta de plataforma em que você não confia mais.** Se a sua passkey fica nas
Chaves do iCloud ou no Gerenciador de senhas do Google, quem controla essa conta
pode, em tese, usá-la. Uma chave de segurança fica com você e não é sincronizada em
lugar nenhum.

E do que isso **não** protege, porque o `1-of-n` corta para os dois lados:
adicionar uma segunda chave adiciona uma segunda forma de *entrar*, não uma segunda
tranca. Quem obtiver qualquer um dos seus signatários consegue assinar sozinho.
Mais chaves significam mais resiliência contra perda e mais superfície contra
roubo; essa é a troca, e a decisão é sua.

## Se uma chave pode ter sido comprometida

Uma chave não pode ser removida. Se uma das suas chaves pode estar nas mãos de
outra pessoa — um celular desbloqueado que sumiu, um código que alguém viu, uma
conta Apple ou Google que você não controla mais —, **transfira tudo para uma
carteira nova**, criada com chaves em que você confia. O endereço antigo continua
podendo ser movimentado por aquela chave em todas as redes, inclusive fundos que
alguém envie para ele depois.

## Recuperar versus adicionar

São coisas diferentes, e a documentação as mantém separadas:

- [Recuperação e login](/pt-BR/docs/recovery) — voltar a uma carteira existente num
  aparelho novo, com uma chave que você já tem.
- Esta página — decidir, de antemão, quais chaves vão existir.
