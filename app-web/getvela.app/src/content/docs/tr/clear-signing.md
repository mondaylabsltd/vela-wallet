---
title: Açık imzalama
description: Vela, siz onaylamadan önce işlemleri sade dile çevirir — niyet, tutarlar, adresler ve risk — anlaşılmaz hex yerine. Bir çağrıyı çözemediğindeyse anlamış gibi yapmak yerine sizi uyarır.
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Açık imzalama

Çoğu cüzdan bir onaltılık duvarı onaylamanızı ister ve iyi niyetle bekler. "Kör
imzalama" — gerçekte okuyamadığınız çağrıları onaylamak — boşaltılan cüzdanların
büyük bir bölümünün arkasındaki şey. Vela'nın yanıtı **açık imzalama**: siz
imzalamadan önce işlem, anlayabileceğiniz bir şeye çevrilir.

## Ne görüyorsunuz

Ham calldata yerine Vela şunları gösterir:

- **Niyet** — işlemin ne yaptığı: *Gönder*, *Onayla*, *Takas* ve benzeri.
- **Özü** — ilgili tutarlar ve adresler; token tutarları gerçek birimleriyle,
  alıcılar da varsa bir ada çözülmüş hâlde.
- **Ayrıntılar** — nonce, son tarihler ve ham calldata; yüzünüze dayatılmak yerine
  istediğinizde açılır.
- **Bir risk göstergesi**, renk kodlu; böylece korkutucu olan şey korkutucu görünür.

## Nasıl çalışıyor (ERC-7730)

Vela hem **sözleşme çağrılarını** hem de **EIP-712 tipli verisini**
[ERC-7730](https://github.com/LedgerHQ/clear-signing-erc7730-registry)
tanımlayıcılarıyla çözer; bunlar bir sözleşmenin işlevlerinin ne anlama geldiğini
söyleyen küçük, paylaşılabilir tanımlardır.

- **Sözleşmeye özel bir tanımlayıcı** varsa işlem **doğrulanmış** olarak işaretlenir
  ve sözleşmenin adıyla etiketlenir.
- Yoksa Vela, yaygın kalıplar için **standart tanımlayıcılara** düşer — ERC-20
  tokenlar, ERC-721 NFT'ler, ERC-4626 kasalar ve ERC-2612 izinleri — böylece
  gündelik işlerin çoğu yine çözülür.

Token tutarları, tokenın **gerçek zincir üstü ondalıkları** kullanılarak
biçimlendirilir. Vela asla öylece 18 varsaymaz; ondalıkları doğrulayamıyorsa değeri
gösterir ama tahmin yürütmek yerine **doğrulanmamış olarak işaretler**.

## Risk düzeyleri

Çözülen her işlem bir risk düzeyi alır, böylece tehlikeli kalıplar göze çarpar:

- Onaylar ve izinler için **Dikkat** — harcama yetkisi veriyorsunuz.
- Gerçekten riskli olanlar için **Tehlike**; örneğin **sınırsız token onayı**.
- Stake etmek ya da yatırmak gibi rutin işlemler için daha düşük risk.

<Callout type="warning" title="Sınırsız onaylar engellenir">
Sınırsız harcama yetkisi veren bir "approve", paranın sonradan boşaltılmasının en
yaygın yollarından biri. Vela bunları yalnızca işaretlemekle kalmaz: isteği sizin
seçtiğiniz sonlu bir tutara yeniden yazar ve gönderimden hemen önceki son kontrol,
hâlâ sınırsız kalacak her onayı göndermeyi reddeder. Bu koruma ham calldata'yı
doğrudan okur, yani sözleşme için hiçbir tanımlayıcı olmadığında bile çalışır.
</Callout>

## Vela bir çağrıyı çözemediğinde

Dürüstlük, temiz bir ekrandan daha önemli. ERC-7730 tanımlayıcısı yokken işlev
herkese açık bir seçici veritabanında görünüyorsa Vela çağrıyı genel biçimde çözer
ve bir dikkat bandının altında **elden gelen** olarak etiketler — çözülmüş ama
doğrulanmamış. Bu da olmazsa ya da Vela bir işlemin yalnızca bir kısmını
çözebiliyorsa, anlamış gibi **yapmaz**.

<Callout type="danger" title="Açık kör imza uyarısı">
Bir çağrı çözülemiyorsa Vela, dost görünümlü sahte bir özet yerine net bir kör imza
uyarısı gösterir. Alanların yalnızca bir kısmını çözebiliyorsa görüntünün eksik
olduğunu söyler ve risk düzeyini yüksek tutar. İmzaladığınız şeyin ne kadarını
Vela'nın gerçekten okuyabildiğini her zaman bilirsiniz.
</Callout>

## Bu neden önemli

Kendi saklamanız, kötü bir işlemi kimsenin sizin için geri alamayacağı anlamına
gelir. Savunma bir destek masası değil; onayladığınız şeyi **onaylamadan önce**
anlamaktır. Açık imzalama, "bu anlaşılmaz yığına güven"i "bunun tam olarak ne
yaptığı şu"ya çevirir. Vela'nın genel güvenlik modelinde nereye oturduğunu
[teknik dokümanda](/tr/docs/whitepaper) bulabilirsiniz.
