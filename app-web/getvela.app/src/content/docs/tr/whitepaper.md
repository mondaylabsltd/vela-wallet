---
title: Teknik doküman
description: "Vela nasıl çalışır ve onu kullanmak için neye güvenmeniz gerekir — neye gerekmez: hesap, anahtarlar, ücret, tehdit modeli, kurtarma ve Vela ortadan kalkarsa ne olacağı."
source: d072d6710855
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Teknik doküman

<Callout type="info" title="Durum: alfa · son güncelleme Eylül 2026">
Bu sayfa Vela'nın bugün nasıl çalıştığını ve onu kullanmak için neye güvenmeniz
gerekip neye gerekmediğini anlatıyor. Vela <a href="/blog/vela-is-in-alpha">alfa</a>
aşamasında — küçük tutarlarla başlayın. Vela'nın tokenı yok. Buradaki her şey açık
kaynak kodla karşılaştırılarak kontrol edilebilir; kod ile bu sayfa çeliştiğinde
doğru olan koddur, hatalı olan da bu sayfadır.
</Callout>

## Özet

Vela, Ethereum ve diğer EVM ağları için **kendi saklamanızda duran bir akıllı
sözleşme cüzdanıdır**. Her cüzdan, **ERC-4337** üzerinden çalıştırılan ve en fazla
yedi **geçiş anahtarıyla** kontrol edilen değiştirilmemiş bir **Safe v1.4.1**
hesabıdır — geçiş anahtarları, cihazlarınızın, parola yöneticinizin ya da donanım
güvenlik anahtarlarınızın tuttuğu WebAuthn P-256 anahtarlarıdır. Kurtarma ifadesi
yoktur.

Şirket olarak Vela anahtarlarınızı hiçbir zaman tutmaz ve Safe'inizde hiçbir rolü
yoktur; bu yüzden paranızı kendi başına **taşıyamaz, donduramaz ya da ona el
koyamaz**. Ama anahtarlarınızdan imza isteyen yazılımı Vela yazar ve sunar —
aşağıdaki tehdit modelinin önemli olmasının nedeni de bu. Uygulamalar, işlemleri
zincire gönderen relay ve destek servisleri açık kaynaktır ve her birinin kendi
kopyasını çalıştırabilirsiniz; relay bugün, kodunu değiştirmediğiniz sürece zincir
verisini hâlâ Vela'nın sunucusundan okuyor. Kısacası güvenmeniz gerekenler: sözleşmeler,
anahtarlarınızı tutan kimlik doğrulayıcılar, imzalarken kullandığınız uygulamanın
kodu, geçiş anahtarlarınızın ait olduğu alan adı ve uygulamayı yönlendirdiğiniz
servisler.

## Vela neden var

- **Kurtarma ifadeli cüzdanlar** her kullanıcının önüne 12–24 kelimelik bir sır koyar:
  tek bir arıza noktası ve sürekli bir oltalama hedefi.
- **Saklamalı (custodial) cüzdanlar** kurtarma ifadesini, paranın saklamasını
  üstlenerek ortadan kaldırır.
- Tek bir şirketin sunucularına ve kapalı koduna bağlı **geçiş anahtarı cüzdanları**
  kurtarma ifadesini ortadan kaldırır, ama şirket ortadan kalkarsa sizi ortada bırakır.
- **Kör imzalama** — okuyamadığınız anlaşılmaz veriyi onaylamak — hâlâ yaygındır ve
  cüzdanların boşaltılma yollarından biridir.

Vela, bu bağımlılıkların hiçbiri olmadan bir geçiş anahtarının kolaylığını amaçlar:
standart bir hesap, açık kod, değiştirilebilir servisler ve imzalamadan önce
okuyabileceğiniz işlemler.

## Tasarım ilkeleri

1. **Kendi saklama, istisnasız.** Anahtarları kimlik doğrulayıcılarınız oluşturur ve
   tutar. Vela'nın servisleri onları hiçbir zaman görmez; neleri gördükleri Gizlilik
   bölümünde listeleniyor.
2. **Standart sözleşmeler, değiştirilmeden.** Paranıza giden yoldaki hiçbir sözleşmeyi
   Vela yazmadı.
