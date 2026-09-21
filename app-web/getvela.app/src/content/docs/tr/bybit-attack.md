---
title: Bybit saldırısı ve izlediği yol
description: "Şubat 2025'te Bybit yaklaşık 1,5 milyar dolar kaybetti. Kırılan Safe sözleşmeleri değil, arayüzdü. Bu sayfa saldırının izlediği yolu ve Vela'nın tasarımında bu yolu neyin kapattığını anlatıyor."
source: ac56b16b531f
---

# Bybit saldırısı ve izlediği yol

21 Şubat 2025'te Bybit, bir Safe çoklu imza soğuk cüzdanından yaklaşık **1,5 milyar
dolar** kaybetti. Sektör tarihindeki en büyük hırsızlık bu ve dikkatle okumaya
değer, çünkü tek bir şey dışında neredeyse her şey *doğruydu*.

## Ne oldu

Kamuya açık olay sonrası incelemelerden kısa hâli:

1. Saldırgan bir **`Safe{Wallet}` geliştiricisinin makinesini** ele geçirdi ve
   `Safe{Wallet}` ön yüzünü sunan AWS S3 kovasına zararlı JavaScript yerleştirdi. Kod
   19 Şubat'ta yerleştirildi, 21 Şubat'ta tetiklendi ve doğrudan Bybit'in o belirli
   Safe'ini hedefliyordu.
2. Bybit'in imzacıları arayüzü açtı ve sıradan görünen bir işlemi gözden geçirdi.
3. **Donanım cüzdanlarına** gerçekte gönderilen yük o işlem değildi. Safe proxy'sinin
   `masterCopy` alanının — slot 0'ın — üzerine yazan, yani hesabın bütün uygulamasını
   (implementation) saldırganınkiyle değiştiren bir `delegatecall`'du.
4. İmzacılar onayladı. İmzalar geçerliydi. Sözleşme kendisine söyleneni tam olarak
   yaptı.

Saldırı kamuya açık biçimde Kuzey Kore bağlantılı faaliyete atfedildi (FBI,
TraderTraitor kümesinin adını verdi).

## Kırılmayan şeyler

- **Safe sözleşmeleri değil.** Geçerli biçimde imzalanmış bir talimatı yürüttüler.
  Safe'teki hiçbir açık istismar edilmedi.
- **Kriptografi değil.** Her imza gerçekti.
- **Donanım cüzdanları değil.** Ledger cihazları süreçteydi ve yine de imzaladılar
  — çünkü donanım cüzdanı kendisine verileni gösterir ve verilen şey zararlı yüktü.
  Bir `delegatecall`'u insanın değerlendirebileceği bir şeye çeviremeyen bir cihaz,
  *kararı* değil, *anahtarı* korur.

Kırılan şey, her cüzdan arayüzünün altında yatan varsayımdı: **bir işlemi anlatan
ekran ile imzalanan baytların aynı şey olduğu.**

## Bu neden bir istisna değil, genel durum

Bir web cüzdanında üretilen imzaların çoğu bu varsayıma dayanır. Yükü arayüz
oluşturur, özeti arayüz çizer ve ikisinin birbirini tuttuğunu bağımsız hiçbir şey
kontrol etmez. O arayüzü sunan kod değiştirilirse — ele geçirilmiş bir derleme hattı,
kaçırılmış bir CDN, zararlı bir bağımlılık, çalınmış bir dağıtım kimlik bilgisi
yüzünden — özet saldırganın istediği şeye dönüşür ve imzanız gerçektir.

Vela'nın imzalama tasarımının hedef aldığı risk bu. Oltalama değil. Sızmış bir anahtar
değil. **Size yalan söyleyen bir imza ekranı.**

## Vela bu konuda ne yapıyor

**Calldata'ya kadar açık imzalama.** Her işlem, siz onaylamadan önce insanın
okuyabileceği bir niyete çevrilir — tutar, alıcı, çağrının gerçekte ne yaptığı
([ERC-7730](/tr/docs/clear-signing)). Çözemediğimiz bir çağrı, sorunsuzmuş gibi
sessizce gösterilmez; **çözülemez olarak işaretlenir**. Bybit'in yükü, bir uygulama
adresini değiştiren bir `delegatecall`'du; bir imzacıyı olduğu yerde durdurması
gereken şeyin tam da biçimi budur ve onu dost görünümlü bir özetin arkasına saklamak,
durdurmamasının nedeni oldu.

Açıkça belirtilmesi gereken iki sınır var. Bir dApp, Vela'dan doğrudan bir
`delegatecall` isteyemez — bir sayfanın yapabileceği istekler sıradan çağrılar
üretir — yani Bybit'in yükünün kendisi bu yoldan gelemezdi. Ama bir sayfa, Safe'inizden
yine Safe'inize yapılan bir çağrı *isteyebilir*: `enableModule`,
`addOwnerWithThreshold`, `setFallbackHandler`, `setGuard`. Bunlardan herhangi biri, bir
kez imzalandığında hesabı Bybit'in yükü kadar eksiksiz biçimde devreder —
etkinleştirilen bir modül daha sonra kendi `delegatecall`'unu çalıştırabilir. Vela bu
çağrıları çözer ama henüz engellemez; **hedefi kendi cüzdan adresiniz olan her isteği
reddedin.** Vela'nın kendi kodu da `Safe{Wallet}`'inki gibi değiştirilirse, çözümleme
de saldırganın çözümlemesi olur — bir sonraki madde bunun için var.

