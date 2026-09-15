---
title: Giriş
description: Vela nedir, kimin için yapıldı ve kurtarma ifadesi olmayan, kendi saklamanızda duran bir akıllı cüzdanın arkasındaki fikirler neler?
---

# Giriş

Vela, EVM ağları için **kendi saklamanızda duran bir akıllı cüzdan**. Anahtarlar
sizin, ama bir yere yazmanız gereken kurtarma ifadesi yok — yüzünüzle ya da
parmak izinizle, bir geçiş anahtarıyla imzalıyorsunuz.

Bu belgeler nasıl başlayacağınızı, cüzdan oluşturmayı, token göndermeyi ve
arkasındaki güvenlik modelini anlatıyor.

## Kısa hâli

- **Kendi saklamanızda.** Paranız yalnızca sizin kullanabileceğiniz bir anahtara
  bağlı. Vela (şirket) onu ne taşıyabilir, ne dondurabilir, ne de sizin için geri
  getirebilir.
- **Kurtarma ifadesi yok.** İmza anahtarınız, cihazınızın güvenli donanımında
  duran bir geçiş anahtarı. Kaybedilecek ya da oltalanacak on iki kelime yok.
- **Bir Safe akıllı hesabı.** Her cüzdan, ERC-4337 hesap soyutlamasıyla çalışan
  bir [Safe](https://github.com/safe-fndn/safe-smart-account) akıllı sözleşmesi —
  geçiş anahtarıyla imzalamayı ve onaylamadan önce her işlemi okumayı mümkün kılan
  şey de bu.
- **12 ağ, tek adres.** Ethereum, BNB Chain, Polygon, Arbitrum, Optimism, Base,
  Avalanche, Gnosis, Unichain, Tempo, Monad ve World Chain — artı kendi
  eklediğiniz ağlar — hepsi aynı adreste.
- **Okunabilir imzalama.** Tanımlayıcısı olan işlemler insanın okuyabileceği bir
  niyete çevrilir (ERC-7730); yoksa Vela elinden gelen çözümlemeyi yapar ve uyarı
  gösterir. Çözemediği çağrıları saklamaz, işaretler.
- **Açık kaynak.** Cüzdan ve bütün servisleri
  [GitHub'da herkese açık](https://github.com/mondaylabsltd/vela-wallet); ne
  yaptıklarını isteyen denetleyebilir.
- **Alfa yazılım.** Vela çalışıyor ve gerçek para tutuyor, ama yıllara yayılmış
  bir üretim sertleşmesinden geçmedi. Küçük tutarlarla başlayın.
  [Alfa yazısı](/blog/vela-is-in-alpha) bunun ne demek olduğunu anlatıyor.

## Kimin için

Vela, kurtarma ifadesi saklamanın tehlikesini göze almadan gerçek anlamda kendi
saklamasını isteyenler için — ve bu işten bir kez yanmış olanlar için — yapıldı.
Telefonunuzun kilidini açabiliyorsanız Vela'yı kullanabilirsiniz.

## Sırada ne var

- [Vela'yı kurun](/tr/docs/install) — tarayıcınızda çalışır, indirmeye gerek yok.
- [Cüzdanınızı oluşturun](/tr/docs/create-wallet) — ilk cüzdanınız yaklaşık bir dakikada.
- [Geçiş anahtarları nasıl çalışır](/tr/docs/passkeys) — güvenlik modeli, sade bir dille.
- [Teknik doküman](/tr/docs/whitepaper) — mimarinin ve güven modelinin tamamı.

*Nasıl*'dan çok *neden*'i merak ediyorsanız, [blog](/blog) Vela'nın nasıl
yapıldığını anlatıyor.
