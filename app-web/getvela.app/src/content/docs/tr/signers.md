---
title: İmzalayıcılar ve güvenlik anahtarları
description: "Bir Vela cüzdanının en fazla yedi imzalayıcısı olabilir — geçiş anahtarları, yakındaki bir telefon ya da YubiKey sınıfı bir güvenlik anahtarı — ve herhangi biri tek başına imzalar. Bunlar cüzdan oluşturulurken seçilir; bu sayfa nedenini ve bir anahtar ele geçirilmiş olabilirse ne yapmanız gerektiğini anlatıyor."
source: 072891bd4768
---

# İmzalayıcılar ve güvenlik anahtarları

Bir Vela cüzdanı bir Safe'tir ve Safe'in sahipleri olur. Sizinkinin **en fazla yedi**
sahibi olabilir ve eşik **bir**dir: herhangi bir imzalayıcı bir işlemi tek başına
yetkilendirebilir. Bu, `1-of-n` diye yazılır.

## Neler imzalayıcı olabilir

Üç tür var ve bunları serbestçe karıştırabilirsiniz:

| Yöntem | Nedir | Tipik örnek |
| --- | --- | --- |
| **Platform** | Kullandığınız cihazın içindeki kimlik doğrulayıcı | Bu telefondaki ya da dizüstündeki Face ID / Touch ID; iCloud Anahtar Zinciri ya da Google Şifre Yöneticisi ile eşitlenir |
| **Yakındaki cihaz** | Bir kod okutarak bağlandığınız başka bir cihaz | Telefonunuzun WebAuthn hibrit taşıması üzerinden masaüstü bilgisayarınız için imzalaması |
| **Güvenlik anahtarı** | USB ya da NFC ile kullanılan çıkarılabilir bir kimlik doğrulayıcı | YubiKey ve diğer FIDO2 anahtarları |

