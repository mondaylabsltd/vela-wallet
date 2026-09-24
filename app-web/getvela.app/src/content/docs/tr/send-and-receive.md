---
title: Gönderme ve alma
description: "Vela ile nasıl para alınır ve gönderilir — her ağda tek adres, bir ya da birçok kişiye gönderim, alıcıların adlarının nereden geldiği, neyi onayladığınız ve relay'in paranızı nasıl taşıdığı."
source: 9e280dfc853b
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Gönderme ve alma

## Alma

1. Cüzdanınızı açın ve **Al**'a dokunun.
2. Adresinizi paylaşın — kopyalayın ya da QR kodunu gösterin. Web cüzdanı, tutar
   içeren bir ödeme isteği de oluşturabilir.
3. Transfer zincirde onaylandığında bakiyenizde görünür.

- Adresiniz **her ağda aynıdır**, yani tek bir adres verirsiniz — ama gönderenin
  yine de Vela'nın desteklediği ya da sizin eklediğiniz bir ağı kullanması gerekir.
- Bir ağda **cüzdanınız dağıtılmadan önce de para alabilirsiniz**. Cüzdan, o ağdaki
  ilk gönderiminizde kendiliğinden dağıtılır.

## Gönderme

1. **Gönder**'e dokunun ve **tokenı** seçin.
2. **Tutarı** (token cinsinden ya da görüntüleme para biriminizde) ve **alıcının
   adresini** girin: yapıştırarak, QR kod okutarak ya da kişilerinizden seçerek.
3. **Gözden geçirin.** Vela ne olacağını, ücreti ve varsa alıcı için bulduğu adı
   gösterir.
4. Anahtarlarınızdan biriyle **onaylayın** — Face ID, parmak izi, PIN ya da güvenlik
   anahtarınıza dokunup PIN'ini girerek.

### Birçok kişiye gönderme ya da toplama

- **Bölüştürme** — tek bir işlemde bir tokenı birkaç kişiye gönderin. Bir liste
  yapıştırabilir ya da tablo içe aktarabilir, tutarları kendi para biriminizde
  girebilirsiniz.
- **Toplama** — tek bir işlemde birkaç tokenı tek bir adrese gönderin.

İkisinde de bir kez imzalarsınız ve işlem tek bir ücret öder.

### Adresler için adlar

Bir adres girdiğinizde Vela onun için bir ad arar: önce kendi kayıt defterinde
(başka bir Vela cüzdanının adı), sonra `.bnb`, `.arb`, `.g`, Basename ve ENS ters
kayıtlarında; hepsini doğrudan ilgili zincirden okur. Bu tek yönlüdür — girdiğiniz
bir adrese ad verir. `alice.eth` gibi bir ad yazmak, adres araması yapmaz.
Kaydettiğiniz **kişiler** de adlarıyla görünür.

Bu ters kayıtlardan gelen bir ad, yalnızca **ileri yönde aynı adrese çözülüyorsa**
gösterilir. Herkes kendi ters kaydına istediği metni yazabilir, yani kayıt tek başına
hiçbir şey kanıtlamaz; cüzdan ad servisine o adın hangi adresi gösterdiğini sorar ve adı
yalnızca ikisi uyuştuğunda gösterir. Kontrol yapılamıyorsa — yanıt vermeyen bir uç
nokta, başarısız olan bir çözümleyici — adresi görürsünüz, adını değil; kontrol
edilmemiş bir ad hiçbir zaman gösterilmez.

### Ücret coini ve hız

Onay ekranı ücreti hem ücret coininde hem de kendi para biriminizde gösterir.
Ağın yerel coiniyle ya da relay'in kabul ettiği yerlerde bir USD stabilcoiniyle
ödeyebilir ve bir hız seçebilirsiniz (varsayılan: hızlı). Bir yerel coinin
**azami** tutarını gönderdiğinizde Vela ücret için yetecek kadarını ayırır.
[Ücret nasıl hesaplanır](/tr/docs/networks-and-fees).

### Onayladığınızda ne olur

1. Vela, Safe hesabınız için relay'e yapılacak ücret ödemesini de içeren bir
   ERC-4337 **UserOperation** oluşturur.
2. Anahtarınız, kimliğinizi doğruladıktan sonra onu bir **WebAuthn (P-256)**
   onayıyla (assertion) imzalar.
3. İmzalı işlem **relay**'e gider; relay onu EntryPoint'e gönderir, Safe'iniz de
   P-256 imzasını zincir üstünde doğrulayıp işlemi yürütür.

<Callout type="info" title="Relay işleminizi değiştiremez">
Relay, zaten imzalanmış bir işlem alır. Alıcıyı, tutarı ya da ücreti değiştiremez
— her değişiklik imzanızı geçersiz kılar. İşlemi geciktirebilir ya da
reddedebilir ve zincire ne zaman gireceğine o karar verir. Açık kaynaktır ve
[kendi relay'inizi çalıştırabilirsiniz](/tr/docs/self-hosting#relay).
</Callout>

Siz imzalamadan önce Vela işlemin ne yaptığını çözer ve çözemediği kısımlar için
sizi uyarır; bkz. [açık imzalama](/tr/docs/clear-signing).

## Gönder'e basmadan önce

- **Adresin başını ve sonunu kontrol edin.** Adres değiştiren zararlı yazılımlar
  gerçek bir tehdit; geçmişinize bilerek yerleştirilen benzer görünümlü adresler de.
- **Ağı doğrulayın.** Yanlış ağda göndermek yaygın ve pahalı bir hatadır.
- **Yeni bir alıcıya küçük bir tutarla başlayın.** Minik bir deneme transferi ucuz
  bir sigortadır.

İşlemler geri alınamaz. Yanlış adrese yapılan bir gönderimi kimse geri çekemez —
kendi saklamanın doğası bu.

## Geçmişiniz

Geçmişiniz, bu cihazdan gönderdiklerinizi ve her zincirin loglarından okunan token
transferlerini birleştirir. Başka bir sözleşme üzerinden size gelen düz bir yerel
coin transferi (örneğin bazı borsa çekimleri) bazı ağlarda log üretmeyebilir; bu
yüzden bakiyenizde görünüp geçmişte görünmeyebilir. Bakiyeler, otomatik yük
devretmeli bir RPC uç noktası havuzu üzerinden canlı okunur; dönen simge "para
gitti" değil, "hâlâ getiriliyor" demektir.

Sırada: [ağlar ve ücretler](/tr/docs/networks-and-fees).
