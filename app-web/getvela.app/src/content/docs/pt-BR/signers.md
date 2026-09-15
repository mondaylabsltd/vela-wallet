---
title: Assinantes e chaves de segurança
description: Uma carteira Vela pode ter até sete assinantes — passkeys, um aparelho por perto ou uma chave de segurança tipo YubiKey — e qualquer um deles assina sozinho. Eles são escolhidos na criação da carteira, e esta página explica por que isso não é uma limitação que esquecemos de remover.
---

# Assinantes e chaves de segurança

Uma carteira Vela é um Safe, e um Safe tem donos. A sua pode ter **até sete**, com
limiar **um**: qualquer assinante autoriza uma transação sozinho. Escreve-se
`1-of-n`.

## O que pode ser um assinante

Três tipos, e dá para misturar à vontade:

| Método | O que é | Exemplo típico |
| --- | --- | --- |
| **Plataforma** | O autenticador embutido no aparelho que você está usando | Face ID / Touch ID neste celular ou notebook, sincronizado pelo Chaveiro do iCloud ou pelo Gerenciador de senhas do Google |
| **Aparelho por perto** | Outro aparelho que você alcança escaneando um código | Seu celular assinando pelo desktop, via transporte híbrido do WebAuthn |
| **Chave de segurança** | Um autenticador removível em USB ou NFC | YubiKey e outras chaves FIDO2 |

Os três são credenciais WebAuthn na curva **P-256**. Para o Safe são
indistinguíveis: cada um é um dono cuja assinatura o verificador WebAuthn on-chain
confere do mesmo jeito.

Uma chave de segurança pode ser seu **primeiro** assinante, não só um reserva. Se
você prefere que sua carteira nunca dependa de uma conta Apple ou Google, é essa a
configuração que resolve: registre uma YubiKey na criação e assine com ela.

## Por que eles são escolhidos na criação

É a parte que surpreende, então aqui vai o mecanismo em vez de um pedido de
desculpas.

O endereço da sua carteira é **derivado** do conjunto de donos. A Vela o calcula com
`CREATE2` a partir dos dados de configuração do Safe — que incluem a chave pública
de cada assinante — antes de qualquer coisa ser implantada on-chain. É exatamente
isso que permite receber fundos em um endereço que ainda não existe.

A consequência é aritmética, não política: **um conjunto de chaves diferente é um
endereço diferente**. Acrescentar um oitavo assinante depois não estenderia sua
carteira; calcularia uma carteira nova, em um endereço novo, sem um centavo do seu
dinheiro dentro.

Então «posso adicionar uma chave depois?» tem duas respostas honestas:

- **Antes de colocar fundos:** pode — o endereço não se comprometeu com nada, é só
  criar a carteira de novo com as chaves que você quiser.
- **Depois de colocar fundos:** o endereço é onde o seu dinheiro está. Trocar donos
  de um Safe já implantado é uma operação do Safe que a Vela hoje não expõe. Planeje
  o conjunto de chaves na criação.

## Do que isso protege, na prática

**Perder um aparelho.** Com mais de um assinante, perder o celular é um transtorno:
outra chave assina. Com exatamente um assinante e a sincronização do sistema
desligada, perder o celular é perder a carteira — por isso «sua passkey sincroniza
automaticamente» descreve uma configuração que você controla, não uma garantia que a
gente possa dar no seu lugar.

**Uma conta de plataforma em que você não confia mais.** Se sua passkey mora no
Chaveiro do iCloud ou no Gerenciador de senhas do Google, quem controla essa conta
pode, em tese, usá-la. Uma chave de segurança fica com você e não sincroniza para
lugar nenhum.

E do que isso **não** protege, porque `1-of-n` corta para os dois lados: acrescentar
uma segunda chave acrescenta uma segunda *entrada*, não uma segunda fechadura. Quem
obtiver qualquer um dos seus assinantes assina sozinho. Mais chaves significa mais
resistência à perda e mais superfície ao roubo; é essa a troca, e a escolha é sua.

## Recuperar não é acrescentar

São duas coisas diferentes, e a documentação as mantém separadas:

- [Recuperação e login](/pt-BR/docs/recovery) — voltar a uma carteira existente em
  um aparelho novo com uma chave que você já tem.
- Esta página — decidir, de antemão, quais chaves existem.
