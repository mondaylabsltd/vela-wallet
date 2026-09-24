---
title: İmza sayfasını kendiniz barındırın
description: "Bir işlemi kendisi çözen ve geçiş anahtarınızla imzalayan, hiçbir bağımlılığı olmayan sayfa ve Chrome uzantısı — kendi kopyanızı nasıl çalıştıracağınız ve hangi kopyanın cüzdanınız için imzalayabileceği."
source: c81389ef7aa2
---

# İmza sayfasını kendiniz barındırın

Vela her işlemi siz onaylamadan önce çözer ve bu çözümleme dürüst bir iştir — ama
işlemi oluşturan uygulamanın kendisinin yaptığı bir iştir. Uygulamaya ya da size
ulaşma yoluna müdahale edilmişse, size bir şey gösterip başka bir şey imzalatabilir.
[Bybit'te](/tr/docs/bybit-attack) tam olarak bu oldu.

İmza sayfası, bunu ikiye bölmek için var: işlem bir yerden gelir, kontrol ve imza ise
sizin denetiminizdeki başka bir yerde gerçekleşir.


Bir imza isteği aldığında, onunla birlikte gelen özete güvenmez. Ham calldata'yı
kendisi çözer, kendi özet değerini (digest) hesaplar, imzanın gerçekte neye yetki
vereceğini size gösterir ve ancak ondan sonra geçiş anahtarınızı ister.

Derleme adımı olmadığı için okuduğunuz dosyalar, çalışan dosyalardır. Klasörü depoyla
karşılaştırıp neyi sunduğunuzu kesin olarak bilebilirsiniz.

## Hangi kopya cüzdanınız için imzalayabilir

Bir geçiş anahtarı, oluşturulduğu alan adına bağlıdır. Vela anahtarlarınız
`getvela.app` altında kayıtlıdır ve tarayıcı onları yalnızca bağlı olan tarafı
(relying party) `getvela.app` olan bir sayfaya sunar. Kendi kopyanızı çalıştırmanın
hangi yolunun işinize yarayacağını bu tek kural belirler.

**Kendi alan adınızda ya da localhost'ta bir sayfa olarak.** HTTPS üzerinden (ya da
localhost'tan) sunulduğunda sayfanın bağlı olan tarafı kendi ana makine adıdır
(hostname) — yani `getvela.app` altında kayıtlı anahtarlarla değil, _o_ ana makine adı
altında kayıtlı anahtarlarla imzalayabilir. Bu da onu bütün süreci uçtan uca denemenin,
masaüstü akışını çalıştırmanın ve anahtarı kendi alan adınızda oluşturulmuş bir cüzdan
için imzalamanın doğru yolu yapar. Mevcut bir `getvela.app` cüzdanı için imzalamanın
yolu değildir.

```sh
cd app-web/trusted-signer
python3 -m http.server 8080   # → http://localhost:8080
```

Uygulamadaki bütün yollar göreli olduğundan, mevcut bir sunucudaki bir alt dizin de
çalışır; `index.html` dosyasını doğrudan diskten (`file://`) açmak da etrafa bakmak
için yeterlidir — kaynak (origin) olmadığı için bağlı olan taraf da yoktur ve hiçbir
şey imzalanamaz.

## İmzalamadan önce ne yapar

- **İşlemi kendisi çözer.** Çağrının ne yaptığını, kime ve ne kadar olduğunu
  calldata'dan okur — bir toplu işlemin içine yerleştirilmiş çağrılar dahil.
- **Yalnızca kendi hesapladığı bir özeti imzalar.** EIP-191, EIP-712, SafeOp ve
  SafeMessage özetleri sayfanın içinde hesaplanır ve cüzdanın kullandığı kodun aynısı
  olan `vela-core` ile çapraz kontrol edilir. Hesaplayamadığı bir özet, imza değil
  rettir.
- **İşlemin istenen işlem olduğunu kontrol eder.** Sitenin istediği çağrının,
  imzalanan işlemin içinde gerçekten bulunması gerekir.
- **"Sınırsız" düzeydeki bir onayı reddeder.** Uyarı değil — ret; bunun yerine ne
  yapılacağına dair bir yönlendirmeyle.
- **Bir şeyi okuyamadığında bunu söyler;** arkasında duramayacağı dost görünümlü bir
  özet göstermez.
- **Hesabın adresini ve identicon'unu gösterir** ve imzayı isteyenin verdiği bir alıcı
  adını göstermez. İsteyenin kontrol ettiği her şey ya çıkarılır ya da ona ait olarak
  etiketlenir.

## Bilerek sahip olmadığı şeyler

- **Düzenleyici yok.** İstek geldiği anda sabittir: ya imzalarsınız ya imzalamazsınız.
  Bir ücret seçici ya da harcama izni düzenleyicisi calldata'yı yeniden yazardı; bu
  sayfanın önlemek için var olduğu hastalık da tam olarak bu.
- **Anahtar oluşturma yok.** İmza sayfası geçiş anahtarı oluşturamaz. Bir tane
  oluşturmak, başka bir hesap oluşturmak olurdu.
- **Ağdan veri yok.** Gösterdiği ya da imzaladığı hiçbir şey ağdan çekilmez. Yüklediği
  tek şey, Vela'nın zincir verisi sunucusundan görsel olarak alınan token logolarıdır;
  yüklenemezlerse yerlerini bir harf alır.

## Bir istek ona nasıl ulaşır

| İsteyen                                      | Kanal                                                                      |
| -------------------------------------------- | -------------------------------------------------------------------------- |
| Aynı tarayıcıdaki bir sayfa                  | `postMessage`                                                              |
| Aynı tarayıcıdaki bir sayfadan uzantıya      | Uzantı portu                                                               |
| Aynı makinedeki bir masaüstü uygulaması      | URL parçası + loopback geri çağrısı (`samples/` içinde örnek var; Vela masaüstü uygulaması henüz kullanmıyor) |
| Bir telefon ya da başka bir bilgisayar       | Bluetooth LE (protokol uygulandı; radyo kısmı gerçek donanımda henüz denenmedi) |

Aktarım biçimi, özetler ve ekrandaki her öğenin nereden geldiğini gösteren bir tablo,
kodun yanındaki `PROTOCOL.md` dosyasında.

## Nereye oturuyor

Uygulamalar isteklerini ona iletebildiğinde amaçlanan kullanım basit: hesabınız
kaybetmeyi göze alamayacağınız bir parayı tuttuğu günden itibaren her imza, kodunu
kendiniz yüklediğiniz bir sayfadan geçer. Yalnızca büyük tutarlar için değil — küçük
bir onay bile bir hesabı boşaltmaya yetecek yetkiyi devredebilir. O zamana kadar bu
sayfa, o ikinci görüşün tam olarak nasıl çalışacağını okumanın ve denemenin bir yolu.
