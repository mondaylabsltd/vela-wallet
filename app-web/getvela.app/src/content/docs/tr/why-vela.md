---
title: Vela'yı neden yaptık
description: "Uzun hâli — o on iki kelimeyi nerede saklamanız bekleniyor, geçiş anahtarları neyi değiştirdi, zaten kullandığımız cüzdanlarda neyi kabul edemedik ve onun yerine hangi bedeli göze aldık."
source: 06307425f631
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Vela'yı neden yaptık

Bir cüzdan daha yapmaya niyetlenmemiştik. İşe, hiçbir zaman düzgün
yanıtlayamadığımız bir soruyla başladık:

> O on iki kelimeyi nerede saklamanız bekleniyor?

## Dürüst yanıt bir ekran görüntüsü

Notlar'a koyarsanız, belayla aranızda yalnızca çalınacak bir telefon kalır.
Kâğıda yazarsanız, artık yangını, suyu, ev taşımayı, ev arkadaşlarını, çöp
poşetlerini ve gelecekteki sizin "güvenli yer"in neresi olduğunu hatırlayıp
hatırlamayacağını düşünürsünüz.

Pek çok insan için dürüst yanıt, galerideki bir ekran görüntüsü. Herkes bunun
yanlış olduğunu biliyor. Yine de yapıyorlar — çünkü "doğru" yanıtla yaşamak fazla
zor.

Kurtarma ifadesi; hiç kopyalanmadan, fotoğraflanmadan, yanlış kutucuğa
yazılmadan ve telefonda "yardımsever" birine okunmadan onlarca yıllık sıradan
hayatı atlatması gereken bir sırdır. Bu, dikkatli bir insan için zor bir problem
değil. İnsan için zor bir problem.

## Sonra geçiş anahtarları, bir cüzdanın nasıl hissettirebileceğini değiştirdi

