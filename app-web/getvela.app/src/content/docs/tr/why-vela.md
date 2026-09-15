---
title: Vela'yı neden yaptık
description: Uzun hâli — o on iki kelimeyi nerede saklamanız bekleniyor, geçiş anahtarları neyi değiştirdi, zaten kullandığımız cüzdanlarda neyi kabul edemedik ve yerine hangi takası seçtik.
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Vela'yı neden yaptık

Bir cüzdan daha yapmaya niyetlenmemiştik. İşe, hiçbir zaman düzgün
yanıtlayamadığımız bir soruyla başladık:

> O on iki kelimeyi nerede saklamanız bekleniyor?

## Dürüst yanıt bir ekran görüntüsü

Notlar'a koyarsanız, çalınan bir telefon kadar yakınsınız belaya. Kâğıda
yazarsanız, artık yangını, suyu, ev taşımayı, ev arkadaşlarını, çöp poşetlerini ve
gelecekteki sizin "güvenli yer"in neresi olduğunu hatırlayıp hatırlamayacağını
düşünüyorsunuz.

Pek çok insan için dürüst yanıt, galerideki bir ekran görüntüsü. Herkes bunun
yanlış olduğunu biliyor. Yine de yapıyorlar — çünkü "doğru" yanıtla yaşamak fazla
zor.

Kurtarma ifadesi; hiç kopyalanmadan, fotoğraflanmadan, yanlış kutucuğa
yazılmadan ve telefonda yardımsever birine okunmadan onlarca yıllık sıradan hayatı
atlatması gereken bir sır. Bu, dikkatli bir insan için zor bir problem değil. Bir
insan için zor bir problem.

## Sonra geçiş anahtarları, bir cüzdanın nasıl hissettirebileceğini değiştirdi

Her gün [Base Account](https://account.base.app) kullanıyorduk ve Face ID ile
imzalamak, kurtarma ifadelerinin hiç olmadığı kadar doğal geliyordu — tehlikeli
madde taşımaktan çok, internetin geri kalanını kullanmak gibi.

Ama kullandıkça, görmezden gelemeyeceğimiz duvarlara tosladık:

- **tarayıcıda üretilen ve yalnızca güvenmek zorunda olduğunuz bir kurtarma
  anahtarı**,
- **özel ağ ekleyememe**,
- **hiçbir parçasını kendimiz barındıramama**,
- ve en büyüğü olan sessiz sorun: **servis kapanırsa cüzdan da onunla birlikte
  gidiyordu.**

Biz de bağımlı olmaya razı olduğumuz sürümü yaptık.

## Vela aslında ne

Vela, **tamamen sizin olabilen bir geçiş anahtarı cüzdanı.**

Geçiş anahtarınız, cihazınızın onu zaten koruduğu yerde kalır — iCloud Anahtar
Zinciri, Google Şifre Yöneticisi ya da elinizde tuttuğunuz bir donanım güvenlik
anahtarı. Bir işlemi imzaladığınızda Vela cihazınıza bir meydan okuma gönderir;
cihazınız onu imzalar ve yalnızca imzayı geri yollar. Vela anahtarın kendisini asla
görmez.

Çoğu cüzdanda hâlâ tehlikeli bir an var, kısa sürse bile: ekrandaki kelimeler,
bellekteki bir kurtarma ifadesi, bir tarayıcı sekmesinde duran bir kurtarma
anahtarı. Vela, o an hiç var olmasın diye tasarlandı.

<Callout type="info" title="Bir söz değil — bir mimari">
Anahtarlarınıza erişemeyiz. "Erişmeyeceğimize söz veriyoruz" değil — Vela'da buna
imkân veren bir kod yolu yok. Cüzdan, cihazınızın ürettiği ve bizim yalnızca
teslim aldığımız bir imzayla çalışan bir <a href="/tr/docs/security-audits">Safe
akıllı hesabı</a>.
</Callout>

Vela'yı, bunu kendiniz kontrol edebilesiniz diye **açık kaynak** yaptık;
cüzdanınız şirketimizin ayakta kalmasına hiç bağlı olmasın diye de **kendiniz
barındırabilir** kıldık. Ve değiştirilmemiş
[Safe sözleşmeleri](https://github.com/safe-fndn/safe-smart-account/tree/release/v1.4.1)
üzerine kurduk; çünkü insanların parası söz konusuysa sıkıcı ve savaşta denenmiş
yol doğru yoldur — zincir üstünde şimdiden milyarları koruyan sözleşmelerin
aynısı.

## Seçtiğimiz takas

Yine de bir takas var ve onu gömmek dürüstlük olmazdı.

Vela'da Apple ya da Google hesabınız önemlidir, çünkü eşitlenen bir geçiş anahtarı
orada durur. O hesabı kaybedin ya da geçiş anahtarını silin: ortada kurtarma
ifadesi, destek sıfırlaması ya da arka kapı yok.

Ama kendi saklamanızda duran her cüzdan, hangi riskle yaşamayı tercih ettiğinizi
sorar. Kurtarma ifadesi kopyalanabilir, ekran görüntüsü alınabilir, oltalanabilir
ya da gecenin birinde yanlış siteye yazılabilir. Geçiş anahtarı farklıdır: ifşa
edilecek kelime, yapıştırılacak bir sır ve onu teslim etmeniz için sizi
kandırabilecek sahte bir site yoktur. Cihazınız gerçek alan adı için imzalar, ya da
hiç imzalamaz.

Üstelik seçim ikili değil. Bir cüzdan, her biri tek başına imzalayabilen **yediye
kadar imzalayıcıyla** oluşturulabilir — farklı cihazlardaki geçiş anahtarları,
okuttuğunuz yakındaki bir telefon ya da bir USB/NFC güvenlik anahtarı. Cüzdanınızın
bir platform hesabına hiç bağlı olmamasını tercih ediyorsanız, en baştaki ilk
anahtarı bir donanım güvenlik anahtarı yapabilirsiniz. Tek koşul zamanlama:
adresiniz anahtarların tamamından türetildiği için, anahtarlar cüzdanı
oluştururken seçilir.

<Callout type="warning" title="Bunun size kazandırmadığı şey">
Ek imzalayıcılar ikinci bir kilit değil, geri dönebileceğiniz ikinci bir yoldur.
Tek bir anahtar imzalayabildiği için, donanım anahtarı eklemek sizi
<em>erişimi kaybetmeye</em> karşı korur — anahtarlarınızdan birini zaten ele
geçirmiş birini durdurmaz. 1-of-n'in dürüst biçimi bu.
</Callout>

## Vela bu yüzden var

Saklanacak kurtarma ifadesi, güvenilecek bir kurtarma anahtarı ve sonsuza kadar
ayakta kalmasını ummanız gereken bir şirket olmayan bir cüzdan.

İddiaları kabul etmek yerine kontrol etmek isterseniz: mimari
[teknik dokümanda](/tr/docs/whitepaper), bağlı olduğumuz her sözleşme ile neyin
denetlenip neyin denetlenmediği
[Denetimler ve bilinen sorunlar](/tr/docs/security-audits) sayfasında, kodun
tamamı da [GitHub'da](https://github.com/mondaylabsltd/vela-wallet).

Sırada: [Vela'yı kurun](/tr/docs/install).
