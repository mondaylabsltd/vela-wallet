---
title: Cüzdanınızı oluşturun
description: "Bir ila yedi anahtarla Vela cüzdanı oluşturun — her adım ne yapar, anahtarlar neden oluştururken sabitlenir, neler herkese açık olur ve cüzdanınız aslında nedir."
source: a2edda21a075
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Cüzdanınızı oluşturun

Cüzdan oluşturmak bir iki dakika sürer. Web cüzdanını
[wallet.getvela.app](https://wallet.getvela.app/) adresinden — ya da uzantıyı,
masaüstü veya telefon uygulamasını — açın ve **Cüzdan Oluştur**'u seçin.

## Adımlar

1. **Cüzdanınıza ad verin.** Ad, cüzdanı tanımanıza yardımcı olur ve anahtarlarınızla
   birlikte herkese açık bir kayıt defterine yazılır — herkese açık kabul edin ve
   içine özel bir şey koymayın.
2. **Ne olacağını onaylayın.** Açık anahtarlarınızın ve cüzdan adınızın zincire
   yazılacağını, özel anahtarlarınızın cihazlarınızda ya da güvenlik anahtarlarınızda
   kalacağını ve [koşulları](/terms) ile [gizlilik politikasını](/privacy) kabul
   ettiğinizi işaretlersiniz.
3. **İlk anahtarınızı oluşturun.** Yöntemi seçin: **bu cihaz** (Face ID, Touch ID,
   parmak izi, Windows Hello), **bir telefon ya da tablet** (uygulama bu seçeneği
   sunuyorsa bir QR kod okutup anahtarı orada oluşturursunuz) ya da bir **USB
   güvenlik anahtarı**. Cihazınız geçiş anahtarını oluşturur ve onunla bir kez
   imzalar; böylece uygulama devam etmeden önce anahtarın gerçekten çalıştığını bilir.
4. **İsterseniz başka anahtarlar ekleyin.** Toplamda en fazla yedi tane, istediğiniz
   türden. Bunlardan herhangi biri tek başına imzalayabilecek. Tek anahtarınız hiçbir
   yere eşitlenmiyorsa — bir güvenlik anahtarı ya da Windows Hello — uygulama ikinci
   bir anahtar ister, çünkü eşitlenmeyen tek bir anahtarla bir cihazı kaybetmek,
   cüzdanı kaybetmek demektir.
5. **Oluşturun.** Uygulama cüzdanınızın adresini anahtarların tamamından hesaplar ve
   bu anahtar grubunu Gnosis Chain üzerindeki herkese açık kayıt defterine yayımlar.
   Kayıt zincire yazıldığında cüzdanınız açılır.

<Callout type="warning" title="Anahtarlarınızı şimdi seçin">
Adresiniz, oluşturmayı tamamladığınız andaki anahtarlardan hesaplanır; bu yüzden anahtarlar
sonradan eklenemez, kaldırılamaz ya da değiştirilemez. Nedenini ve nasıl
seçeceğinizi [İmzalayıcılar ve güvenlik anahtarları](/tr/docs/signers) sayfası
anlatıyor.
</Callout>

## Cüzdanınız nedir

Cüzdanınız bir **Safe akıllı hesabıdır** — tek bir özel anahtarı olan düz bir hesap
değil, bir sözleşme. Anahtarlarınız onun sahipleridir ve herhangi biri bir işlemi
yetkilendirebilir. İlgili bütün sözleşmelerin listesi
[Hesap sözleşmesi](/tr/docs/account-contract) sayfasında.

Adres **her ağda aynıdır** ve **karşıolgusaldır** (counterfactual): hiçbir şey
dağıtılmadan önce hesaplanır; bu yüzden herhangi bir ağda hemen para
alabilirsiniz. Sözleşme, o ağdan ilk kez gönderim yaptığınızda kendiliğinden
dağıtılır ve bu ilk işlemin ücreti dağıtımı da kapsar. Cüzdanı oluşturmak size
hiçbir şeye mal olmaz.

## Neler herkese açık

<span id="what-is-public"></span>

Cüzdan oluşturmak, Gnosis Chain üzerindeki herkese açık bir kayıt defteri
sözleşmesine kalıcı bir kayıt yazar; bu kaydı herkes okuyabilir, düzenlemek ya da
silmek mümkün değildir:

- her anahtarın **açık anahtarı** (asla özel anahtarı değil) ve **kimlik bilgisi
  kimliği** (credential ID);
- her anahtarın **kimlik doğrulayıcı modeli** (onu hangi parola yöneticisinin ya da
  güvenlik anahtarının oluşturduğu) ve kimliğinizin doğrulanıp doğrulanmadığını, anahtarın
  eşitlenip eşitlenmediğini gösteren işaretler;
- **cüzdan adı** ve **her anahtar için bir etiket**;
- **cüzdan adresi** ve oluşturulma zamanı;
- **imzalı kayıt verisinin** kendisi.

Kaydı Vela'nın açık anahtar dizini gönderir ve gas ücretini öder; bu yüzden kaydı
ilk gören odur. Kayıttaki hiçbir şey paranızı taşıyamaz; anahtarlarınızdan
herhangi birinin yeni bir cihazda cüzdanı yeniden bulmasını sağlayan şey odur
([kurtarma](/tr/docs/recovery)). Tam liste [gizlilik politikasında](/privacy),
bütün kayıtlar ise [kayıt defteri sayfasında](/registry).

## Sonraki adımlar

- [İlk tokenlarınızı alın](/tr/docs/send-and-receive)
- [Ağları ve ücretleri anlayın](/tr/docs/networks-and-fees)
- [Bir cihazı kaybederseniz ne yapmalısınız](/tr/docs/recovery)
