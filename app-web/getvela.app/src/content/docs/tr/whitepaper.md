---
title: Teknik doküman
description: Vela nasıl çalışıyor ve onu kullanmak için neye güvenmeniz gerekiyor — neye gerekmiyor. Mimari, güvenlik modeli, kurtarma ve bütün bunları kendiniz nasıl doğrularsınız.
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Teknik doküman

<Callout type="info" title="Durum: alfa · v0.1">
Bu sayfa Vela'nın bugün nasıl çalıştığını ve onu kullanmak için neye güvenmeniz
gerekip gerekmediğini anlatıyor. Pazarlamadan çok dürüstlüğü gözetiyor. Vela
<a href="/blog/vela-is-in-alpha">alfada</a> — küçük tutarlarla başlayın. Vela'nın
tokenı yok. Buradaki her şey açık kaynak kodla doğrulanabilir.
</Callout>

## Özet

Vela, EVM ağları için **kendi saklamanızda duran bir akıllı sözleşme cüzdanı**. Her
cüzdan, bir **geçiş anahtarının** kontrol ettiği bir
[Safe](https://github.com/safe-fndn/safe-smart-account) akıllı hesabı; geçiş anahtarı
ise cihazınızın işletim sisteminin tuttuğu, uçtan uca şifreli saklanan ve Face ID,
Touch ID veya parmak iziyle açılan bir WebAuthn (P-256) kimlik bilgisi. Kopyalamanız,
saklamanız ya da kaybetmeniz gereken kurtarma ifadeleri ve özel anahtarlar yok.

Şirket olarak Vela, anahtarlarınızı da paranızı da hiç tutmuyor ve onları **taşıyamaz,
donduramaz ya da el koyamaz**. Uygulama, işlem relay'i ve destek servisleri açık kaynak
ve kendiniz barındırabilirsiniz. Güvenmeniz gereken şey, denetlenmiş akıllı
sözleşmelere, işletim sisteminizin geçiş anahtarı kasasına ve — yalnızca çalışırlık
için — değiştirebileceğiniz ya da kendiniz çalıştırabileceğiniz bir relay'e iner.

## Vela neden var

Çoğu cüzdan bir takası dayatıyor:

- **Kurtarma ifadeli cüzdanlar** her kullanıcının önüne 12–24 kelimelik bir sır koyar.
  Bu, tek arıza noktasıdır ve sürekli bir oltalama hedefidir.
- **Saklayıcı cüzdanlar** kurtarma ifadesini ortadan kaldırır ama paranızı kendi
  saklamalarına alır — kriptonun ortadan kaldırması gereken karşı taraf riskini geri
  getirerek.
- **Kör imzalama** — okuyamadığınız anlaşılmaz hex'i onaylamak — ekosistemde
  normalleşti ve boşaltılan cüzdanların büyük bir bölümünün arkasında bu var.

Vela, sizi tamamen kendi saklamanızda tutarken saklayıcı bir uygulama kadar kolay
olmayı amaçlıyor: kurtarma ifadesi yok, saklayıcı yok ve imzalamadan önce
okuyamayacağınız işlem yok.

## Tasarım ilkeleri

1. **Kendi saklamanız, istisnasız.** Anahtarlar cihazınızda üretilir ve işletim
   sisteminizin geçiş anahtarı sağlayıcısında uçtan uca şifreli durur. Vela'nın
   sunucuları yalnızca genel veriyi görür.
2. **Güvenmeyin, doğrulayın.** Bütün yığın — uygulama ve dört arka uç servisinin
   tamamı — MIT lisansıyla açık kaynak.
3. **Kör imzalama yok.** İşlemler, tanımlayıcısı olan her yerde insanın okuyabileceği
   bir niyete çevrilir; bilinmeyen çağrılar saklanmaz, işaretlenir.
4. **Daha az yapın.** Cüzdan ETH ve ERC-20'leri tutar ve seçtiğiniz dApp'lere
   bağlanır. Güvenilecek daha az kod, daha küçük saldırı yüzeyi.

## Mimari

```text
Vela uygulaması (iOS / Android / web, tek kod tabanı)
  • Geçiş anahtarı (WebAuthn P-256, işletim sisteminin sağlayıcısı)
  • UserOperation oluşturma ve imzalama
  • Açık imzalama arayüzü (ERC-7730)
        │  imzalı UserOperation
        ▼
Vela relay'i (ERC-4337, kendiniz barındırabilirsiniz)
  • handleOps çağrısını EntryPoint'e gönderir
  • İşleminizi değiştiremez ya da taklit edemez
        ▼
EVM zinciri
  EntryPoint v0.7 → Safe akıllı hesabı
  WebAuthn imzalayıcısı P-256'yı zincir üstünde doğrular
```

### Hesap modeli

Cüzdanınız, **Safe 4337 Modülü** ve hesabın sahibi olarak bir **WebAuthn
imzalayıcısıyla**, **ERC-4337** hesap soyutlaması (EntryPoint v0.7) üzerinden
çalıştırılan bir **Safe v1.4.1** akıllı hesabı (bir proxy sözleşmesi).

Adres **deterministik** ve **karşıolgusal**: herhangi bir işlem gönderilmeden önce
geçiş anahtarınızın genel anahtarından `CREATE2` ile hesaplanır, yani hesap hiç
kurulmadan önce oraya para alabilirsiniz. Hesap, ilk işleminizde kendi bakiyesinden
ödeyerek kendini kurar.

### Anahtarlar ve kimlik doğrulama

Kimlik doğrulama, **P-256** eğrisindeki **WebAuthn geçiş anahtarlarıyla** yapılır. Özel
anahtar cihazınızda üretilir ve işletim sisteminizin geçiş anahtarı sağlayıcısında
(iCloud Anahtar Zinciri ya da Google Şifre Yöneticisi) uçtan uca şifreli durur;
sağlayıcı onu cihazlarınız arasında eşitler. **Vela'nın sunucuları yalnızca genel
anahtarınızı görür.** İmzalamak her seferinde yeni bir biyometrik doğrulama gerektirir
— uzun ömürlü bir oturum anahtarı yoktur. Ayrıntılar için
[geçiş anahtarları nasıl çalışır](/tr/docs/passkeys).

### İmzalama ve işlem akışı

1. Safe'iniz için bir ERC-4337 `UserOperation` **oluşturulur** ve gaz tahmin edilir.
2. Çağrı, insanın okuyabileceği bir niyete **çözülür** ve incelemeniz için gösterilir.
3. **İmzalarsınız** — cihazınız, biyometrik doğrulamanın ardından işlem özeti üzerinde
   bir WebAuthn onayı üretir.
4. Onay, bir **EIP-1271** sözleşme imzası olarak **kodlanır**.
5. İmzalı işlem relay'e **iletilir**, relay de onu EntryPoint'e gönderir.
6. **Zincir üstünde doğrulanır** — Safe, yürütmeden önce P-256 imzasını RIP-7212 ön
   derlemesiyle zincir üstünde doğrular. Ön derleme zorunludur: yedek doğrulayıcı yoktur
   ve Vela, onu sunmayan bir ağı açmayı reddeder.

Relay, **zaten imzalanmış** bir işlem alır. Alıcıyı, tutarı ya da başka bir alanı, imzayı
geçersiz kılmadan değiştiremez.

### Relay ve gaz modeli

- Gaz, **kendi cüzdan bakiyenizden** ödenir — varsayılan olarak ağın yerel tokenıyla ya
  da relay'in sunduğu yerlerde desteklenen bir stabilcoinle. Yerel parası olmayan
  Tempo'da gaz her zaman dolar stabilcoinleriyle ödenir. Ortada **paymaster** ve
  işlemlerinizi sponsorlayan — ya da kapılayan — üçüncü bir taraf yok.
- **Gaz fiyatının tek doğruluk kaynağı relay'dir.** Fiyatı canlı zincir koşullarından
  belirler; cüzdan o fiyatı gösterir ve tam olarak gösterdiğini imzalar.
- Vela'nın relay ücreti bilerek basit: toplam, **ağ maliyeti artı relay'in hizmet
  ücreti**; çok ucuz işlemlerde küçük bir asgari tutarla. Bir kısmı zincirin
  doğrulayıcılarına gider; kalanı altyapıyı çalıştıran ve gaz hesabınızı fonlu tutan
  relay'e.
- Cüzdan, **tahmini ücreti onaylamadan önce gösterir** — hem ücret varlığında hem de
  görüntüleme para biriminizde — ve bildirilen tutar ile alıcısı imzaladığınız şeyin
  parçasıdır; yani relay'e tam olarak gösterilen kadar ödenir. Gizli marj yok.
- Her Safe'in her zincirde **ayrılmış bir relay hesabı** (gaz hesabı) vardır ve bu hesap
  **iade edilmeyen** bir depozitoyla etkinleştirilir. Hesap zamanla tükenebilir, yani
  sonradan **yeniden etkinleştirme** gerekebilir — tam olarak tek seferlik bir depozito
  değildir.

Relay bir **çalışırlık** bağımlılığı, bir **saklama** bağımlılığı değil: iletmeyi
geciktirebilir ya da reddedebilir, ama hiçbir şeyi değiştiremez, taklit edemez ya da
çalamaz. Açık kaynak ve kendinizinkini çalıştırabilirsiniz; üstelik fiyat gizlenmek
yerine **bildirilip gösterildiği** için, kendi barındırdığınız ya da üçüncü taraf bir
relay'in ücreti bile imzalamadan önce size görünür. Bkz.
[ağlar ve ücretler](/tr/docs/networks-and-fees).

### Açık imzalama (ERC-7730)

Vela, calldata'yı ve EIP-712 tipli verisini **ERC-7730** tanımlayıcılarıyla çözer ve
**niyeti** (Takas, Gönder, Onayla…), **özü** (tutarlar, adresler) ve **ayrıntıları**
(nonce, son tarih, ham calldata) istendiğinde, riske göre renk kodlu biçimde gösterir.
Eşleşen bir tanımlayıcı yoksa Vela çağrıyı anlamış gibi yapmak yerine açık bir kör imza
uyarısı gösterir.

### Ağlar

Vela 12 EVM ağını destekliyor — Ethereum, BNB Chain, Polygon, Arbitrum, Optimism, Base,
Avalanche, Gnosis, Unichain, Tempo, Monad ve World Chain — artı kendi eklediğiniz ağlar.
Özel bir ağ, ancak Vela'nın dayandığı sözleşmeleri (EntryPoint, Safe sözleşmeleri,
WebAuthn imzalayıcısı) ve RIP-7212 P-256 ön derlemesini zaten barındırıyorsa
eklenebilir; Vela bunu açmadan önce kontrol eder.