3. **Güvenmeyin, doğrulayın.** Uygulamalar ve servisler herkese açık; servisleri kendi
   sunucunuzda barındırabilirsiniz.
4. **İmzalamadan önce çözümleme.** Çözülemeyen her şey açık bir kör imzalama uyarısı
   taşır.
5. **Daha azını yapmak.** Cüzdan gönderir, alır ve seçtiğiniz dApp'ler için imzalar.

## Mimari

```text
Vela uygulamaları — web, tarayıcı uzantısı, masaüstü (macOS/Windows/Linux), iOS, Android
  ortak tek bir Rust çekirdeği (kurallar, kriptografi, ABI, açık imzalama) + her biri için yerel bir kabuk
  • UserOperation'ı oluşturur ve ne yaptığını gösterir
  • anahtarınızdan bir WebAuthn onayı (assertion) ister
        │  imzalı UserOperation (ücret dahil)
        ▼
Relay (vela-relay, kendi sunucunuzda barındırılabilir)
  • ücreti teklif eder, gas bedelini peşin öder, handleOps'u gönderir
  • işlemi değiştiremez
        ▼
EVM zinciri
  EntryPoint v0.7 → Safe v1.4.1 hesabınız → Safe 4337 modülü
  Safe geçiş anahtarı modülü P-256'yı RIP-7212 ön derlemesiyle doğrular
```

Destek servisleri, hepsi açık kaynak: yeni cüzdanları zincir üstündeki bir kayıt
defterine kaydeden ve sorguları yanıtlayan bir **açık anahtar dizini**, bir **zincir
verisi** dizini ve bir **döviz kuru** kaynağı. Bkz.
[kendi sunucunuzda barındırma kılavuzu](/tr/docs/self-hosting).

### Hesap

Cüzdanınız bir **Safe v1.4.1** proxy'sidir (SafeL2 singleton'ı); Safe'in **4337 modülü
v0.3.0** hem modülü hem de yedek işleyicisi (fallback handler) olarak etkindir ve
**EntryPoint v0.7** üzerinden çalıştırılır. Sahipleri, Safe'in **geçiş anahtarı modülü
v0.2.1**'deki imzalayıcılardır: ilk anahtarı paylaşılan imzalayıcı, her ek anahtarı ise
Safe'in fabrikasının oluşturduğu kendi imzalayıcı sözleşmesi doğrular. Eşik **1**'dir.

Adres **deterministik ve karşıolgusaldır** (counterfactual): hiçbir şey dağıtılmadan
önce, kurucu anahtarların hepsini içeren Safe kurulum verisinden `CREATE2` ile
hesaplanır. Her ağda aynıdır. O adrese hemen para alabilirsiniz; her ağdaki ilk
işleminiz cüzdanı dağıtır ve bunun bedelini o işlemin ücreti içinde öder.

### Anahtarlar

Bir cüzdanın, oluşturulduğu anda sabitlenen **bir ila yedi anahtarı** vardır.
Herhangi biri tek başına imzalayabilir (1-of-n). Bir anahtar şunlardan biri olabilir:

- kullandığınız cihazdaki bir geçiş anahtarı — izin verirseniz iCloud Anahtar Zinciri,
  Google Şifre Yöneticisi ya da başka bir parola yöneticisi tarafından eşitlenir;
- QR kod okutarak bağlanılan başka bir telefon (WebAuthn hibrit taşıması);
- USB ya da NFC ile kullanılan ve hiçbir yere eşitlenmeyen bir donanım güvenlik
  anahtarı.

Her imza, kimlik doğrulayıcının kendi kullanıcı doğrulamasını gerektirir — biyometri ya
da cihaz PIN'i veya güvenlik anahtarının PIN'i ve dokunuşu. Oturum anahtarı yoktur.
Anahtarlar sonradan eklenemez, kaldırılamaz ya da değiştirilemez: cüzdanın henüz
dağıtılmadığı her zincirdeki adres hâlâ kurucu kümeyi temsil eder; bu yüzden tek bir
zincirde sahipleri değiştirmek, hesabın zincirden zincire farklılaşmasına yol açardı.

