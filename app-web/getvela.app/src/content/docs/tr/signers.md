---
title: İmzalayıcılar ve güvenlik anahtarları
description: Bir Vela cüzdanının yedi imzalayıcıya kadar sahibi olabilir — geçiş anahtarları, yakındaki bir cihaz ya da YubiKey sınıfı bir güvenlik anahtarı — ve herhangi biri tek başına imzalar. Bunlar cüzdan oluşturulurken seçilir; bu sayfa bunun kaldırmayı unuttuğumuz bir kısıt olmadığını anlatıyor.
---

# İmzalayıcılar ve güvenlik anahtarları

Vela cüzdanı bir Safe'tir ve Safe'in sahipleri olur. Sizinkinin **yediye kadar**
sahibi olabilir, eşik ise **bir**: tek bir imzalayıcı bir işlemi kendi başına
yetkilendirebilir. Bu, `1-of-n` diye yazılır.

## Ne imzalayıcı olabilir

Üç tür var ve bunları serbestçe karıştırabilirsiniz:

| Yöntem | Nedir | Tipik örnek |
| --- | --- | --- |
| **Platform** | Kullandığınız cihazın içindeki kimlik doğrulayıcı | Bu telefondaki ya da dizüstündeki Face ID / Touch ID; iCloud Anahtar Zinciri veya Google Şifre Yöneticisi ile eşitlenir |
| **Yakındaki cihaz** | Bir kod okutarak eriştiğiniz başka bir cihaz | Telefonunuzun, WebAuthn hibrit taşıması üzerinden masaüstünüz için imzalaması |
| **Güvenlik anahtarı** | USB ya da NFC ile takılan çıkarılabilir kimlik doğrulayıcı | YubiKey ve diğer FIDO2 anahtarları |

Üçü de **P-256** eğrisi üzerindeki WebAuthn kimlik bilgileri. Safe açısından
aralarında fark yok: her biri, imzasını zincir üstü WebAuthn doğrulayıcısının aynı
şekilde kontrol ettiği bir sahiptir.

Bir güvenlik anahtarı yalnızca yedek değil, **ilk** imzalayıcınız da olabilir.
Cüzdanınızın bir Apple ya da Google hesabına hiç bağlı olmamasını tercih
ediyorsanız bunu sağlayan ayar tam olarak budur — oluştururken bir YubiKey
kaydedin ve onunla imzalayın.

## Neden oluştururken seçiliyor

İnsanları şaşırtan kısım bu, o yüzden özür yerine mekanizmayı anlatalım.

Cüzdan adresiniz, sahip kümesinden **türetilir**. Vela onu, zincirde hiçbir şey
kurulmadan önce Safe kurulum verisinden — ki bu veri her imzalayıcının genel
anahtarını içerir — `CREATE2` ile hesaplar. Henüz var olmayan bir adrese para
alabilmenizi sağlayan şey de bu.

Sonuç bir politika değil, aritmetik: **farklı bir anahtar kümesi, farklı bir
adrestir**. Sonradan sekizinci bir imzalayıcı eklemek cüzdanınızı genişletmez;
yeni bir adreste, içinde paranızın hiç bulunmadığı yeni bir cüzdan hesaplar.

Yani "sonradan anahtar ekleyebilir miyim?" sorusunun iki dürüst yanıtı var:

- **Para yatırmadan önce**: evet — adres henüz hiçbir şeye bağlanmadı, o yüzden
  cüzdanı istediğiniz anahtarlarla yeniden oluşturun.
- **Para yatırdıktan sonra**: adres, paranızın bulunduğu yer. Kurulmuş bir Safe'in
  sahiplerini değiştirmek, Vela'nın şu anda sunmadığı bir Safe işlemi. Anahtar
  kümesini oluştururken planlayın.

## Bu sizi neye karşı koruyor

**Cihaz kaybetmek.** Birden fazla imzalayıcınız varsa kaybolan telefon bir
zahmettir: başka bir anahtar imzalar. Tek bir imzalayıcınız varsa ve işletim
sistemi eşitlemesi kapalıysa kaybolan telefon kaybolan cüzdandır — "geçiş
anahtarınız otomatik eşitlenir" cümlesinin, sizin adınıza verebileceğimiz bir
garanti değil, sizin kontrol ettiğiniz bir ayarın tarifi olmasının nedeni de bu.

**Artık güvenmediğiniz bir platform hesabı.** Geçiş anahtarınız iCloud Anahtar
Zinciri'nde ya da Google Şifre Yöneticisi'nde duruyorsa, o hesabı kontrol eden
kişi onu kullanabilir. Güvenlik anahtarı ise sizde durur ve hiçbir yere
eşitlenmez.

Sizi neye karşı **korumadığına** gelince, çünkü `1-of-n` iki yönlü çalışır: ikinci
bir anahtar eklemek ikinci bir kilit değil, ikinci bir *giriş yolu* ekler.
İmzalayıcılarınızdan herhangi birini ele geçiren, tek başına imzalayabilir. Daha
çok anahtar, kayba karşı daha çok dayanıklılık ve hırsızlığa karşı daha geniş bir
yüzey demek; takas bu ve kararı size ait.

## Kurtarmak ile eklemek

Bunlar farklı şeyler ve belgeler ikisini ayrı tutuyor:

- [Kurtarma ve giriş](/tr/docs/recovery) — elinizde zaten olan bir anahtarla,
  yeni bir cihazda mevcut cüzdanınıza dönmek.
- Bu sayfa — hangi anahtarların var olacağına baştan karar vermek.