## Güvenlik modeli

**Vela'nın yapamadıkları:**

- Paranızı taşımak, harcamak ya da devretmek — Safe'e yalnızca geçiş anahtarınız yetki
  verebilir.
- Hesabınızı dondurmak ya da ona el koymak — Safe zincirdeki sizin sözleşmeniz ve
  Vela'nın onda ayrıcalıklı bir rolü yok.
- Sizin adınıza imzalamak — her işlem yeni bir biyometrik onay gerektirir.
- Özel anahtarınızı görmek — anahtar Vela'ya hiç ulaşmaz; onunla yalnızca cihazınız
  imzalayabilir.
- Siz imzaladıktan sonra işlemi değiştirmek — her değişiklik imzayı geçersiz kılar.

**"Donduramaz"ın kapsamadığı şey:** *tokenın* kendisi. İzinli bir stabilcoin — USDC,
USDT ve fiat teminatlı tokenların çoğu — ihraççısının sizinki dahil herhangi bir adrese
karşı çağırabileceği bir kara liste işlevi taşır. O güç ihraççıya aittir ve tokenı hangi
cüzdanda tutarsanız tutun vardır; Vela dahil hiçbir kendi saklamanızda duran cüzdan onu
elinden alamaz. Kendi saklamanın verdiği şey şu: **biz**, bunu yapabilecek ikinci bir
taraf değiliz.