Geçiş anahtarları bir bağlı olan tarafa (relying party) aittir — Vela'nınkiler
**`getvela.app`** için oluşturulur. Tarayıcılar onları yalnızca getvela.app ya da alt
alan adlarındaki sayfalara sunar; onları oltalamaya karşı dayanıklı kılan şey budur,
ama bu aynı zamanda bu belgenin aşağıda yeniden ele aldığı bir bağımlılıktır.

### İmzalama akışı

1. Safe'iniz için relay'e ödeme yapan bir transferi de içeren bir UserOperation
   **oluşturulur** ve simüle edilir.
2. İnsanın okuyabileceği bir niyete **çözülür** ve size gösterilir.
3. **İmzalanır**: kimlik doğrulayıcınız, kimliğinizi doğruladıktan sonra işlem özeti
   üzerinde bir WebAuthn onayı üretir.
4. Onay, geçiş anahtarı modülünün beklediği Safe imzası olarak **kodlanır**.
5. İmzalı işlem, EntryPoint'i çağıran relay'e **gönderilir**.
6. **Zincir üstünde doğrulanır**: Safe herhangi bir şeyi yürütmeden önce geçiş anahtarı
   modülü P-256 imzasını RIP-7212 ön derlemesiyle kontrol eder. Yedek bir doğrulayıcı
   yoktur; ön derlemesi olmayan bir ağ eklenemez.

### Ücretler

- Relay'e **işlem içinde** ödeme yapılır: işlem sıfır EntryPoint ücreti bildirir ve
  Safe'inizden relay'in adresine bir transfer içerir. Tutar ve alıcı imzaladığınız
  şeyin parçasıdır; yani onay ekranında gösterilenin tam olarak aynısını ödersiniz.
- Ücret, **cüzdanın işlem için ayırdığı gas'ın üç katıdır** (simülasyon tahminleri
  yarı yarıya artırılır ve alt sınırlar uygulanır); **cüzdanın kendi gas fiyatı
  okuması ile relay'in seçilen hız için verdiği fiyattan yüksek olanıyla
  fiyatlanır** ve asgari tutar yaklaşık 0,01 dolardır. Tempo'da çarpan ikidir. Hem
  ayrılan miktardaki pay hem de fiyattaki pay yüzünden ücret çoğu zaman işlemin gerçek
  zincir üstü maliyetinin on katı ya da daha fazlasıdır ve bir ağdaki ilk işlemde daha
  da yüksektir; aradaki farkı relay tutar.
- Ücret ağın coiniyle ya da relay'in kabul ettiği bir USD stabilcoiniyle ödenir
  (yerel coini olmayan Tempo'da pathUSD). **Paymaster yoktur**: kimse gas
  sponsorluğu yapmaz ve kimse işlemleri bir sponsorluk politikasıyla süzemez.
- Bir relay'in bir ağdaki kendi gas kasası boşsa, cüzdan bunu siz imzalamadan önce
  söyler. Kullanıcı başına bir depozito yoktur.

Ayrıntılar: [ağlar ve ücretler](/tr/docs/networks-and-fees).

### Açık imzalama

Çağrılar ve EIP-712 mesajları **ERC-7730** tanımlayıcılarıyla çözülür — yaygın
sözleşmeler için uygulamaya yerleşik olanlar, zincir verisi servisinden alınanlar ya da
standart token biçimleriyle eşleştirilenler — ve son çare olarak, "elden gelen en iyi
çözümleme" olarak etiketlenen herkese açık bir seçici veritabanı kullanılır. Geriye
kalan her şey açık bir kör imzalama uyarısı alır. Alınan tanımlayıcılar kriptografik
olarak doğrulanmaz. "Sınırsız" düzeydeki (2^200 ya da daha fazla) zincir üstü bir
onay, siz onu düşürene kadar gönderilemez; büyük ama sınırlı bir onay ve imzalı izinler
(permit) bir uyarıyla gösterilir ama engellenmez. Ayrıntılar:
[açık imzalama](/tr/docs/clear-signing).

### Ağlar

Vela'da 24 yerleşik ağ var — Ethereum, BNB Chain, Polygon, Arbitrum, Optimism, Base,
Avalanche, Gnosis, Unichain, Tempo, Monad, World Chain, Arc, X Layer, Stable, Soneium,
MegaETH, Robinhood Chain, Mantle, Kaia, Celo, Ink, Plume ve XRPL EVM — ve kontrol ettiği
on bir sözleşmeye ve RIP-7212 ön derlemesine sahip her EVM ağını kabul eder. (İkinci ila
yedinci anahtarlar o ağda Safe'in geçiş anahtarı imzalayıcı fabrikasına da ihtiyaç
duyar; kontrol bunu henüz kapsamıyor.)

## Güvenlik modeli

**Vela'nın yapamadıkları**

- Paranızı kendi başına taşımak, harcamak ya da dondurmak — Safe'inize yalnızca
  anahtarlarınız yetki verir ve Vela'nın onda hiçbir rolü yoktur. (Vela'nın
  yapabildiği, sizden imza isteyen yazılımı yayımlamaktır; aşağıdaki tehditlere
  bakın.)
