---
title: Kurtarma ve giriş
description: Vela'nın kurtarma ifadesi olmadan yeni bir cihazda cüzdanınızı geri getirme yöntemi — ve bu modelin dürüstçe söylenmiş sınırları.
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Kurtarma ve giriş

Kurtarma ifadesi olmayan bir cüzdanın en zor kısmı kurtarmadır: on iki kelime
yoksa yeni bir telefonda hesabınıza nasıl döneceksiniz? Vela bunu tam olarak
şöyle çözüyor.

## Nasıl çalışıyor

Bir cüzdan oluşturduğunuzda Vela'nın **geçiş anahtarı dizinine** iki şey yazılır:

- Geçiş anahtarınızın **genel anahtarı** (özel anahtar asla).
- Cüzdana verdiğiniz **ad**.

Genel anahtar, bir akıllı sözleşme aracılığıyla Gnosis blok zincirinde saklanır;
yani herkes tarafından okunabilir ve Vela'nın sunucularının ayakta kalmasına bağlı
değildir.

**Özel** anahtarınız ise platform anahtar zincirinizin eşitlediği bir geçiş
anahtarı: Apple cihazlarda **iCloud Anahtar Zinciri**, Android'de **Google Şifre
Yöneticisi**.

Yeni bir cihazda giriş yapmak için:

1. Anahtar zinciri eşitlemesi açık olacak şekilde aynı iCloud ya da Google
   hesabına girin.
2. Vela'yı açın ve giriş yapmayı seçin.
3. Geçiş anahtarınızla kimliğinizi doğrulayın. Platform eşitlenmiş geçiş
   anahtarını, dizin de eşleşen hesabı verir. Cüzdanınız geri geldi.

Dizin bir önbellek, tek bir arıza noktası değil. Erişilemez hâle gelirse ve
hesabınız yerel depoda yoksa Vela, genel anahtarınızı iki geçiş anahtarı
imzasından cihaz üzerinde yeniden kurabilir ve cüzdan adresinizi ondan yeniden
türetebilir — hiçbir sunucuya ihtiyaç duymadan.

<Callout type="info" title="Neden böyle ikiye ayrıldı">
Zincir üstü dizindeki genel anahtar, herkesin (sıfırdan kurulmuş bir uygulama
dahil) hesabınızı bulmasını sağlar. İşlemlere asıl yetkiyi veren şeyse, güvendiğiniz
platform anahtar zincirinin eşitlediği özel anahtar. Dizindeki her şey herkese açık
veridir; içindeki hiçbir şey paranızı taşıyamaz — bunu yalnızca geçiş anahtarınızın
imzaları yapabilir.
</Callout>

## Dürüstçe söylenmiş sınırlar

Kendi saklamanız, sorumluluğun da gerçekten sizde olması demek. Anlaşılması
gerekenler şunlar.

<Callout type="warning" title="Kurtarmanız platform anahtar zincirinize bağlı">
Vela'nın cihazlar arası girişi, geçiş anahtarınızın iCloud Anahtar Zinciri ya da
Google Şifre Yöneticisi üzerinden eşitlenmesine dayanır. O hesabı güvende tutun ve
kurtarma seçeneklerini güncel tutun. Hem cihazlarınıza <strong>hem de</strong>
platform hesabınızın anahtar zincirine erişiminizi kaybederseniz, Vela özel
anahtarınızı sizin için yeniden üretemez — tasarım gereği o anahtar bizde hiç
olmadı.
</Callout>

Pratik öneriler:

- **Anahtar zinciri eşitlemesini açık tutun.** Geçiş anahtarınızı cihazlar
  arasında taşıyan şey o.
- **Apple / Google hesabınızı güvenceye alın**: güçlü bir parola ve kendi kurtarma
  yöntemleriyle. O hesap artık cüzdanınızın güvenliğinin bir parçası.
- **Mümkünse birden fazla cihazda giriş yapmış olun**, böylece kaybolan tek bir
  telefon bir kriz değil, sadece bir zahmet olur.

## Vela'nın yapabildikleri ve yapamadıkları

- **Yapabilir:** genel dizin üzerinden hesabınızı yeniden bulmanıza yardım etmek.
- **Yapamaz:** paranızı taşımak, cüzdanınızı dondurmak ya da bir özel anahtarı
  geri getirmek. Vela o anahtarı hiç tutmuyor. Kendi saklamanın bütün mesele
  olduğu yer burası — ve karşılığında yaptığınız takas da bu.