**Neye güveniyorsunuz:**

- **Safe sözleşmelerine** (denetlenmiş, yaygın kullanılan) ve P-256 anahtarınızı
  doğrulayan WebAuthn imzalayıcısına.
- Kimlik bilgilerinizi koruyup eşitlemesi için **işletim sisteminizin geçiş anahtarı
  sağlayıcısına** (Apple / Google).
- Sorguladığınız **RPC sağlayıcılarına** (Vela, yük devretmeli çok kaynaklı bir havuz
  kullanır; kendinizinkini de tanımlayabilirsiniz).
- Yalnızca çalışırlık için **relay'e** — ve onu kendiniz barındırabilirsiniz.

**Ele alınan tehditler:**

- **Kaybolan ya da çalınan cihaz** — hırsızın imzalamak için yine de biyometrinize ya da
  PIN'inize ihtiyacı var.
- **Oltalama / zararlı dApp** — açık imzalamayla karşılanır.
- **Ele geçirilmiş Vela sunucusu** — imzalama yeteneği vermez; etki alanı fon kaybı
  değil, bozulan hizmettir.
- **Tedarik zinciri riski** — açık kaynak ve kendi barındırmayla hafifletilir.

## Kurtarma

Geçiş anahtarınız işletim sisteminizin sağlayıcısı tarafından yedeklenir; yeni bir
cihazda aynı Apple ya da Google hesabıyla giriş yapmak onu geri getirir ve cüzdanınız
yeniden görünür.