- Siz imzaladıktan sonra bir işlemi değiştirmek — her değişiklik imzayı geçersiz kılar.
- Özel anahtarlarınızı okumak — kimlik doğrulayıcılarınızda kalırlar.
- Cüzdanınıza anahtar eklemek ya da ondan anahtar kaldırmak.

**"Donduramaz"ın kapsamadığı şey: token.** USDC, USDT ve itibari paraya dayalı
tokenların çoğu, ihraççısının sizinki dahil herhangi bir adresi kara listeye almasına
izin verir. Bu güç ihraççıya aittir ve hangi cüzdanı kullanırsanız kullanın vardır.
Kendi saklamanın size verdiği şey, Vela'nın bunu yapabilecek ikinci bir taraf
olmamasıdır.

**Neye güveniyorsunuz**

- **Sözleşmelere**: Safe, onun 4337 ve geçiş anahtarı modülleri, EntryPoint v0.7 ve
  zincirin RIP-7212 ön derlemesi.
- **Alan adına**: getvela.app'ten ya da alt alan adlarından birinden sunulan her sayfa,
  anahtarlarınızdan imza isteyebilir.
- Anahtarlarınızı tutan **kimlik doğrulayıcılara** ve — eşitlenen geçiş anahtarları
  için — onların arkasındaki Apple, Google ya da parola yöneticisi hesabına.
- **İmzalarken kullandığınız uygulamanın koduna.** İşlemi o oluşturur ve ne yaptığını
  size o gösterir. Ele geçirilmiş bir uygulama size bir şey gösterip başka bir şeyi
  imzalamanızı isteyebilir; kimlik doğrulayıcının istemi aradaki farkı size söylemez.
- Okuduğunuz **RPC uç noktalarına**: yalan söyleyen bir düğüm yanlış bakiyeler ya da
  yanlış bir simülasyon önizlemesi gösterebilir. Kendi uç noktanızı tanımlayabilirsiniz.
- **Zincir verisi ve döviz kuru servislerine**: token listelerini, tanımlayıcıları,
  ücret tokenı listesini ve itibari para tutarını token tutarına çevirmekte kullanılan
  kurları onlar sağlar.
- **Relay'e**: imzaladığınızı değiştiremez, ama geciktirebilir ya da reddedebilir,
  zincire ne zaman gireceğini seçebilir (dolayısıyla kayma toleransınız içinde bir
  takasın önüne geçebilir) ve ücretinizin dayandığı gas fiyatını, cüzdanın kendi
  okumasının üç katına kadar belirleyebilir.

**Ele alınan tehditler**

- **Kaybolan ya da çalınan cihaz** — hırsızın yine de kimlik doğrulayıcının kontrolünü
  geçmesi gerekir; başka bir anahtar erişimi geri getirir. Ama bir anahtar
  kaldırılamaz: biri başkasının eline geçmiş olabilirse paranızı yeni bir cüzdana
  taşıyın, çünkü eski adres her ağda o anahtar tarafından harcanabilir olmaya devam
  eder.
- **Oltalama** — geçiş anahtarı sahte bir siteye yazılamaz ve tarayıcılar onu yalnızca
  getvela.app ve alt alan adlarındaki sayfalara sunar.