Her gün [Base Account](https://account.base.app) kullanıyorduk ve Face ID ile
imzalamak, kurtarma ifadelerinin hiç olmadığı kadar doğal geliyordu — tehlikeli
madde taşımaktan çok, internetin geri kalanını kullanmak gibi.

Ama kullandıkça görmezden gelemeyeceğimiz sınırlara takıldık:

- **tarayıcıda üretilen** ve yalnızca güvenmek zorunda olduğunuz **bir kurtarma
  anahtarı**,
- **özel ağ ekleyememe**,
- **kendimiz barındıramama**,
- ve en büyüğü olan sessiz sorun: **servis ortadan kalkarsa cüzdan da onunla
  birlikte ortadan kalkıyordu.**

Biz de gönül rahatlığıyla bel bağlayabileceğimiz sürümü yaptık.

## Vela aslında ne

Vela, **tamamen sizin olabilen bir geçiş anahtarı cüzdanı.**

Geçiş anahtarınız, cihazınızın onu zaten koruduğu yerde kalır — iCloud Anahtar
Zinciri, Google Şifre Yöneticisi ya da elinizde tuttuğunuz bir donanım güvenlik
anahtarı. Bir işlemi imzaladığınızda Vela, cihazınızdan onu imzalamasını ister;
cihazınız imzalar ve yalnızca imzayı geri gönderir. Vela anahtarın kendisini asla
görmez.

Çoğu cüzdanda hâlâ tehlikeli bir an var, kısa sürse bile: ekrandaki kelimeler,
bellekteki bir kurtarma ifadesi, bir tarayıcı sekmesinde duran bir kurtarma
anahtarı. Vela, o an hiç var olmasın diye tasarlandı.

<Callout type="info" title="Bir söz değil — bir mimari">
Anahtarlarınıza erişemeyiz. "Erişmeyeceğimize söz veriyoruz" değil — Vela'da buna
imkân veren bir kod yolu yok; WebAuthn buna izin vermez. Cüzdan, cihazınızın
ürettiği ve bizim yalnızca teslim aldığımız bir imzayla çalışan bir
<a href="/tr/docs/account-contract">Safe akıllı hesabıdır</a>. İmza için kullandığınız
uygulamanın belirlediği şey ise anahtarınızdan <em>neyi</em> imzalamasının
isteneceğidir — <a href="/tr/docs/whitepaper">tehdit modelinin</a> bu konuya bu
kadar yer ayırmasının nedeni de bu.
</Callout>

Vela'yı, bunu kendiniz kontrol edebilesiniz diye **açık kaynak** yaptık; mevcut bir
cüzdan şirketimizin sunucuları olmadan da çalışmaya devam etsin diye de **kendiniz
barındırabileceğiniz** biçimde kurduk — tek bir sınırla: geçiş anahtarlarınızın ait
olduğu alan adı. Bunu ve etrafından dolaşmanın yollarını
[kendi sunucunuzda barındırma kılavuzu](/tr/docs/self-hosting) anlatıyor. Ve
değiştirilmemiş
[Safe sözleşmeleri](https://github.com/safe-fndn/safe-smart-account/tree/release/v1.4.1)
üzerine kurduk; çünkü insanların parası söz konusuysa sıkıcı ve sınanmış yol doğru
yoldur — zincir üstünde şimdiden milyarları koruyan sözleşmelerin aynısı.

## Göze aldığımız bedel

Yine de bir bedel var ve onu gizlemek dürüstlük olmazdı.

Vela'da Apple ya da Google hesabınız önemlidir, çünkü eşitlenen bir geçiş anahtarı
orada durur. O hesabı kaybederseniz ya da geçiş anahtarını silerseniz, ortada ne
kurtarma ifadesi ne destek ekibinin sıfırlaması ne de bir arka kapı vardır.

Ama kendi saklamanızda duran her cüzdan, hangi riskle yaşamayı tercih ettiğinizi
seçmenizi ister. Kurtarma ifadesi kopyalanabilir, ekran görüntüsü alınabilir,
oltalama ile çalınabilir ya da gecenin birinde yanlış siteye yazılabilir. Geçiş
anahtarı farklıdır: ifşa edilecek kelime, yapıştırılacak bir sır ve onu teslim
etmeniz için sizi kandırabilecek sahte bir site yoktur. Tarayıcınız onu yalnızca
gerçek alan adındaki sayfalara sunar.

Üstelik seçim ikili değil. Bir cüzdan, her biri tek başına imzalayabilen **en fazla
yedi imzalayıcıyla** oluşturulabilir — farklı cihazlardaki geçiş anahtarları, QR kod
okutarak bağladığınız yakındaki bir telefon ya da bir USB/NFC güvenlik anahtarı.
Cüzdanınızın bir platform hesabına hiç bağlı olmamasını tercih ediyorsanız yalnızca
donanım güvenlik anahtarları kullanabilirsiniz — iki tane, çünkü bir cüzdan hiçbir
yere eşitlenmeyen tek bir anahtara dayanamaz. Tek koşul zamanlama: adresiniz
anahtarların tamamından türetildiği için anahtarlar cüzdanı oluştururken seçilir.

<Callout type="warning" title="Bunun size kazandırmadığı şey">
Ek imzalayıcılar ikinci bir kilit değil, geri dönmenin bir yoludur. Herhangi bir
anahtar tek başına imzalayabildiği için, donanım anahtarı eklemek sizi erişimi
<em>kaybetmeye</em> karşı korur — anahtarlarınızdan birini zaten ele geçirmiş birini
durdurmaz. 1-of-n'in dürüst hâli bu.
</Callout>

## Vela bu yüzden var

Saklanacak bir kurtarma ifadesi, güvenilecek bir kurtarma anahtarı ve sonsuza kadar
ayakta kalmasını ummanız gereken bir şirket olmayan bir cüzdan.

İddiaları kabul etmek yerine kontrol etmek isterseniz: mimari
[teknik dokümanda](/tr/docs/whitepaper), bağlı olduğumuz her sözleşme ile neyin
denetlenip neyin denetlenmediği
[Denetimler ve bilinen sorunlar](/tr/docs/security-audits) sayfasında, kodun
tamamı da [GitHub'da](https://github.com/mondaylabsltd/vela-wallet).

Sırada: [Vela'yı kurun](/tr/docs/install).
