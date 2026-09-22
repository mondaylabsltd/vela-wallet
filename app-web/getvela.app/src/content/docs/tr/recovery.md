---
title: Kurtarma ve giriş
description: "Yeni bir cihazda anahtarlarınızdan herhangi biriyle cüzdanınıza nasıl dönersiniz, cüzdan nerede aranır ve kurtarma ifadesi olmadan kurtarmanın dürüstçe söylenmiş sınırları."
source: 77cf3f24c7c7
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Kurtarma ve giriş

Kurtarma ifadesi olmadığında kurtarma iki şeye dayanır: **hâlâ elinizde olan bir
anahtar** ve **hangi anahtarların cüzdanınıza ait olduğunu gösteren herkese açık bir
kayıt**.

## Cüzdan oluşturduğunuzda neler kaydedilir

Cüzdanınızın adresi, cüzdanı oluştururken kullandığınız anahtarların tamamından
hesaplanır. Bu anahtarlardan herhangi biri cüzdanı sonradan yeniden bulabilsin diye,
cüzdan oluşturmak Gnosis Chain üzerindeki herkese açık bir **kayıt defteri
sözleşmesine** bir kayıt yazar: her anahtarın açık anahtarı, cüzdanın adresi, adı ve
imzalı kayıt verisi. Kayıt defterinin sahibi yoktur; kayıtlar düzenlenemez ve
silinemez. (Neyin herkese açık olduğunun tam listesi
[cüzdanınızı oluşturun](/tr/docs/create-wallet#what-is-public) sayfasında.)

Bu kaydı Vela'nın açık anahtar dizini servisi gönderir ve gas ücretini öder; kaydın
kendisi zincir üstündedir ve herhangi bir uygulama onu doğrudan okuyabilir.

## Yeni bir cihazda giriş yapmak

1. Vela'yı açın ve giriş yapmayı seçin.
2. Anahtarlarınızdan **herhangi birini** kullanın: bu cihaza eşitlenmiş bir geçiş
   anahtarı, yakındaki bir telefon (QR kodu okutun) ya da güvenlik anahtarınız.
3. Vela o anahtarın açık anahtarını imzadan çıkarır ve arar — önce Vela'nın
   dizininde; dizin yanıt vermezse Gnosis'teki kayıt defteri sözleşmesinde, ardından
   Ethereum'dakinde — ve cüzdanı yeniden kurar. Cüzdanı size göstermeden önce,
   bulduğu anahtarların gerçekten kayıtlı adresi verdiğini kontrol eder.

Hesap listeleri cihazlar arasında eşitlenmez; giriş yapmak onları yeniden kurar.

<Callout type="info" title="Ne dizin ne kayıt defteri yanıt verirse">
<strong>Tek anahtarlı</strong> bir cüzdan, hiçbir sunucuya ihtiyaç duymadan cihazın
üzerinde yeniden kurulabilir: o anahtardan alınan iki imza, açık anahtarını geri
çıkarmaya ve adresi yeniden hesaplamaya yeter. Birden fazla anahtarı olan bir cüzdan
kayıt defteri kaydına ihtiyaç duyar, çünkü tek bir anahtar uygulamaya diğerlerinin ne
olduğunu söyleyemez.
</Callout>

## Kaydın kopyaları

Uygulamaların ilk okuduğu kayıt defteri Gnosis'tekidir. **Ayarlar**'dan cüzdanınızın
kaydını gas ücretini kendiniz ödeyerek **Ethereum**'daki aynı kayıt defteri
sözleşmesine de kopyalayabilirsiniz; böylece kayıt ikinci bir zincirde de bulunur. Bu
kopyayı herkes yapabilir; içinde para taşıyabilecek hiçbir şey yoktur.

## Dürüstçe söylenmiş sınırlar

<Callout type="warning" title="Kaybolan anahtar kaybolmuştur">
Cüzdanı oluştururken kullandığınız anahtarların hepsi gittiyse — eşitlenen geçiş
anahtarları, telefonlar, güvenlik anahtarları — cüzdanı kimse kurtaramaz: ne Vela, ne
Apple ya da Google, ne de bir başkası. Kurtarma ifadesi, destek ekibinin sıfırlaması
ya da arka kapı yoktur.
</Callout>

Bunun olasılığını düşüren şey, cüzdana birden fazla yoldan girebilmektir:

- Bu cihazın geçiş anahtarını kullanıyorsanız **geçiş anahtarı eşitlemesini açık
  tutun**. Anahtarı yeni bir telefona ya da bilgisayara taşıyan şey odur.
- **Arkasındaki hesabı güvenceye alın.** Apple ya da Google hesabınızı kontrol eden
  kişi, eşitlenmiş bir geçiş anahtarını kullanabilir; hesaba güçlü bir parola ve ayrı
  kurtarma seçenekleri tanımlayın.
- **Cüzdanı birden fazla anahtarla oluşturun**; örneğin telefonunuzun geçiş anahtarı
  ve güvenli bir yerde saklanan bir donanım güvenlik anahtarı. Anahtarlar yalnızca
  cüzdanı oluştururken eklenebilir ([neden](/tr/docs/signers)). Unutmayın, herhangi bir
  anahtar tek başına imzalayabilir — ve kaldırılamaz; bu yüzden biri ele geçirilirse
  paranızı yeni bir cüzdana taşıyın ([ne yapmalı](/tr/docs/signers)).

## Vela'nın yapabildikleri ve yapamadıkları

- **Yapabilir:** dizini çalışır durumda tutmak; böylece cüzdanınız yeni bir cihazda
  hızla bulunur.
- **Yapamaz:** paranızı taşımak, cüzdanınızı dondurmak, anahtar eklemek ya da
  kaldırmak veya kaybettiğiniz bir anahtarı kurtarmak. Vela anahtarlarınızı hiçbir
  zaman tutmaz.

Sırada: [açık imzalama](/tr/docs/clear-signing).