- **Kötü niyetli dApp** — açık imzalama ve onay korumasıyla karşılanır, ama ciddi bir
  eksik var: bir dApp, Safe'inizden yine Safe'inize bir çağrı isteyebilir —
  `enableModule`, `addOwnerWithThreshold`, `setFallbackHandler`, `setGuard` — ve
  bunlardan herhangi biri, bir kez imzalandığında hesabı Bybit'in yükü kadar eksiksiz
  biçimde devreder. Vela bu çağrıları çözer ama henüz engellemez. Hedefi kendi cüzdan
  adresiniz olan her isteği reddedin.
- **Ele geçirilmiş arka uç servisi** (relay, dizin, zincir verisi, döviz kurları) —
  imzalama gücü yoktur ama gerçek bir etkisi vardır: hizmeti reddetmek, yanıltıcı
  tanımlayıcılar ya da token listeleri, bir itibari para tutarının ne kadar gönderdiğini
  değiştiren yanlış döviz kurları ve (relay için) yukarıdaki zamanlama ve gas fiyatı.
  Alınan tanımlayıcılar doğrulanmış kabul edilmez ve her servis değiştirilebilir.
- **Ele geçirilmiş uygulama dağıtımı** — değiştirilmiş bir web dağıtımı, uzantı
  güncellemesi ya da uygulama derlemesi, size imzalatmak için kötü niyetli bir işlem
  sunabilir. Bu, [Bybit](/tr/docs/bybit-attack) sınıfı bir saldırıdır. Bugünkü
  önlemler sınırlı: uygulamanın kendi içindeki çözümleme ve onay koruması, onaylanmış
  (notarized) macOS derlemeleri ve uzantıyı ya da uygulamaları kaynak koddan kendiniz
  derlemeniz (sürüm paketleri imza değil, SHA-256 sağlama toplamı taşır). Uygulamanın
  kodunu paylaşmayan bağımsız bir imza sayfası hazır ama henüz bağlanmadı.
- **Alan adından sunulan her şey** — getvela.app ya da alt alan adlarındaki herhangi bir
  sayfa, yüklediği bir betik dahil, Vela geçiş anahtarlarından imza isteyebilir ve istem
  yalnızca "getvela.app"i gösterir. Bu yüzden web sitesi kendi sayfalarının geçiş
  anahtarı kullanmasını yasaklar ve analitik betiğini anahtar barındıran sayfanın
  dışında tutar. Alan adı el değiştirirse, yeni sahibi geçiş anahtarlarını hangi
  uygulamaların kullanabileceğini de kontrol eder. Uzantı ve kendi derlediğiniz
  uygulamalar kendi kodlarını taşır; ama varsayılan olarak tanımlayıcıları yine
  getvela.app altından alır ve oradaki servisleri kullanırlar.

## Kurtarma

