---
title: Kendi sunucunuzda barındırma kılavuzu
description: "Vela'nın sizin için çalıştırdığı her şey, her parçanın ne yaptığı ve onu kendinizinkiyle nasıl değiştireceğiniz — relay, açık anahtar dizini, zincir verisi, döviz kurları ve uygulamalar — ayrıca değiştiremeyeceğiniz tek şey ve getvela.app olmadan nasıl idare edeceğiniz."
source: 24c423d10c89
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Kendi sunucunuzda barındırma kılavuzu

Paranız zincir üstünde, anahtarlarınızın kontrol ettiği bir Safe sözleşmesinde duruyor.
Vela'nın çalıştırdığı hiçbir şey onu taşıyamaz. Vela'nın çalıştırdığı şey, cüzdanı
kullanışlı kılan düzenektir: işlemlerinizi zincire gönderen bir relay, yeni bir cihazın
cüzdanınızı bulmasına yardım eden bir dizin, bir zincir verisi dizini, bir döviz kuru
kaynağı ve uygulamaların kendileri.

Bu sayfa bu parçaların her birini, onsuz neyin bozulduğunu ve kendinizinkini nasıl
çalıştıracağınızı listeliyor. Değiştiremeyeceğiniz tek parçayı — geçiş
anahtarlarınızın ait olduğu alan adını — ve getvela.app ortadan kalkarsa ne
yapacağınızı da anlatıyor. Baştan bir sınır: relay bugün zincir verisini Vela'nın
sunucusundan okuyor; yani hiç Vela altyapısı olmadan gönderim yapmak, relay'in kodunda
bir adresi değiştirmeyi gerektiriyor.

<Callout type="info" title="Bu sayfa kimler için">
Terminal, Docker ya da Cloudflare Workers kullanmaya ve bir zincirdeki bir adrese para
yüklemeye alışkın olmalısınız. Vela'yı günlük kullanmak için buradaki hiçbir şeye gerek
yok.
</Callout>

## Harita