Üçü de **P-256** eğrisi üzerindeki WebAuthn kimlik bilgileridir. Safe açısından
aralarında fark yoktur: her biri, imzasını Safe'in geçiş anahtarı modülünün zincir
üstünde aynı şekilde kontrol ettiği bir sahiptir. (İlk anahtarı Safe'in paylaşılan
imzalayıcısı doğrular; her ek anahtarı ise cüzdan bir zincirde ilk kez dağıtıldığında
Safe'in fabrikasının oluşturduğu küçük bir imzalayıcı sözleşme doğrular.)

Hangi uygulama hangi türleri kullanabilir:

| Uygulama | Bu cihaz | Yakındaki telefon (QR) | Güvenlik anahtarı |
| --- | --- | --- | --- |
| Web cüzdanı, tarayıcı uzantısı | Evet | Evet | Tarayıcı üzerinden USB ya da NFC |
| Masaüstü (macOS, Windows, Linux) | macOS ve Windows | Evet | USB |
| Android | Evet (Google Play hizmetleriyle) | Evet | USB |
| iOS | Evet | Evet | USB-C ya da Lightning YubiKey, ürün yazılımı 5.8 ya da sonrası |

Masaüstü uygulamasının "bu cihaz" seçeneği (Touch ID, Windows Hello) ve genel olarak
Windows desteği yenidir ve diğer yollardan daha az test edilmiştir; bugün orada
güvenilir seçim bir telefon ya da bir güvenlik anahtarıdır.

Bir güvenlik anahtarı yalnızca yedek değil, **ilk** imzalayıcınız da olabilir.
Cüzdanınızın bir Apple ya da Google hesabına hiç bağlı olmamasını tercih
ediyorsanız, cüzdanı **iki** güvenlik anahtarıyla oluşturun ve birini güvenli bir
yerde saklayın. (Tek anahtarı hiçbir yere eşitlenmeyen bir cüzdan oluşturulamaz:
uygulama ikinci bir anahtar ister, çünkü o tek cihazı kaybetmek cüzdanı kaybetmek
olurdu.)

## Neden oluştururken seçiliyorlar

İnsanları şaşırtan kısım bu; o yüzden özür dilemek yerine mekanizmayı anlatalım.

Cüzdan adresiniz, sahip kümesinden **türetilir**. Vela onu, zincirde hiçbir şey
dağıtılmadan önce, her imzalayıcının açık anahtarını içeren Safe kurulum verisinden
`CREATE2` ile hesaplar. Henüz var olmayan bir adrese para alabilmenizi sağlayan şey de
budur.

Adres açısından sonuç basit bir aritmetiktir: **farklı bir anahtar kümesi, farklı bir
adrestir**. Sonradan başka bir imzalayıcı eklemek cüzdanınızı genişletmez; yeni bir
adreste, içinde paranızın hiç bulunmadığı yeni bir cüzdan hesaplar.

Yani "sonradan anahtar ekleyebilir miyim?" sorusunun iki dürüst yanıtı var:

- **Cüzdana para koymadan önce**: evet — adres henüz hiçbir şeye bağlanmadı; cüzdanı
  istediğiniz anahtarlarla yeniden oluşturun.
- **Para koyduktan sonra**: adres, paranızın bulunduğu yerdir. Safe'in kendisi,
  cüzdanınızın zaten dağıtılmış olduğu bir zincirde sahipleri değiştirebilir — ama
  henüz dağıtılmadığı her zincirde aynı adres hâlâ ilk anahtarları temsil eder;
  böylece sahip kümeleri zincirden zincire birbirinden ayrışır. Bunları zincirler
  arasında eşit tutmak mümkün — bazı akıllı cüzdanlar bunu yapıyor — ama Vela bunu
  geliştirmedi, bu yüzden sahip değişikliği sunmuyor. Anahtar kümesini oluştururken
  planlayın.

## Bu sizi aslında neye karşı koruyor

**Cihaz kaybetmek.** Birden fazla imzalayıcınız varsa kaybolan bir telefon yalnızca
bir zahmettir: başka bir anahtar imzalar. Tek bir imzalayıcınız varsa ve işletim
sistemi eşitlemesi kapalıysa, kaybolan telefon kaybolan cüzdan demektir — "geçiş
anahtarınız otomatik eşitlenir" cümlesinin, sizin adınıza verebileceğimiz bir garanti
değil, sizin kontrol ettiğiniz bir ayarın tarifi olmasının nedeni de bu.

**Artık güvenmediğiniz bir platform hesabı.** Geçiş anahtarınız iCloud Anahtar
Zinciri'nde ya da Google Şifre Yöneticisi'nde duruyorsa, o hesabı kontrol eden kişi
onu kullanabilir. Güvenlik anahtarı ise sizde durur ve hiçbir yere eşitlenmez.

Sizi neye karşı **korumadığına** gelince, çünkü `1-of-n` iki yönlü keser: ikinci bir
anahtar eklemek ikinci bir kilit değil, ikinci bir *giriş yolu* ekler.
İmzalayıcılarınızdan herhangi birini ele geçiren kişi tek başına imzalayabilir. Daha
çok anahtar, kayba karşı daha çok dayanıklılık ve hırsızlığa karşı daha geniş bir
yüzey demektir; tartmanız gereken denge bu ve karar sizin.

## Bir anahtar ele geçirilmiş olabilirse

Bir anahtar kaldırılamaz. Anahtarlarınızdan biri başkasının eline geçmiş olabilirse —
kaybolan kilidi açık bir telefon, birinin gördüğü bir parola kodu, artık kontrol
etmediğiniz bir Apple ya da Google hesabı — güvendiğiniz anahtarlarla oluşturulmuş
**yeni bir cüzdana her şeyi taşıyın**. Eski adres, sonradan herhangi birinin ona
gönderdiği paralar dahil, her ağda o anahtar tarafından harcanabilir olmaya devam
eder.

## Kurtarmak ile eklemek

Bunlar farklı şeyler ve belgeler ikisini ayrı tutuyor:

- [Kurtarma ve giriş](/tr/docs/recovery) — elinizde zaten olan bir anahtarla, yeni bir
  cihazda mevcut bir cüzdana geri dönmek.
- Bu sayfa — hangi anahtarların var olacağına baştan karar vermek.
