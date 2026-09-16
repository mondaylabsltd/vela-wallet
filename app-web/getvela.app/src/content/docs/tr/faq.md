---
title: SSS
description: Vela hakkında sık sorulan sorular — saklama, geçiş anahtarları, akıllı hesaplar, kurtarma, desteklenen ağlar, ücretler ve gizlilik.
---

# SSS

## Vela kendi saklamamda mı duruyor?

Evet. Cüzdanınız, yalnızca sizin kullanabileceğiniz bir anahtarla kontrol edilen bir
akıllı hesap; o anahtarı cihazınızın işletim sistemi tutuyor ve Vela onu hiç
görmüyor. Vela paranızı ne taşıyabilir, ne dondurabilir, ne de geri getirebilir.

## Cüzdanım normal bir hesap mı, yoksa bir sözleşme mi?

Bir **Safe akıllı hesabı** (yani bir akıllı sözleşme); ERC-4337 hesap soyutlamasıyla
çalışıyor. Geçiş anahtarıyla imzalamanızı, onaylamadan önce her işlemi okumanızı ve
her ağda aynı adresi kullanmanızı sağlayan şey de bu. Mimari için
[teknik dokümana](/tr/docs/whitepaper) bakın.

## Gerçekten kurtarma ifadesi yok mu?

Gerçekten. İmza anahtarınız, cihazınızın işletim sisteminde duran bir geçiş anahtarı
ve Vela onu hiç görmüyor. Yazıp saklayacağınız, kaybedeceğiniz ya da oltalanacağınız
on iki kelimelik bir ifade yok. Bunun neden güvenli olduğunu
[geçiş anahtarları nasıl çalışır](/tr/docs/passkeys) sayfasında okuyabilirsiniz.

## Telefonumu kaybedersem ne olur?

Geçiş anahtarınız iCloud Anahtar Zinciri ya da Google Şifre Yöneticisi üzerinden
eşitleniyorsa, yeni bir cihazda aynı hesapla giriş yaparsınız ve cüzdanınız geri
gelir. Modelin tamamı ve sınırları için
[kurtarma ve giriş](/tr/docs/recovery) sayfasına bakın.

## Hangi ağlar ve tokenlar destekleniyor?

Vela **12 EVM ağıyla** geliyor — Ethereum, BNB Chain, Polygon, Arbitrum, Optimism,
Base, Avalanche, Gnosis, Unichain, Tempo, Monad ve World Chain — artı kendi
eklediğiniz ağlar; yerel tokenları ve ERC-20'leri tutuyor. Adresiniz hepsinde aynı.
Bkz. [ağlar ve ücretler](/tr/docs/networks-and-fees).

## Maliyeti ne kadar?

Cüzdan ücretsiz ve Vela'nın **tokenı yok**. Kendi cüzdan bakiyenizden ağın **gaz**
ücretini, artı bir relay ücreti ödersiniz. Fiyatı relay bildirir ve siz imzalamadan
önce _ağ ücreti / relay ücreti / toplam_ olarak ayrıştırılmış biçimde gösterilir —
her işlemin tam maliyeti onay ekranındadır ve bildirilen tutar imzaladığınız şeyin
parçası olduğu için sonradan değişemez. Çok ucuz işlemler küçük bir asgari ücrete
takılabilir. Yerel parası olmayan Tempo'da gaz, dolar stabilcoinleriyle ödenir. Her
ağ ayrıca gaz relay hesabını etkinleştirmek için küçük ve **iade edilmeyen bir
depozito** ister (Vela bunu yeni kullanıcılar için üstlenebilir); o hesap zamanla
tükenebildiği için sonradan yeniden yüklemeniz gerekebilir — tam olarak tek seferlik
değildir. Ayrıntılar [ağlar ve ücretler](/tr/docs/networks-and-fees) sayfasında.

## Vela (şirket) ne görebiliyor, ne yapabiliyor?

Vela, cihazlar arası girişi mümkün kılmak için geçiş anahtarınızın **genel**
anahtarını ve seçtiğiniz **adı** saklıyor. Özel anahtarınızı göremez, bakiyeleriniz
herkese açık zincirlerden okunur ve e-postayla kayıt diye bir şey yoktur. Bağlayıcı
metin [gizlilik politikasıdır](/privacy).

## Vela açık kaynak mı?

Evet — cüzdan ve dört arka uç servisi (zincir verisi, geçiş anahtarı dizini, relay,
döviz kurları) MIT lisansıyla
[GitHub'da herkese açık](https://github.com/mondaylabsltd/vela-wallet) ve bunları
kendiniz barındırabilirsiniz.

## Burada olmayan bir sorum var.

[GitHub'da](https://github.com/mondaylabsltd/vela-wallet) bir issue açın ya da
[X](https://x.com/realvelawallet) veya [Telegram](https://t.me/velawallet)
üzerinden bize ulaşın.