| Parça | Ne yapar | Vela'nın varsayılanı | Değiştirebilir misiniz? | Olmazsa |
| --- | --- | --- | --- | --- |
| **Relay** | İmzalı işleminizi alır, gas bedelini öder, zincire gönderir, imzaladığınız ücreti tahsil eder | `vela-relay-cf.getvela.app` | Evet — [vela-relay](#relay) çalıştırın ve cüzdanı ona yönlendirin | Gönderim yapamazsınız |
| **Açık anahtar dizini** | Yeni bir cüzdanın anahtarlarını zincire kaydeder; "bu anahtar hangi cüzdana ait?" sorusunu yanıtlar | `p256-index-v2.getvela.app` | Evet — [p256-index](#index) çalıştırın | Yeni cüzdan oluşturulamaz; giriş, zinciri doğrudan okumaya geri döner |
| **Kayıt defteri sözleşmesi** | Her cüzdanın anahtarlarının kalıcı, herkese açık kaydı | Gnosis'te `0x94fD…1EA9` | Gerek yok — sahibi yok; cüzdan onu doğrudan okur | — |
| **Zincir verisi** | Ağ ayrıntıları, token listeleri, logolar, açık imzalama tanımlayıcıları | `ethereum-data.getvela.app` | Evet — [ethereum-data](#chain-data) çalıştırın | Token listesi ve logo olmaz; daha az işlem çözülür; ağ ekleme başarısız olur |
| **Döviz kurları** | Görüntüleme para biriminizdeki itibari para karşılıkları | `vela-currency.getvela.app` | Evet — [vela-currency](#exchange-rates) ya da Frankfurter uyumlu herhangi bir kaynak çalıştırın | Uygulamalar mümkün olduğunda zincir üstü Chainlink kurlarına geri döner (masaüstü USD gösterir) |
| **RPC düğümleri** | Bakiyeleri okumak, işlemleri simüle etmek | Her ağ için herkese açık uç noktalar | Evet — ağ başına, Ayarlar → Ağlar bölümünden | Vela uç noktalar arasında otomatik geçiş yapar |
| **Uygulamalar** | Cüzdanın kendisi | wallet.getvela.app, sürüm derlemeleri | Evet — [kendiniz derleyin](#web-app) | — |
| **getvela.app** | Geçiş anahtarlarınızın ait olduğu alan adı | — | **Hayır** — [aşağıya](#if-getvela-app-disappears) bakın | — |

Vela'ya ait olmayan birkaç üçüncü taraf servisle de bağlantı kurulur: bir işlemi
çözerken son çare olarak kullanılan herkese açık işlev seçici veritabanları (sourcify,
openchain, 4byte), güvenlik anahtarınızın modelini adlandıran kimlik doğrulayıcı dizini
ve bir telefonla QR kod okutarak imzaladığınızda Apple'ın ve Google'ın tünel
sunucuları.

## Değiştiremeyeceğiniz tek şey: geçiş anahtarının alan adı

<span id="if-getvela-app-disappears"></span>

Bir geçiş anahtarı, oluşturulduğu web sitesine aittir. Vela'nın anahtarları
`getvela.app` için oluşturulur. Tarayıcılar onları yalnızca getvela.app ya da alt alan
adlarındaki sayfalara (veya getvela.app'in ilişkili ilan ettiği kaynaklara) sunar;
telefonun yerleşik geçiş anahtarları da yalnızca getvela.app'in kefil olduğu
uygulamalarda çalışır. Tarayıcının dışında kural daha gevşektir: Chrome, getvela.app
için izni olan bir uzantının bunları kullanmasına izin verir; bilgisayarınızdaki bir
program da bir güvenlik anahtarından ya da telefondan doğrudan getvela.app imzası
isteyebilir — kendi derlediğiniz uygulamalar böyle çalışır ve çalıştırdığınız
yazılımın önemli olmasının nedeni de budur. Bundan iki sonuç çıkar.

**Web cüzdanının kendi alan adınızdaki bir kopyası başka bir cüzdandır.**
`wallet.example.com` adresinden sunulan aynı kod, `wallet.example.com` için geçiş
anahtarları oluşturur — yeni anahtarlar, dolayısıyla yeni bir adres.
wallet.getvela.app'te oluşturulmuş bir cüzdan için imzalayamaz. Bu kopya yine de işe
yarar: orada oluşturduğunuz bir cüzdan için ya da bütün yığını sıfırdan kendiniz
çalıştırmak için.

**Mevcut bir cüzdan için, getvela.app çevrimdışıysa ya da tamamen ortadan kalktıysa
bunlar çalışmaya devam eder:**

| Giriş yolu | Kullanabileceği anahtarlar | Nereden edinilir |
| --- | --- | --- |
| **Vela tarayıcı uzantısı** (Chromium tabanlı tarayıcılar: Chrome, Edge, Brave) | Tarayıcının erişebildiği her anahtar: bu cihazın geçiş anahtarı, bir USB güvenlik anahtarı (bilgisayar destekliyorsa NFC), QR kodla bağlanan bir telefon | [GitHub](https://github.com/mondaylabsltd/vela-wallet/releases)'daki bir sürüm zip'i ya da [kendiniz derleyin](#web-app) |
| **Kendi derlediğiniz bir masaüstü ya da telefon uygulaması** | QR kodla bağlanan bir telefon ve USB güvenlik anahtarları | [Kendiniz derleyin](#web-app) |
| **Mağaza sürümleri ve onaylanmış (notarized) masaüstü uygulamaları** | QR kodla bağlanan telefon ve güvenlik anahtarları her zaman; "bu cihaz" geçiş anahtarları ise yalnızca işletim sistemi uygulamayı getvela.app'e karşı hâlâ doğrulayabildiği sürece | GitHub sürümleri (mağazalar sonra) |

Uzantı `getvela.app` anahtarlarını kullanabilir, çünkü Chrome bir site için izni olan
bir uzantının o sitenin geçiş anahtarlarını kullanmasına izin verir. Tarayıcı bu izni
yerel olarak kontrol eder; bunun çalıştığını ölçtük, ama alan adı gerçekten çevrimdışıyken
henüz denemedik. Kendi derlediğiniz bir uygulama bir telefonu ya da güvenlik anahtarını
kullanabilir, çünkü Vela onlarla doğrudan konuşur; telefonun kendi geçiş anahtarı ("bu
cihaz") ise uygulamanın Vela tarafından imzalanmış olmasını gerektirir, sizinki öyle
değil.

[İmza sayfası](/tr/docs/clear-signing-self-host) tek başına bir giriş yolu değildir:
başka bir programın kendisine gönderdiği istekleri imzalar ve henüz hiçbir Vela
uygulaması ona istek göndermiyor.

<Callout type="warning" title="Alan adını kontrol eden, imza isteyebilir">
getvela.app'ten ya da alt alan adlarından birinden sunulan her sayfa — ya da gelecekte
alan adını kim kontrol ederse onun sunduğu sayfalar — anahtarlarınızdan imza
isteyebilir ve sistem istemi işlemi değil, "getvela.app"i gösterir. Geçiş anahtarları
her yerde böyle çalışır. Vela'nın web sitesi bu yüzden kendi sayfalarının geçiş anahtarı
kullanmasını yasaklar. Uzantının ve kendi derlediğiniz uygulamaların önemli olmasının
nedeni de bu: kendi kodlarını taşırlar; ama varsayılan olarak tanımlayıcıları yine
getvela.app altından alır ve oradaki servisleri kullanırlar.
</Callout>

## Cüzdanı kendi servislerinize yönlendirin

Her uygulamada **Ayarlar → Gelişmiş → Servis Uç Noktaları** (masaüstünde **Ayarlar →
Servis Uç Noktaları**) altında dört alan vardır: zincir verisi, geçiş anahtarı dizini,
Vela relay'i ve itibari para kurları. Siz değiştirene kadar her alan Vela'nın
varsayılanını gösterir; **Varsayılanlara Sıfırla** dördünü birden geri yükler. Relay,
dizin ve zincir verisi için cüzdan `/api/health` adresini çağırır ve bir rozet gösterir;
rozet yalnızca uç nokta doğru servisin adını verip `status: "ok"` bildirdiğinde yeşil
olur. Yazdığınızı her durumda kaydeder — yeşili bekleyin.

| Servis | `/api/health` içindeki `service` |
| --- | --- |
| Relay | `vela-relay` |
| Açık anahtar dizini | `webauthn-p256-publickey-registry` |
| Zincir verisi | `ethereum-data` |
| Döviz kurları | adla kontrol edilmez — USD tabanlı bir kur listesi döndürmelidir |

Uygulamaların bu ayarlara bugün ne kadar uyduğu:

| Uygulama | Servis uç noktaları | Ağ başına RPC |
| --- | --- | --- |
| Web ve uzantı | Zincir verisi, relay ve itibari para kurları. Geçiş anahtarı dizini adres adlarını aramak için kullanılır; ama cüzdan oluşturma ve giriş hâlâ Vela'nın dizinini kullanır | Evet |
| Masaüstü | Dördü de; yeni bir geçiş anahtarı dizini, yeniden başlattıktan ya da oturumu kapattıktan sonra geçerli olur | Evet |
| Android | Dördü de; yalnızca adresler için ad arama hâlâ Vela'nın dizinine sorar | Evet |
| iOS | **Henüz değil**: sayfa yer tutucu değerler gösterir ve kaydetmez. Varsayılana ulaşılamadığında geçiş anahtarı dizini giriş ekranında değiştirilebilir | Salt okunur |

Bu eksikler birer hatadır ve takip ediliyor.

## Kendi relay'inizi çalıştırın

<span id="relay"></span>

Relay, [vela-relay](https://github.com/mondaylabsltd/vela-relay)'dir (Rust, MIT). Tek bir
dağıtım bütün zincirlere hizmet verir: cüzdan `https://your-relay/<chainId>` adresini
çağırır. vela-relay olmak zorundadır — cüzdan ücret teklifini, genel ERC-4337
bundler'larının uygulamadığı, Vela'ya özgü bir yöntemle ister.

**İhtiyacınız olanlar**

- Ya Docker ile birlikte zaten çalıştırdığınız bir Redis ve bir
  [Iggy](https://iggy.apache.org) sunucusu ya da **Workers Paid** planında bir
  Cloudflare hesabı ve makinenizde Node.js ile bir Rust araç zinciri
  (`wasm32-unknown-unknown` hedefiyle).
- Bir `OPERATOR_SECRET` (hex, en az 32 bayt). Bundan bir kasa adresi ve bir relayer
  adresleri havuzu türetilir; hepsi her zincirde aynıdır. Gizli tutun: relay'in
  paralarını o kontrol eder.
- Hizmet vermek istediğiniz her zincirde gas: zincirin coinini (Tempo'da pathUSD) kasa
  adresinize gönderin. Kasa, relayer'ları kendisi doldurur.

**Docker**

```sh
git clone https://github.com/mondaylabsltd/vela-relay
cd vela-relay
cp .env.example .env
# .env içinde: VELA_RELAY_IGGY_URL, VELA_RELAY_REDIS_URL, OPERATOR_SECRET
# ve güvendiğiniz bir sürüm imajına ayarlanmış VELA_RELAY_IMAGE (bkz. docs/docker.md)
docker compose pull relay
docker compose up -d --no-build
curl --fail http://127.0.0.1:4567/readyz
```

Yayımlanmış imajı tercih edin: `docker compose up --build` ile kaynak koddan derlemek,
mevcut Dockerfile ile başarısız olabilir. Docker olmadan
`cargo run --release --bin vela-relay` onu doğrudan çalıştırır.

**Cloudflare Workers**

```sh
cd vela-relay/vela-relay-cf
npx wrangler queues create vela-relay-ops
npx wrangler queues create vela-relay-dlq
npx wrangler secret put OPERATOR_SECRET
npx wrangler deploy
```

**Kontrol edin**

```sh
curl https://your-relay/api/health        # {"service":"vela-relay","status":"ok",…}
curl https://your-relay/v1/treasury/100   # Gnosis'teki kasa adresiniz ve gas gerekip gerekmediği
```

Ardından `https://your-relay` adresini **Vela Relay** alanına girin.

**Bilmeniz gerekenler**

- Cüzdanın ödediği ücret sizin kasanıza gider. Cüzdan, hangi relay'i kullanırsanız
  kullanın ücreti aynı şekilde hesaplar (bkz. [ağlar ve ücretler](/tr/docs/networks-and-fees)).
- Relay'i değiştirmeden önce eklediğiniz özel bir ağ, eklendiği sıradaki relay
  adresini korur.
- Relay, her zincirin ayrıntılarını ve stabilcoin listesini
  `ethereum-data.getvela.app` adresinden okur. Bu adres bugün relay'in koduna sabit
  yazılmıştır (`src/utils/rpc.rs` ve `vela-relay-cf/src/arms/market.rs`); onu
  değiştirmek, bu iki satırı değiştirip relay'i kendiniz derlemek demektir.

## Kendi açık anahtar dizininizi çalıştırın

<span id="index"></span>

Dizin, [p256-index](https://github.com/mondaylabsltd/p256-index)'tir (Rust). Bir cüzdan
oluşturulduğunda her anahtarın kanıtını kontrol eder, ardından grubu Gnosis'teki
**kayıt defteri sözleşmesine** yazar ve gas bedelini öder.
`0x94fD1A891EB6c5F340622Baf2F3A0cb70A941EA9` adresindeki mevcut kayıt defterini
kullanmaya devam edin: sahibi yoktur, bakiyesi olan her adres ona yazabilir ve her Vela
uygulaması onu doğrudan okur. Kendinize ait bir kayıt defteri onlar için görünmez olur.

**İhtiyacınız olanlar**

- Redis ve Iggy ile Docker (sunucu sürümü) ya da bir Cloudflare hesabı (Worker sürümü;
  kendi README'si zincir üstüne yazma işleminin henüz uçtan uca test edilmediğini
  belirtiyor).
- xDAI'si olan bir Gnosis özel anahtarı. Bir cüzdanı kaydetmek tek anahtarla yaklaşık
  1,1M, yedi anahtarla yaklaşık 3,6M gas tutar.
- Şu ayarlar:

```dotenv
P256_INDEX_IGGY_URL=iggy+tcp://user:password@iggy.example:5100
P256_INDEX_REDIS_URL=redis://redis.example:6379/0
P256_INDEX_CONTRACT_ADDRESS=0x94fD1A891EB6c5F340622Baf2F3A0cb70A941EA9
P256_INDEX_DOMAIN_REGISTRY=0x5266DfF591B9F9EecfEdb8E7EfEf6c687854edaf
PRIVATE_KEY=0x…
```

Sunucunun örnek dosyası `P256_INDEX_DOMAIN_REGISTRY` değişkenini içermese de bu değişken
önemlidir: o olmadan sunucu, sözleşmenin reddettiği sınamalar (challenge) üretir ve her
kayıt başarısız olur.

**Çalıştırın ve kontrol edin**

```sh
git clone https://github.com/mondaylabsltd/p256-index
cd p256-index
cargo run --release -p p256-index-server
curl https://your-index/api/health   # "service":"webauthn-p256-publickey-registry","status":"ok"
```

Sunucu düz HTTP üzerinden dinler (varsayılan port 11256); cüzdan yalnızca `https://`
uç noktalarını kabul ettiği için önüne bir TLS vekil sunucusu koyun. Bu yazının
yazıldığı sırada kaynak koddaki Dockerfile derlenmeyebilir; Cargo ile derlemek çalışır.
Deponun henüz bir lisans dosyası yok.

**Hiçbir dizin yanıt vermezse** mevcut cüzdanlar yine çalışır: giriş sırasında uygulama
Gnosis'teki (ardından Ethereum'daki) kayıt defteri sözleşmesini sizin RPC
düğümleriniz üzerinden okur. Tek anahtarlı bir cüzdan, kayıt defterine hiç
başvurmadan iki imzadan bile yeniden kurulabilir. Yeni cüzdan oluşturmak ise bir dizin
gerektirir, çünkü kaydın bedelini birinin ödemesi gerekir.

## Kendi zincir verinizi çalıştırın

<span id="chain-data"></span>

Zincir verisi, [ethereum-data](https://github.com/atshelchin/ethereum-data)'dır (MIT):
yaklaşık 2.600 ağ ve tokenları için statik JSON ve görseller, ayrıca Vela'nın işlemleri
açıklamak için kullandığı ERC-7730 tanımlayıcıları.

```sh
docker run -d --name ethereum-data -p 3000:3000 --restart unless-stopped \
  ghcr.io/atshelchin/ethereum-data:latest
curl http://localhost:3000/api/health   # "service":"ethereum-data","status":"ok"
```

README'si kaynak koddan derlemeyi ve Cloudflare'e dağıtmayı da anlatıyor. HTTPS
üzerinden sunun ve adresi **Zincir Veri Dizini** alanına girin.

Relay bu dosyalardaki Vela'ya özgü alanlara da bağlıdır (`stables` listesi, ücretleri
hangi stabilcoinlerin ödeyebileceğini belirler) ve — yukarıda belirtildiği gibi —
onları Vela'nın kopyasından okur.

## Kendi döviz kuru servisinizi çalıştırın

<span id="exchange-rates"></span>

[vela-currency](https://github.com/mondaylabsltd/vela-currency) (MIT), Avrupa Merkez
Bankası'nın günlük kurlarını yeniden yayımlar. Hiçbir anahtar gerektirmez.

```sh
docker run -d -p 8080:8080 -v rates-data:/data ghcr.io/mondaylabsltd/vela-currency:latest
curl "http://localhost:8080/v2/rates?base=USD"
```

`https://your-host/v2/rates?base=USD` adresini **Fiat Kurları** alanına girin.
Frankfurter uyumlu herhangi bir servis de çalışır. `?base=USD` kısmını koruyun: her
dönüşüm buna göre yapılır.

## Uygulamaları kendiniz derleyin

<span id="web-app"></span>

Bütün uygulamalar [tek bir depoda](https://github.com/mondaylabsltd/vela-wallet) (MIT).
Her uygulamanın derleme adımları README'de; kısa hâli:

| Uygulama | Derleme | Mevcut getvela.app cüzdanınız için imzalar mı? |
| --- | --- | --- |
| Tarayıcı uzantısı | `cd app-web/vela-wallet && pnpm install && pnpm build:extension`, ardından `extension/dist` klasörünü `chrome://extensions` adresinde paketlenmemiş öğe olarak yükleyin | Evet, herhangi bir anahtarla |
| Web cüzdanı | `cd app-web/vela-wallet && pnpm install && pnpm build`; bir Cloudflare Worker olarak dağıtılır | Hayır — sizin alan adınızda başka bir cüzdandır (yukarıya bakın) |
| Masaüstü | `cd app-desktop/vela-wallet && cargo run` (paketleme betikleri README'sinde) | Evet, QR kodla bağlanan bir telefonla ya da bir USB güvenlik anahtarıyla |
| Android | Çekirdek bağlamalarını (bindings) üretin, ardından `./gradlew :app:installDebug` | Evet, QR kodla bağlanan bir telefonla ya da bir USB güvenlik anahtarıyla |
| iOS | `./rust/scripts/build-ios-xcframework.sh`, ardından kendi ekibinizle Xcode'da derleyin | Evet, QR kodla bağlanan bir telefonla ya da bir USB-C / Lightning YubiKey ile (ürün yazılımı 5.8 ya da sonrası) |

Kendi derlediğiniz bir uygulamanın "bu cihaz" geçiş anahtarı getvela.app cüzdanları için
çalışmaz: Apple ve Google, `getvela.app` geçiş anahtarlarını yalnızca Vela tarafından
imzalanmış uygulamaların kullanmasına izin verir.

## Vela'da hazır gelmeyen bir ağ ekleyin

Vela, P-256 ön derlemesi ve kontrol ettiği standart sözleşmeler bulunan her EVM
zincirinde çalışır. [Zincir kurulumu](/tr/chain-setup) bir zincirde neyin eksik
olduğunu söyler ve herkesin dağıtabileceği olanları dağıtır;
[ağlar ve ücretler](/tr/docs/networks-and-fees) gereksinimleri açıklar. Bir eksik var:
birden fazla anahtarı olan bir cüzdan, o zincirde Safe'in geçiş anahtarı imzalayıcı
fabrikasına da ihtiyaç duyar ve kontrol henüz buna bakmıyor — o fabrika yoksa orada
yalnızca ilk anahtar imzalayabilir.

## Bütün bunlardan sonra hâlâ Vela'ya işaret edenler

Yukarıdakilerin hepsini değiştirseniz de şunlar kalır:

- **Relay'in zincir dizini** — relay'in kodunda `ethereum-data.getvela.app` olarak
  sabit.
- Güvenlik anahtarı modellerini adlandıran **kimlik doğrulayıcı dizini** — yalnızca
  görünümü etkiler; uygulamalar genel bir ada geri döner.
- Mağaza uygulamalarının "bu cihaz" geçiş anahtarları için ihtiyaç duyduğu
  **getvela.app ilişkilendirme dosyaları**. Bir telefonun ya da güvenlik anahtarının
  bunlara ihtiyacı yoktur.

Şunlar ise Vela'nın değil: herkese açık seçici veritabanları, Apple'ın ve Google'ın
telefonla giriş tünelleri ve seçtiğiniz RPC sağlayıcıları.

Sırada: [kendiniz çalıştırabileceğiniz imza sayfası](/tr/docs/clear-signing-self-host).