**Arayüzü kontrol edebilen bağımsız bir yol.** Vela, isteği kendisi çözen ve WebAuthn
imzasını kendisi yapan, derleme adımı ve bağımlılığı olmayan bir
[imza sayfası](/tr/docs/clear-signing-self-host) geliştirdi — baştan sona
okuyabileceğiniz, kendiniz sunabileceğiniz ya da tarayıcı uzantısı olarak
yükleyebileceğiniz tek bir statik dosya klasörü. Amacı, ana uygulamanın tedarik
zincirini paylaşmayan ikinci bir görüş olmak. *Durum: hazır ve test edildi; yayımlanmadı
ve henüz hiçbir Vela uygulaması ona istek göndermiyor.* Bu değiştiğinde bu sayfa bunu
açıkça yazacak.

**Kaybedebileceğimiz bir yönetici rolü yok.** Vela hesapları
[değiştirilmemiş Safe v1.4.1](/tr/docs/account-contract) hesaplarıdır ve Vela'nın
onlarda ayrıcalıklı bir rolü yoktur — baskıyla ya da ele geçirilerek kullanmaya
zorlanabileceğimiz bir yönetici anahtarımız ya da yükseltme yolumuz yok. Bunun neyi
ortadan *kaldırmadığı* konusunda net olalım: Bybit saldırganlarının kullandığı temel
yapı taşı — hesabın uygulamasını yeniden yazan, sahip tarafından imzalanmış bir
`delegatecall` — Vela'nınkiler dahil her Safe'te hâlâ var (Vela'nın kendi toplu
işlemleri de Safe'in MultiSend'ine `delegatecall` yapar). Bunun için
anahtarlarınızdan birinden geçerli bir imza gerekir. Kandırılıp böyle bir imza
vermenize karşı savunma, yukarıdaki çözümleme ve bağımsız kontroldür.

**Her imza için yeni bir onay.** Her imza, anahtarınızın kendi onayını gerektirir —
Face ID, parmak izi, PIN ya da güvenlik anahtarına dokunup PIN girmek. Uzun ömürlü bir
oturum anahtarı yoktur; yani bir şeyin siz orada değilken sizin adınıza
imzalayabileceği bir zaman aralığı da yoktur.

**Son güvence olarak kendi sunucunuzda barındırma.** Uygulamalar ve arka uç servisleri
açık kaynaktır. Derleme hattımıza hiç güvenmek istemiyorsanız uzantıyı ya da bir
uygulamayı kendiniz derleyin ve ihtiyacınız olan servisleri çalıştırın —
[kendi sunucunuzda barındırma kılavuzu](/tr/docs/self-hosting) bunu adım adım
anlatıyor. Bu, derleme hattımızı güven zincirinden çıkarır; derlediğiniz koda yine
güveniyorsunuz, o yüzden onu okuyun.

## Vela'nın iddia etmedikleri

Vela'nın ön yüzü de `Safe{Wallet}`'inki gibi ele geçirilebilir. Kodumuz denetlenmedi.
Aksini söylemek, tam da bu olayın sona erdirmesi gereken türden bir güvence vermek
olurdu.

Tasarımın yapmaya çalıştığı şey yolu daraltmak: yükü anlaşılmaz değil okunur kılmak,
sizin aleyhinize kötüye kullanılabilecek hiçbir yönetici rolü tutmamak ve size biz
olmayan bir şeyle doğrulama imkânı vermek. Dürüst özet şu: **bu saldırı sınıfı
tasarım gereği hafifletiliyor, ortadan kaldırılmıyor**; onu daha da sağlamlaştıracak
parçalar da bitmemiş hâlleriyle [denetimler ve bilinen sorunlar](/tr/docs/security-audits)
sayfasında listeleniyor.

## Kaynaklar

- [SlowMist — Bybit's $1.5 billion theft unveiled: `Safe{Wallet}` front-end code tampered](https://slowmist.medium.com/bybits-1-5-billion-theft-unveiled-safe-wallet-front-end-code-tampered-84b78f0fa9c2)
- [NCC Group — Bybit hack: in-depth technical analysis](https://www.nccgroup.com/research/in-depth-technical-analysis-of-the-bybit-hack/)
- [Sygnia — Investigation into the Bybit hack](https://www.sygnia.co/blog/sygnia-investigation-bybit-hack/)
- [BlockSec — A web2 breach enables the largest crypto hack in history](https://blocksec.com/blog/bybit-incident-a-web2-breach-enables-the-largest-crypto-hack-in-history)
