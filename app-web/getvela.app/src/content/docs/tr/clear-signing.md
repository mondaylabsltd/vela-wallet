---
title: Açık imzalama
description: "Vela, siz onaylamadan önce işlemleri anlaşılmaz hex yerine sade bir dile çevirir — niyet, tutarlar, adresler ve risk. Bir çağrıyı çözemediğinde anlamış gibi yapmaz, sizi uyarır."
source: 7232328b724e
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Açık imzalama

Pek çok cüzdan, tanımadığı her sözleşme için hâlâ ham veri gösterir; "kör imzalama"
— gerçekte okuyamadığınız çağrıları onaylamak — cüzdanların boşaltılma yollarından
biridir. Vela'nın yanıtı **açık imzalama**: siz imzalamadan önce işlem, mümkün olduğu
kadar anlayabileceğiniz bir şeye çevrilir.

## Ne görürsünüz

Ham calldata yerine Vela şunları gösterir:

- **Niyet** — işlemin ne yaptığı: *Gönder*, *Onayla*, *Takas* ve benzeri.
- **Özü** — ilgili tutarlar ve adresler; token tutarları gerçek birimleriyle, alıcılar
  da varsa bir ada çözülmüş hâlde.
- **Ayrıntılar** — nonce, son tarihler ve ham calldata; gözünüze sokulmak yerine
  istediğinizde açılır.
- **Bir risk göstergesi**, renk kodlu; böylece tehlikeli işlemler hemen göze çarpar.

## Nasıl çalışır (ERC-7730)

Vela hem **sözleşme çağrılarını** hem de **EIP-712 tipli verilerini**
[ERC-7730](https://github.com/LedgerHQ/clear-signing-erc7730-registry)
tanımlayıcılarıyla çözer; bunlar bir sözleşmenin işlevlerinin ne anlama geldiğini
anlatan küçük, paylaşılabilir tanımlardır.

Vela bir tanımlayıcıyı şu sırayla arar:

1. **Uygulamaya yerleşik** — yaygın kullanılan sözleşmelerin tanımlayıcıları: Uniswap,
   PancakeSwap ve SushiSwap router'ları, WETH, Aave v3 havuzu, 1inch, Lido ve wstETH ile
   Seaport.
2. Herkese açık ERC-7730 kayıt defterini yeniden yayımlayan **Vela'nın zincir verisi
   sunucusundan alınan** tanımlayıcılar.
3. **Standart biçimler** — ERC-20 tokenları, ERC-721 ve ERC-1155 NFT'leri, ERC-4626
   kasaları ve ERC-2612 izinleri (permit); böylece gündelik işlemlerin çoğu yine
   çözülür.

**Doğrulanmış** yalnızca ilk kaynağa ayrılmıştır. Bir işlem ancak açıklama,
çalıştırdığınız uygulamaya yerleşik bir tanımlayıcıdan geldiğinde — ya da zincir
verisi sunucusundan gelip yerleşik kopyanın aynısı olduğunda, ki bu da yolda hiçbir
şeyin değiştirilmediğini kanıtlar — doğrulanmış olarak etiketlenir. Sunucunun
gönderdiği diğer her şey yine çözülür ve yine gösterilir; yanında, tanımlayıcı
servisinden geldiğini ve hiçbir şeyin onu doğrulamadığını söyleyen bir satır bulunur.
O servis imzalı değildir, yani ancak onu çalıştıran kişi kadar güvenilirdir — bu da
[kendi sunucunuzu çalıştırabilmenizin](/tr/docs/self-hosting#chain-data) bir nedeni.

Token tutarları, tokenın **gerçek zincir üstü ondalık basamaklarıyla**
biçimlendirilir. Vela bir tokenın ondalıklarını doğrulayamazsa tutarı token 18
ondalıklıymış gibi gösterir ve **doğrulanmamış olarak işaretler**; böylece yanlış bir
sayı hiçbir zaman kontrol edilmiş gibi görünmez.

## Risk düzeyleri

Çözülen her işlem bir risk düzeyi alır, böylece tehlikeli kalıplar göze çarpar:

- Onaylar ve izinler için **Dikkat** — harcama yetkisi veriyorsunuz.
- Gerçekten riskli olanlar için **Tehlike**; örneğin **sınırsız token onayı**.
- Stake etme ya da yatırma gibi rutin işlemler için daha düşük risk.

<Callout type="warning" title="Zincir üstündeki 'sınırsız' onaylar gönderilemez">
Sınırsız tutar için verilen zincir üstü bir onay, paranın sonradan boşaltılmasının en
yaygın yollarından biridir. Bir dApp "sınırsız" düzeyde bir onay istediğinde
(<code>approve</code>, <code>increaseAllowance</code> ya da Permit2'nin
<code>approve</code>'u) — 2^200 ya da daha fazla (Permit2 için 2^152); dApp'lerin
"sınırsız" için kullandığı değer budur — Vela, siz onu belirli bir tutara ya da
bakiyenize düşürene veya onayı iptal edene kadar göndermez; gönderimden hemen önceki son kontrol ham
calldata'yı okur, yani tanımlayıcı olsun olmasın çalışır. Durdurmadığı şeyler:
<strong>büyük ama sınırlı bir onay</strong> (bakiyenizin çok üstünde olsa bile),
<strong>imzalı izinler</strong> (EIP-2612 ve Permit2 imzaları) ve NFT
<code>setApprovalForAll</code> — her biri bir uyarıyla gösterilir ve karar sizindir.
</Callout>

## Vela bir çağrıyı çözemediğinde

ERC-7730 tanımlayıcısı yoksa ama işlev herkese açık bir seçici (selector)
veritabanında görünüyorsa, Vela çağrıyı genel biçimde çözer ve bir dikkat bandının
altında **elden gelen en iyi çözümleme** olarak etiketler — çözülmüş ama
doğrulanmamış. Bu da olmazsa ya da Vela bir işlemin yalnızca bir kısmını
çözebiliyorsa, onu anlamış gibi **yapmaz**.

<Callout type="danger" title="Açık bir kör imzalama uyarısı">
Bir çağrı çözülemiyorsa Vela, dost görünümlü sahte bir özet yerine net bir kör
imzalama uyarısı gösterir. Alanların yalnızca bir kısmını çözebiliyorsa görünümün
eksik olduğunu söyler ve risk düzeyini yüksek tutar. İmzaladığınız şeyin ne kadarını
Vela'nın gerçekten okuyabildiğini her zaman bilirsiniz.
</Callout>

## Bu neden önemli

Kendi saklama, kötü bir işlemi kimsenin sizin için geri alamayacağı anlamına gelir.
Savunma bir destek masası değil; onayladığınız şeyi **onaylamadan önce** anlamaktır.
Açık imzalama, Vela'nın bunu size gösterme çabasıdır ve sınırları vardır: ancak onu
gösteren uygulama kadar dürüst olabilir; [bağımsız bir kontrolün](/tr/docs/clear-signing-self-host)
önemli olmasının nedeni de bu. Vela'nın güvenlik modelinde nereye oturduğunu
[teknik dokümanda](/tr/docs/whitepaper) bulabilirsiniz.
