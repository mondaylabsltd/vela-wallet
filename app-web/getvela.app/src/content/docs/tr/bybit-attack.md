---
title: Bybit saldırısı
description: "Şubat 2025'te Bybit yaklaşık 1,5 milyar dolar kaybetti. Kırılan Safe sözleşmeleri değil, arayüzdü. Bu sayfa saldırının izlediği yolu ve Vela'nın tasarımında o yolu neyin kapattığını anlatıyor."
---

# Bybit saldırısı ve izlediği yol

21 Şubat 2025'te Bybit, bir Safe çoklu imza soğuk cüzdanından yaklaşık **1,5
milyar dolar** kaybetti. Sektör tarihindeki en büyük hırsızlık ve dikkatle
okumaya değer, çünkü tek bir şey dışında neredeyse her şey *doğruydu*.

## Ne oldu

Kamuya açık incelemelerden kısa hâli:

1. Saldırgan bir **`Safe{Wallet}` geliştiricisinin makinesini** ele geçirdi ve
   `Safe{Wallet}` ön yüzünü sunan AWS S3 kovasına zararlı JavaScript enjekte etti.
   Kod 19 Şubat'ta yerleştirildi, 21 Şubat'ta tetiklendi ve doğrudan Bybit'in
   Safe'ini hedefliyordu.
2. Bybit'in imzalayıcıları arayüzü açtı ve sıradan görünen bir işlemi gözden
   geçirdi.
3. **Donanım cüzdanlarına** asıl gönderilen yük o işlem değildi. Safe proxy'sinin
   `masterCopy` alanını — slot 0'ı — ezen bir `delegatecall`'dı; yani hesabın
   bütün uygulamasını saldırganınkiyle değiştiriyordu.
4. İmzalayıcılar onayladı. İmzalar geçerliydi. Sözleşme kendisine söyleneni tam
   olarak yaptı.

Saldırı kamuya açık biçimde Kuzey Kore bağlantılı faaliyete atfedildi (FBI
TraderTraitor kümesini adıyla andı).

## Kırılmayan şeyler

- **Safe sözleşmeleri değil.** Geçerli biçimde imzalanmış bir talimatı yürüttüler.
  Safe'te hiçbir hata sömürülmedi.
- **Kriptografi değil.** Her imza gerçekti.
- **Donanım cüzdanları değil.** Ledger cihazları devredeydi ve yine de imzaladılar
  — çünkü donanım cüzdanı kendisine verileni gösterir ve verilen şey zararlı yüktü.
  Bir `delegatecall`'ı insanın değerlendirebileceği bir şeye çeviremeyen cihaz,
  *anahtarı* korur, *kararı* değil.

Kırılan şey, her cüzdan arayüzünün altındaki varsayımdı: **işlemi anlatan ekran ile
imzalanan baytların aynı şey olduğu.**

## Bu neden bir tuhaflık değil, genel durum

Bir web cüzdanında ürettiğiniz her imza bu varsayıma dayandı. Yükü arayüz
oluşturur, özeti arayüz çizer ve ikisinin birbirine uyduğunu bağımsız hiçbir şey
kontrol etmez. O arayüzü sunan kod değiştirilirse — ele geçirilmiş bir derleme
hattı, kaçırılmış bir CDN, zararlı bir bağımlılık, çalınmış bir dağıtım kimlik
bilgisi yüzünden — özet, saldırganın istediği her şey olur ve imzanız gerçektir.

Vela'nın imzalama tasarımının hedeflediği risk bu. Oltalama değil. Sızmış bir
anahtar değil. **Size yalan söyleyen bir imza ekranı.**

## Vela bu konuda ne yapıyor

