---
title: İmza sayfasını kendin barındır
description: İşlemi kendisi çözen ve geçiş anahtarınızla imzalayan, hiçbir bağımlılığı olmayan sayfa ve Chrome uzantısı — kendi kopyanızı nasıl çalıştıracağınız ve hangi kopyanın cüzdanınız için imzalayabileceği.
---

# İmza sayfasını kendin barındır

Vela her işlemi siz onaylamadan önce çözer ve bu çözümleme dürüst bir iştir — ama
işlemi oluşturan uygulamanın yaptığı bir iştir. Uygulamaya ya da size ulaşma
yoluna müdahale edilmişse, size bir şey gösterip başka bir şey imzalatabilir.
[Bybit'te](/tr/docs/bybit-attack) olan tam olarak buydu.

İmza sayfası, bunu ikiye bölmek için var: işlem bir yerden gelir, kontrol ile imza
ise sizin denetlediğiniz başka bir yerde olur.

## Nedir

Tek bir klasör — depodaki `app-web/trusted-signer` — ve bu klasör hem bir web sayfası
hem bir Chrome uzantısı. Saf HTML, CSS ve JavaScript: framework yok, bundler yok,
derleme adımı yok, bağımlılık yok ve kendi başına hiçbir ağ isteği yok.

Bir imza isteği geldiğinde, yanında gelen özete güvenmez. Ham calldata'yı kendisi
çözer, kendi özetini hesaplar, imzanın gerçekte neye yetki vereceğini gösterir ve
ancak ondan sonra geçiş anahtarınızı ister.

Derleme adımı olmadığı için okuduğunuz dosyalar, çalışan dosyalardır. Klasörü
depoyla karşılaştırıp neyi sunduğunuzu bilebilirsiniz.

## Hangi kopya cüzdanınız için imzalayabilir

Geçiş anahtarı, oluşturulduğu alan adına bağlıdır. Vela anahtarlarınız
`getvela.app` altında kayıtlı ve tarayıcı bunları yalnızca ilgili tarafı
`getvela.app` olan bir sayfaya sunar. Kendi kopyanızı çalıştırmanın hangi yolunun
işinize yarayacağını belirleyen tek kural bu.

**Chrome uzantısı olarak — mevcut cüzdanınızla kullanacağınız yol budur.**
Uzantının ilgili tarafı, klasör nereden gelmiş olursa olsun `getvela.app`'tir; yani
mevcut anahtarlarınız onda imzalayabilir, çalışan kod ise yüklediğiniz ve
incelediğiniz klasördür.

1. `chrome://extensions` adresini açıp **Geliştirici modu**'nu açın.
2. **Paketlenmemiş öğe yükle** deyip `app-web/trusted-signer` klasörünü seçin.
3. Araç çubuğundaki simge sayfayı bir sekmede açar.

**Kendi alan adınızda ya da localhost'ta bir sayfa olarak.** HTTP(S) üzerinden
sunulduğunda sayfanın ilgili tarafı kendi alan adıdır — yani `getvela.app` altında
değil, _o_ alan adı altında kayıtlı anahtarlarla imzalayabilir. Bu da onu bütün
töreni baştan sona denemenin, masaüstü akışını çalıştırmanın ve anahtarı kendi
alan adınızda oluşturulmuş bir cüzdan için imzalamanın doğru yolu yapar. Mevcut bir
`getvela.app` cüzdanı için imzalamanın yolu değildir.

```sh
cd app-web/trusted-signer
python3 -m http.server 8080   # → http://localhost:8080
```

Uygulamadaki bütün yollar göreli, yani var olan bir sunucudaki bir alt dizin de
çalışır; `index.html`'i doğrudan diskten açmak (`file://`) da etrafa bakmak için
yeterlidir — kaynak olmadığı için ilgili taraf da yoktur ve hiçbir şey imzalanamaz.

## İmzalamadan önce ne yapıyor

- **İşlemi kendisi çözüyor.** Çağrının ne yaptığını, kime ve ne kadar olduğunu
  calldata'dan okur — bir toplu işlemin içine gömülmüş çağrılar dahil.
- **Yalnızca kendi hesapladığı bir özeti imzalıyor.** EIP-191, EIP-712, SafeOp ve
  SafeMessage özetleri sayfanın içinde hesaplanır ve cüzdanın kullandığı kodun
  aynısıyla, `vela-core` ile karşılıklı kontrol edilir. Hesaplayamadığı bir özet,
  imza değil rettir.
- **İşlemin istenen işlem olduğunu kontrol ediyor.** Sitenin istediği çağrının,
  imzalanan işlemin içinde gerçekten bulunması gerekir.
- **Sınırsız onayı reddediyor.** Uyarı değil — ret; üstelik bunun yerine ne
  yapılacağını da söyleyerek.
- **Bir şeyi okuyamadığında bunu söylüyor;** arkasında duramayacağı dost görünümlü
  bir özet göstermiyor.
- **Hesabın adresini ve identicon'unu gösteriyor** ve imzayı isteyenin verdiği bir
  alıcı adını göstermiyor. İsteyenin kontrol ettiği her şey ya atılır ya da ona ait
  olarak etiketlenir.

## Bilerek sahip olmadığı şeyler

- **Düzenleyici yok.** İstek geldiği anda sabittir: ya imzalarsınız ya
  imzalamazsınız. Bir ücret seçici ya da yetki düzenleyici calldata'yı yeniden
  yazardı; bu sayfanın var olma nedeni de tam olarak o hastalığı önlemek.
- **Anahtar oluşturma yok.** İmza sayfası geçiş anahtarı oluşturamaz. Oluşturmak,
  başka bir hesap oluşturmak olurdu.
- **Ağ isteği yok.** Getirilecek bir şey yoksa, araya girilecek bir şey de yoktur.

## Bir istek ona nasıl ulaşıyor

| İsteyen | Kanal |
| -------------------------------------------- | -------------------------------------------------------------------------- |
| Aynı tarayıcıdaki bir sayfa | `postMessage` |
| Aynı tarayıcıdaki bir sayfadan uzantıya | Uzantı portu |
| Aynı makinedeki bir masaüstü uygulaması | URL parçası + loopback geri çağrısı |
| Bir telefon ya da başka bir bilgisayar | Bluetooth LE (protokol yazıldı; radyo gerçek donanımda henüz denenmedi) |

Tel biçimi, özetler ve ekrandaki her öğenin nereden geldiğini gösteren tablo,
kodun yanındaki `PROTOCOL.md` dosyasında.

## Ne zaman kullanmalı

Hesap, kaybetmeye üzüleceğiniz bir parayı tuttuğu gün — ve o günden sonra her imza
için. Yalnızca büyük tutarlar için değil: küçük bir onay bile hesabı boşaltmaya
yetecek yetkiyi devredebilir. Özel günlere saklanan bir imzalama alışkanlığı,
gerektiği gün yerinde olmaz.
