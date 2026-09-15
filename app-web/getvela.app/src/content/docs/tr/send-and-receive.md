---
title: Gönderme ve alma
description: Vela'da token alma ve gönderme — ağlar arasında tek adres, açıkça imzalanan işlemler ve hesap soyutlamasının paranızı gerçekte nasıl taşıdığı.
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Gönderme ve alma

## Alma

1. Cüzdanınızı açıp **Al**'a dokunun.
2. Adresinizi paylaşın — kopyalayın ya da gönderen QR kodu okutsun.
3. Transfer zincirde onaylandığında bakiye cüzdanınızda görünür.

Bilmeye değer iki şey:

- Adresiniz **desteklenen her ağda aynı**, yani her yerde tek bir adres
  paylaşırsınız — yalnızca gönderenin doğru ağı kullandığından emin olun.
- **Cüzdanınız kurulmadan önce de para alabilirsiniz.** Vela hesapları
  karşıolgusal akıllı hesaplardır; bu yüzden bir zincirde sözleşme var olmadan
  önce adresinize para gelebilir, sözleşme ise oradaki ilk gönderiminizde kendini
  kurar.

## Gönderme

1. **Gönder**'e dokunup **token**'ı seçin.
2. **Tutarı** (token ile görüntüleme para biriminiz arasında geçiş yapabilirsiniz)
   ve **alıcıyı** girin. Vela, tanıdığı alıcıları mümkün olduğunda bir ada
   çözer — bir Vela hesabı, bir ENS adı, bir Basename vb.
3. **Gözden geçirin ve onaylayın.** Vela transferi gösterir, ardından geçiş
   anahtarınızı ister (Face ID / Touch ID / parmak izi).

### Onayladığınızda ne oluyor

Vela bir işlemi öylece "yayınlamıyor". Perde arkasında:

1. Safe hesabınız için bir ERC-4337 **UserOperation** oluşturuyor.
2. Biyometrik kontrolünüzden sonra cihazınız bunu bir **WebAuthn (P-256)**
   onayıyla imzalıyor.
3. İmzalı işlem **relay**'e gidiyor; relay onu EntryPoint'e iletiyor, Safe'iniz
   P-256 imzasını **zincir üstünde** doğrulayıp işlemi yürütüyor.

<Callout type="info" title="Relay işleminizi kurcalayamaz">
Relay, <strong>zaten imzalanmış</strong> bir UserOperation alır. İletmeyi
geciktirebilir ya da reddedebilir; ama alıcıyı, tutarı veya başka bir alanı
değiştiremez — her değişiklik imzanızı geçersiz kılar. O bir saklayıcı değil,
çalışırlık yardımcısıdır; üstelik açık kaynak olduğu için kendinizinkini
çalıştırabilirsiniz.
</Callout>

### Açık imzalama — kör onay yok

Siz imzalamadan önce Vela, işlemi **ERC-7730** tanımlayıcılarıyla çözer ve
**niyeti** (Gönder, Onayla, Takas…), **tutarları ve adresleri** ve bir risk
göstergesini gösterir — anlaşılmaz hex'i değil. Bir çağrıyı tam olarak
çözemediğindeyse anladığını varsaymak yerine açık bir **kör imza uyarısı**
gösterir. Sınırsız token onayı yalnızca işaretlenmez: Vela onu sonlu bir tutara
yeniden yazar ve hâlâ sınırsız kalacak bir onayı göndermeyi reddeder.

## Göndere basmadan önce

- **Adresin ilk ve son karakterlerini kontrol edin.** Adres değiştiren zararlı
  yazılım gerçek bir tehdit.
- **Ağı doğrulayın.** Yanlış ağda göndermek, en yaygın pahalı hata. Bkz.
  [ağlar ve ücretler](/tr/docs/networks-and-fees).
- **Yeni alıcılarla küçük başlayın.** Önce minik bir deneme transferi, ucuz bir
  sigorta.

İşlemler geri alınamaz. Yanlış adrese yapılan bir gönderimi geri çekebilecek bir
destek masası yok — kendi saklamanın doğası bu.

## Geçmişinizi okumak

Bakiyeler ve geçmiş, otomatik yük devretmeli bir genel RPC uç noktası havuzundan
canlı okunur. Ağ yavaşsa geçmiş biraz gecikebilir — dönen simge "hâlâ getiriyor"
demektir, "para gitti" değil.