**Calldata'ya kadar açık imzalama.** Her işlem, siz onaylamadan önce insanın
okuyabileceği bir niyete çevrilir — tutar, alıcı, çağrının gerçekte ne yaptığı
([ERC-7730](/tr/docs/clear-signing)). Çözemediğimiz bir çağrı, sorunsuzmuş gibi
sessizce çizilmez; **çözülemez olarak işaretlenir**. Bybit'in yükü, bir uygulama
adresini değiştiren bir `delegatecall`'dı; bir imzalayıcıyı olduğu yerde durdurması
gereken şeyin şekli tam olarak budur, ve onu dost görünümlü bir özetin arkasına
saklamak da durdurmamasının yoludur.

**Arayüzü kontrol edebilecek bağımsız bir yol.** Vela; niyeti kendi çizen ve
WebAuthn imzasını kendi üreten, derlemesiz ve bağımlılıksız bir imza sayfası
geliştiriyor — baştan sona okuyabileceğiniz, kendiniz sunabileceğiniz ya da
tarayıcı uzantısı olarak çalıştırabileceğiniz tek bir statik dosya klasörü. Bütün
amacı, ana uygulamanın tedarik zincirini paylaşmayan bir ikinci görüş olmak.
*Durum: yazıldı ve test edildi, henüz yayında değil.* Yayına girdiğinde tercihe
bağlı olacak ve bu sayfa değiştiğinde bunu açıkça yazacak.

**Yükseltebileceğimiz bir sözleşme yok.** Bybit'in yükü, hesabın uygulamasını
değiştirerek işledi. Vela hesapları
[değiştirilmemiş Safe v1.4.1](/tr/docs/account-contract) ve Vela'nın onlarda
ayrıcalıklı bir rolü yok — yönetici anahtarı yok, baskıyla ya da ele geçirilerek
kullandırılabileceğimiz bir yükseltme yolu yok.

**Her imza için yeni bir biyometri.** Uzun ömürlü bir oturum anahtarı yok; yani bir
şeyin siz orada değilken sizin adınıza imzalayabileceği bir pencere de yok.

**Son çare olarak kendi barındırma.** Uygulama ve bütün arka uç servisleri açık
kaynak. Derleme hattımıza hiç güvenmek istemiyorsanız kendinizinkini çalıştırın —
bu saldırı sınıfının, birine güvenmeyi gerektirmeyen tek yanıtı bu.

## Vela'nın iddia etmediği şeyler

Vela'nın ön yüzü de `Safe{Wallet}`'ınki gibi ele geçirilebilir. Kodumuz denetlenmiş
değil. Başka bir şey söylemek, tam olarak bu olayın sonunu getirmesi gereken türden
bir güvence vermek olurdu.

Tasarımın yapmaya çalıştığı şey yolu daraltmak: yükü anlaşılmaz yerine okunur
kılmak, saldırının dayandığı yükseltme yeteneğini ortadan kaldırmak ve size biz
olmayan bir şeyle doğrulama imkânı vermek. Dürüst özet şu: **bu saldırı sınıfı
tasarım gereği hafifletiliyor, ortadan kaldırılmıyor** ve onu daha da
sağlamlaştıracak parçalar, bitmemiş hâlleriyle
[denetimler ve bilinen sorunlar](/tr/docs/security-audits) sayfasında listeli.

## Kaynaklar

- [SlowMist — Bybit's $1.5 billion theft unveiled: `Safe{Wallet}` front-end code tampered](https://slowmist.medium.com/bybits-1-5-billion-theft-unveiled-safe-wallet-front-end-code-tampered-84b78f0fa9c2)
- [NCC Group — Bybit hack: in-depth technical analysis](https://www.nccgroup.com/research/in-depth-technical-analysis-of-the-bybit-hack/)
- [Sygnia — Investigation into the Bybit hack](https://www.sygnia.co/blog/sygnia-investigation-bybit-hack/)
- [BlockSec — A web2 breach enables the largest crypto hack in history](https://blocksec.com/blog/bybit-incident-a-web2-breach-enables-the-largest-crypto-hack-in-history)
