---
title: SSS
description: "Saklama, anahtarlar, kurtarma, ağlar, ücretler, Vela'nın neleri görebildiği, açık kaynak ve Vela ortadan kalkarsa ne olacağı hakkında kısa yanıtlar."
source: 0762e55bf54d
---

# SSS

## Vela, varlıkları kendi saklamamda tutan bir cüzdan mı?

Evet. Cüzdanınız, yalnızca anahtarlarınızın kontrol ettiği bir Safe akıllı
hesabıdır; anahtarlarınız da cihazlarınızda, parola yöneticinizde ya da güvenlik
anahtarlarınızda kalır. Vela'nın onda ne bir anahtarı ne de bir rolü vardır; bu yüzden
paranızı kendi başına taşıyamaz, donduramaz ya da kurtaramaz. Ama anahtarlarınızdan
imza isteyen yazılımı yazan Vela'dır — bkz. [tehdit modeli](/tr/docs/whitepaper).

## Gerçekten kurtarma ifadesi yok mu?

Gerçekten yok. Anahtarlarınız geçiş anahtarlarıdır ve bir geçiş anahtarının bir yere
yazabileceğiniz ya da klavyeyle girebileceğiniz bir sırrı yoktur. Bkz.
[geçiş anahtarları nasıl çalışır](/tr/docs/passkeys).

## Cüzdan oluşturmak için neye ihtiyacım var?

Geçiş anahtarlarını destekleyen bir cihaz (Face ID, parmak izi ya da Windows Hello olan
güncel bir telefon veya bilgisayar) ya da iki donanım güvenlik anahtarı. E-posta, hesap
ya da başlangıç bakiyesi gerekmez. Cüzdanı en fazla yedi anahtarla oluşturabilirsiniz;
anahtarlar sonradan eklenemez. Bkz. [cüzdanınızı oluşturun](/tr/docs/create-wallet).

## Telefonumu kaybedersem ne olur?

Yeni bir cihazda başka herhangi bir anahtarla giriş yapın: iCloud Anahtar Zinciri ya
da Google Şifre Yöneticisi üzerinden eşitlenmiş aynı geçiş anahtarı, başka bir telefon
ya da güvenlik anahtarınız. Telefon tek anahtarınızı tutuyorduysa ve o anahtar
eşitlenmemişse cüzdan kurtarılamaz. Bkz. [kurtarma ve giriş](/tr/docs/recovery).

## Hangi ağlar ve tokenlar destekleniyor?

Ethereum, Base, Arbitrum, Optimism, Polygon, BNB Chain, Gnosis ve Avalanche dahil 24
yerleşik EVM ağı, artı gereksinimleri karşılayan ve sizin eklediğiniz her EVM ağı.
Yerel coinler ve ERC-20 tokenları. Adres her ağda aynıdır. Bkz.
[ağlar ve ücretler](/tr/docs/networks-and-fees).

## Maliyeti ne?

- **Uygulamalar:** web cüzdanı, tarayıcı uzantısı ve masaüstü uygulamaları ücretsizdir.
  iOS ve Android uygulamaları mağazalarda tek seferlik satın alma olacak; herhangi bir
  uygulamayı kaynak koddan ücretsiz olarak kendiniz de derleyebilirsiniz.
- **Her işlem:** cüzdanınızdan, işlemi zincire gönderen relay'e ödenen bir ücret. Gas
  bedelini ve relay'in payını karşılar; çoğu zaman işlemin zincir üstü maliyetinin on
  katı ya da daha fazlasıdır ve asgari tutar yaklaşık 0,01 dolardır. Kesin tutar onay
  ekranındadır ve imzaladığınız şeyin parçasıdır. Depozito ya da abonelik yoktur.
  [Ücret nasıl hesaplanır](/tr/docs/networks-and-fees).
- **Token yok.** Vela'nın tokenı yok ve çıkarma planı da yok.

## Vela'yı dApp'lerle kullanabilir miyim?

Evet; Vela tarayıcı uzantısı (Chrome, Edge, Brave) ve masaüstü (macOS, Windows), iOS
ve Android uygulamalarının yerleşik tarayıcısı üzerinden. wallet.getvela.app
adresindeki web cüzdanı dApp'lere bağlanmaz. Bkz. [kurulum](/tr/docs/install#dapps).

## Vela neleri görebilir, neler yapabilir?

Vela anahtarlarınızı okuyamaz ya da paranızı kendi başına taşıyamaz. Servisleri IP
adresinizi ve uygulamanın onlara sorduklarını görür: dizin, yeni bir cüzdanı
kaydederken açık anahtarlarınızı ve cüzdan adınızı, ayrıca adını aradığınız adresleri
görür; relay adresinizi, gönderdiğiniz işlemleri ve uygulamanızın kullandığı RPC uç
noktasını görür; zincir verisi servisi, uygulamanızın hangi tokenları ve sözleşmeleri
sorduğunu görür. Zincir üstünde neyin herkese açık olduğu
[cüzdanınızı oluşturun](/tr/docs/create-wallet#what-is-public) sayfasında
listeleniyor. Eksiksiz ve bağlayıcı sürüm [gizlilik politikasıdır](/privacy).

## Vela açık kaynak mı?

Cüzdan uygulamaları, relay ve döviz kuru servisi
[GitHub'da](https://github.com/orgs/mondaylabsltd/repositories) MIT lisanslıdır;
zincir verisi dizini de MIT'dir. Açık anahtar dizini herkese açık ama henüz bir lisans
dosyası yok. Her servisi kendiniz çalıştırabilirsiniz — bkz.
[kendi sunucunuzda barındırma kılavuzu](/tr/docs/self-hosting).

## Vela denetlendi mi?

Paranızın durduğu sözleşmeler — Safe ve modülleri ile ERC-4337 EntryPoint —
denetlendi. Vela'nın kendi kodu denetlenmedi ve takvime alınmış bir denetim de yok.
Bkz. [denetimler ve bilinen sorunlar](/tr/docs/security-audits).

## Vela kapanırsa ne olur?

Paranız zincir üstünde, kendi Safe'inizde kalır. Mevcut bir cüzdan için Vela tarayıcı
uzantısı ve kendi derlediğiniz uygulamalar getvela.app olmadan çalışmaya devam eder ve
her servis açık kaynaktır, başka biri de çalıştırabilir (relay'in zincir verisini
Vela'nın sunucusundan okumayı bırakması için kodunda bir değişiklik gerekir). Yolları
ve sınırlarını
[kendi sunucunuzda barındırma kılavuzu](/tr/docs/self-hosting#if-getvela-app-disappears)
listeliyor.

## Sorum burada yok.

[GitHub'da](https://github.com/mondaylabsltd/vela-wallet/issues) bir issue açın ya da
bize [X](https://x.com/realvelawallet) veya [Telegram](https://t.me/velawallet)
üzerinden ulaşın.