Cüzdan oluşturmak, cüzdanın açık anahtarlarını ve adresini Gnosis'teki herkese açık
bir **kayıt defteri sözleşmesine** yayımlar (Ethereum'a kopyalanabilir). Yeni bir
cihazda anahtarlarınızdan **herhangi biriyle** giriş yaparsınız; uygulama cüzdanı dizin
aracılığıyla ya da o olmazsa doğrudan kayıt defterinden bulur ve anahtarların kayıtlı
adresi yeniden verdiğini kontrol eder. Tek anahtarlı bir cüzdan, hiç kayıt defteri
olmadan iki imzadan da yeniden kurulabilir.

<Callout type="warning" title="Anahtarlarınız, kurtarmanızın kendisidir">
Kurtarma ifadesi, sosyal kurtarma ya da vasi yok — Vela'nın kaybedebileceği,
sızdırabileceği ya da kullanmaya zorlanabileceği hiçbir şey yok. Kurucu anahtarların
hepsi kaybolursa cüzdan kurtarılamaz. Cüzdanı birden fazla anahtarla oluşturun, geçiş
anahtarı eşitlemesine güveniyorsanız onu açık tutun ve arkasındaki hesabı güvenceye
alın.
</Callout>

Ayrıntılar: [kurtarma ve giriş](/tr/docs/recovery).

## Vela ortadan kalkarsa

Paranız zincir üstünde, kendi Safe'inizde kalır. Sözleşmeler Vela'ya bağlı değildir ve
Vela'nın çalıştırdığı her servis açık kaynaktır, başkaları da çalıştırabilir — yalnız
relay'in zincir verisini Vela'nın sunucusundan okumayı bırakması için kodunda bir
değişiklik gerekir. Taşınamayan tek şey, geçiş anahtarlarının bağlı olan tarafı olan
`getvela.app`'tir: web cüzdanının başka bir alan adındaki kopyası başka bir cüzdan
oluşturur. Mevcut cüzdanlar için Vela tarayıcı uzantısı (izinle `getvela.app` geçiş
anahtarlarını kullanabilir) ve kendi derlediğiniz uygulamalar (bir telefon ya da
güvenlik anahtarıyla) getvela.app olmadan çalışmaya devam eder.
[Kendi sunucunuzda barındırma kılavuzu](/tr/docs/self-hosting#if-getvela-app-disappears)
her yolu ve sınırlarını tek tek açıklıyor. Bir zincirde bağımsız erişim, o zincirin
RIP-7212'yi desteklemesini de gerektirir.

## Gizlilik

Hesap yok, e-posta yok, KYC yok. Bir cüzdan oluşturduğunuzda kayıt defterine yazılan
şeyler herkese açık olur: her anahtarın açık anahtarı ve kimlik bilgisi kimliği,
kimlik doğrulayıcı modeli, cüzdan adınız ve anahtar etiketleriniz, adres ve imzalı
kayıt verisi. Vela'nın dizini bu kaydı göndermeden önce görür, ayrıca adını aradığınız
adresleri de görür; Vela'nın relay'i adresinizi, gönderdiğiniz işlemleri ve
uygulamanızın kullandığı RPC uç noktasını (URL'sindeki olası API anahtarı dahil) görür
ve işlemleri yeniden denemek ve sorun teşhis etmek için sınırlı bir süre saklar. Her
servis IP adresinizi görür. Web sitesi çerezsiz bir analitik kullanır. Bağlayıcı liste
[gizlilik politikasıdır](/privacy).

## Açık kaynak

Cüzdan (bütün uygulamalar ve çekirdek), relay ve döviz kuru servisi MIT lisanslıdır;
zincir verisi dizini de MIT'dir. Açık anahtar dizini herkese açıktır ama henüz bir
lisans dosyası yoktur. Kod:
[github.com/mondaylabsltd](https://github.com/orgs/mondaylabsltd/repositories).

## Token yok

Vela'nın tokenı yok ve çıkarma planı da yok. Satın alınacak, farm edilecek ya da
üzerine spekülasyon yapılacak bir şey yok. Ücretler her ağın coiniyle ya da bir
stabilcoinle ödenir.

## Denetim durumu ve sınırlar

Safe'in sözleşmeleri, onun 4337 ve geçiş anahtarı modülleri ve EntryPoint v0.7 bağımsız
olarak denetlenmiştir ve yaygın biçimde kullanılır. **Vela'nın kendi kodu —
uygulamalar, arka uç servisleri ve kayıt defteri sözleşmesi — bağımsız bir üçüncü taraf
denetiminden geçmedi ve takvime alınmış bir denetim de yok**; profesyonel bir denetim,
tarihi olan bir taahhüt değil, proje bir denetimi finanse edebildiğinde ulaşılacak bir
hedef. O zamana kadar inceleme gayriresmî: kod açık, yetkin topluluk üyeleri onu
okuyor ve yapay zekâ araçlarıyla inceleniyor. Bu yardımcı oluyor; ama profesyonel bir
denetime denk değil. Vela'yı alfa yazılım olarak görün. Ayrıntılar:
[denetimler ve bilinen sorunlar](/tr/docs/security-audits).

## Kaynaklar

- ERC-4337 — EntryPoint üzerinden hesap soyutlaması
- EIP-1271 — Sözleşmeler için imza doğrulama
- ERC-7730 — Açık imzalama tanımlayıcıları
- EIP-5792 — Cüzdan çağrılarını toplu gönderme (`wallet_sendCalls`)
- RIP-7212 / EIP-7951 — P-256 imza doğrulama ön derlemesi
- WebAuthn / FIDO2 — Geçiş anahtarları
- [Safe akıllı hesabı v1.4.1](https://github.com/safe-fndn/safe-smart-account/tree/v1.4.1)