<Callout type="warning" title="Platform geçiş anahtarı yedeğiniz, kurtarmanızın kendisi">
Vela'nın kurtarması, iCloud Anahtar Zinciri ya da Google Şifre Yöneticisi tarafından
eşitlenen geçiş anahtarınızdır. Tasarım gereği kurtarma ifadesi, sosyal kurtarma ve
vasi yok — yani Vela'nın kaybedebileceği, sızdırabileceği ya da kullanmaya
zorlanabileceği bir şey yok. Madalyonun öbür yüzü gerçek: <strong>hem</strong>
cihazınızı <strong>hem de</strong> bulutta eşitlenen geçiş anahtarınızı kaybeder ve
başka bir kopyanız olmazsa hesap kurtarılamaz. Platformunuzun geçiş anahtarı yedeğini
açık, hesabını da güvende tutun.
</Callout>

Dürüstçe söylenmiş sınırları da içeren tam kurtarma modeli
[kurtarma ve giriş](/tr/docs/recovery) sayfasında.

## Vela ortadan kalkarsa

Kendi saklamanız, anahtarlarınızın ve paranızın Vela'nın çevrimiçi olmasına bağlı
olmaması demek. Para **zincir üstündeki kendi Safe sözleşmenizde** duruyor ve relay
açık kaynak ve değiştirilebilir.

Dürüst bir uyarı: WebAuthn, bir geçiş anahtarını bir ilgili taraf alan adına
(`getvela.app`) bağlar. O alan adı kalıcı olarak kaybedilseydi, ona bağlı geçiş
anahtarlarının başka bir yerde çalışması için yardıma ihtiyacı olurdu — kimlik
doğrulayıcıya özgün ilgili tarafı sunabilen bir araca. Vela bu durum için eskiden
geliştirici düzeyinde bir tarayıcı uzantısı sunuyordu ve onu Eylül 2026'da kaldırdı;
alan adı kaybı için tüketici düzeyinde bir kurtarma yolu hâlâ bitmemiş bir iş ve biz de
var gibi ima etmek yerine bunu söylüyoruz. Zincir üstünde bağımsız erişim ayrıca hedef
zincirin P-256 (RIP-7212) desteğine bağlı; o destek de zincirlerde giderek yaygınlaşıyor.

## Gizlilik

Hesap yok, e-posta yok, KYC yok, toplanacak bir kurtarma ifadesi yok. Sunucular
yalnızca **genel anahtarınızı** ve seçtiğiniz hesap adını saklıyor (cihazlar arası
kurtarma için) ve bunlar tasarım gereği zincir üstünde yayımlanıyor. İşlem içerikleri
loglanmıyor. Site, çerezsiz ve kendi barındırdığımız bir analitik kullanıyor. Bkz.
[gizlilik politikası](/privacy).

## Doğrulanabilirlik ve açık kaynak

Her şey **MIT lisanslı ve açık kaynak** — uygulama ve dört arka uç servisinin tamamı
(zincir verisi, geçiş anahtarı dizini, relay, döviz kurları) ve bunları **kendiniz
barındırabilirsiniz** (Ayarlar → Gelişmiş → Servis Uç Noktaları). Kodu
[github.com/mondaylabsltd/vela-wallet](https://github.com/mondaylabsltd/vela-wallet)
adresinde okuyun.

## Token yok

Vela'nın **tokenı yok** ve çıkarma planı da yok. Satın alınacak, farm edilecek ya da
üzerine spekülasyon yapılacak bir şey yok. Gaz, her ağın yerel varlığıyla ödenir.

## Denetim durumu ve sınırlar

Her Vela hesabının çekirdeğindeki **Safe sözleşmeleri** bağımsız olarak denetlendi ve
sahada sınandı. Vela'nın onların etrafındaki **kendi entegrasyonu**, **bağımsız bir
üçüncü taraf denetiminden geçmedi** ve planlanmış bir denetim yok — profesyonel bir
denetim, tarihi olan bir taahhüt değil, proje karşılayabildiğinde ulaşılacak bir hedef.
O zamana kadar entegrasyonun incelemesi gayriresmî: kod açık kaynak ve onu okuyan
yetkin, ilgili topluluk üyelerine ve yapay zekâ destekli incelemeye dayanıyor. Bu
yardımcı oluyor, ama profesyonel bir denetime denk değil. Vela'yı alfa yazılım sayın ve
bu kadar genç bir şeye koymaktan rahatsız olmayacağınız tutarları kullanın.

## Kaynaklar

- ERC-4337 — EntryPoint üzerinden hesap soyutlaması
- EIP-1271 — Sözleşmeler için standart imza doğrulama
- ERC-7730 — Açık imzalama / yapılandırılmış veri tanımlayıcıları
- EIP-5792 — Cüzdan çağrı toplu işleme
- RIP-7212 — secp256r1 (P-256) imza doğrulaması için ön derleme
- WebAuthn / FIDO2 — Geçiş anahtarı kimlik doğrulaması
- [Safe akıllı hesabı v1.4.1](https://github.com/safe-fndn/safe-smart-account/tree/release/v1.4.1)
