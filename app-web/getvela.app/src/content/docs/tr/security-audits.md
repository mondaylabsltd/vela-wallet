---
title: Denetimler ve bilinen sorunlar
description: Vela'nın bağlı olduğu her zincir üstü sözleşme, onu kimin denetlediği, denetlenen sürümün gerçekte kurulanla eşleşip eşleşmediği ve hiç denetlenmemiş olanlar.
---

"Denetlendi", belirli bir kodun belirli bir sürümü hakkında bir iddiadır; bu yüzden
bu sayfa kelimeyi havada sallamıyor: tam raporları, tam kurulum adreslerini ve
denetlenen ile kurulan sürümler arasındaki farkları adıyla anıyor. Denetlenmemiş
olanları da sayıyor, çünkü o liste de en az ilki kadar yük taşıyor.

Son gözden geçirme: Ağustos 2026. Burada bir hata bulursanız söyleyin, düzeltelim.

## Paranın geçtiği yol

Paranıza dokunabilecek dört sözleşme katmanı var. Dördü de yayımlanmış denetimleri
olan üçüncü taraf sözleşmeler ve her birinde kurulu adres, resmî kanonik kurulumun
kendisi.

### Safe v1.4.1 — hesabın kendisi

Cüzdanınız bir [Safe](https://github.com/safe-global/safe-smart-account) proxy'si:
SafeL2 singleton, proxy fabrikası, uyumluluk yedek işleyicisi ve toplu işlemler için
MultiSend.

[Ackee Blockchain, Safe v1.4.0'ı denetledi](https://github.com/safe-global/safe-smart-account/blob/main/docs/audit_1_4_0.md)
(nihai rapor Mart 2023): 11 bulgu, hiçbiri kritik ya da yüksek değil. Kurduğumuz
v1.4.1, denetlenen v1.4.0'dan yalnızca tek satırlık bir ERC-4337 uyumluluk
düzeltmesiyle ayrılıyor
([PR #572](https://github.com/safe-global/safe-smart-account/pull/572)). MultiSend'in
mantığı, [G0 Group'un denetlediği v1.3.0'dan](https://github.com/safe-global/safe-smart-account/tree/main/docs)
beri değişmedi. Bütün adresler
[safe-deployments](https://github.com/safe-global/safe-deployments) içindeki kanonik
kurulumlarla eşleşiyor ve sözleşmeler
[Safe Foundation ödül programının](https://docs.safefoundation.org/security/bug-bounty)
kapsamında (kritik bulgular için 1.000.000 dolara kadar).

Bir denetimin kapsamadığı bir şey: 2025'teki Bybit olayı. O saldırı sözleşmeleri
değil, Safe'in resmî web ön yüzünün derleme hattını ele geçirdi —
[resmî adli inceleme sonucu](https://safefoundation.org/blog/safe-ecosystem-foundation-statement)
Safe akıllı sözleşmelerinde bir açık bulmadı. Biz bunu web ve operasyon katmanına
dair bir ders olarak okuyoruz; bizi de asıl o katmanda sıkıştırmanız gerekir.

### Safe4337Module v0.3.0 — ERC-4337 adaptörü

`0x75cf11467937ce3F2f357CE24ffc3DBF8fD5c226` adresinde kurulu; v0.3.0'ın kanonik
adresi (Sourcify'da tam eşleşme — zincirdeki byte kodu, denetlenen kodun kendisi).
[Ackee Blockchain tarafından denetlendi](https://github.com/safe-global/safe-modules/blob/main/modules/4337/docs/v0.3.0/audit.md)
(nihai rapor Mart 2024); bilgilendirme düzeyinin üzerinde çözülmemiş bulgu yok.
Kullandığımız v0.3.0 + EntryPoint v0.7 + Safe ≥1.4.1 bileşimi, denetimin ve sürüm
notlarının anlattığı yapılandırmanın aynısı.

Modülün geçmişinde açıklanmış tek bir sorun var: v0.1.0 (2023), `initCode` ve
`paymasterAndData` alanlarını imzalamıyordu; bu bir gaz taciz vektörüydü.
[v0.2.0'da düzeltildi](https://safefoundation.org/blog/strengthening-security-addressing-the-incident-of-the-canonical-4337-module)
ve v0.1.0 test ağlarının dışına hiç çıkmadı. Biz düzeltmeyi devralan v0.3.0'ı
kullanıyoruz.

### SafeWebAuthnSharedSigner v0.2.1 — geçiş anahtarı imzalayıcısı

`0x94a4F6affBd8975951142c3999aEAB7ecee555c2` adresinde kurulu; v0.2.1'in kanonik
adresi (Safe singleton fabrikası sayesinde her zincirde aynı).

"Paylaşılan" ne demek — ve ne demek değil: paylaşılan şey, tıpkı Safe singleton'ı
gibi, _sözleşme kurulumudur_. Anahtarınız değil. Her Safe, `configure()` işlevini
delegatecall ile çağırır ve kendi P-256 genel anahtarını kendi deposunda tutar. Bir
imzalayıcı örneği, Safe başına tam olarak bir geçiş anahtarını temsil eder ve başka
kimsenin Safe'i sizinkini kullanamaz.

Burada sürüm önemli. v0.2.0 denetimi, paylaşılan imzalayıcının kapsam dışı olduğunu
[açıkça belirtmişti](https://github.com/safe-global/safe-modules/blob/main/modules/passkey/docs/v0.2.0/audit.md)
— sözleşme o sırada henüz yoktu. Kurduğumuz şeyi kapsayan denetimler v0.2.1
denetimleri:
[bir Hats Finance denetim yarışması](https://github.com/safe-global/safe-modules/blob/main/modules/passkey/docs/v0.2.1/audit-competition-report-hats.md)
(Haziran–Temmuz 2024: sıfır yüksek, sıfır orta, üç düşük bulgu — hepsi düzeltildi) ve
[Certora'nın sürüm commit'ini incelemesi](https://github.com/safe-global/safe-modules/blob/main/modules/passkey/docs/v0.2.1/audit.md),
yeni bulgu yok. Sürümden bu yana sözleşme düzeyinde bir açık açıklanmadı; geçiş
anahtarı sözleşmeleri Safe Foundation ödül programının kapsamında.

Safe'in kendi belgeleri, geçiş anahtarıyla sahiplik kurarken tek bir kimlik bilgisini
hesabın tek anahtarı saymak yerine bir kurtarma yolu eklemeyi öneriyor. Vela'nın bunu
nasıl ele aldığı [Kurtarma ve giriş](/tr/docs/recovery) sayfasında.

Zincir üstü P-256 doğrulaması, Solidity yedek doğrulayıcısı olmadan doğrudan RIP-7212
ön derlemesiyle yapılır. Uygulama, herhangi bir ağı açmadan önce ön derlemeyi gerçek
bir imzayla yoklar ve doğrulama başarısız olursa o ağı reddeder. İki dürüst uyarı:
özgün RIP-7212 spesifikasyonunda, düzeltmek için
[EIP-7951](https://eips.ethereum.org/EIPS/eip-7951)'in yazıldığı sınır durumu
kusurları var (düzgün biçimlendirilmiş WebAuthn imzalarını etkilemiyorlar) ve bir
yoklama, bir zincirin uygulamasının alışılmadık yürütme bağlamlarında sapabileceği
her yolu yakalayamaz.

### EntryPoint v0.7 — ERC-4337 giriş noktası

`0x0000000071727De22E5E9d8BAf0edAc6f37da032` adresinde kurulu;
[kanonik v0.7.0 kurulumu](https://github.com/eth-infinitism/account-abstraction/releases/tag/v0.7.0).
[OpenZeppelin tarafından denetlendi](https://www.openzeppelin.com/news/erc-4337-account-abstraction-incremental-audit)
(Ethereum Foundation'ın siparişiyle, Ocak 2024): sıfır kritik, sıfır yüksek, beş orta
bulgu, hepsi çözüldü — ve denetlenen commit, kurulu sürümün kendisi. EntryPoint
v0.7.0, Ethereum Foundation'ın
[ERC-4337 ödül programının](https://docs.erc4337.io/community/bug-bounty) kapsamında
(250.000 dolara kadar).

## İzlediğimiz bilinen sorunlar

### EntryPoint taciz vektörü

Şubat 2026'da Trust Security'deki güvenlik araştırmacıları, v0.9 öncesindeki bütün
EntryPoint sürümlerini — kullandığımız v0.7 dahil — etkileyen bir taciz ve sansür
vektörünü
[açıkladı](https://erc4337.substack.com/p/improving-useroperation-execution).
İmzalanmış bir UserOperation'ı bloğa girmeden önce yakalayan bir saldırgan, onu kendi
kontrolündeki bir çağrı çerçevesi içinde yürütüp iç yürütmenin geri sarmasına yol
açabiliyor: işlem başarısız oluyor ama gaz yine de tahsil ediliyor. Ethereum
Foundation araştırmacılara 50.000 dolarlık ödül ödedi; sorunu fon hırsızlığı değil,
sansür/taciz vektörü olarak sınıflandırdı ve sorun hiç istismar edilmedi.

Yapabildiği: bir ücreti yakmak ve bir işlemi geciktirmek. Yapamadığı: fon çalmak ya da
imza taklit etmek. Vela'nın maruziyeti dar, çünkü UserOperation'lar herkese açık bir
mempool yerine doğrudan bir relay'e gidiyor; yakalamak için pek fırsat yok ve en kötü
durum, zaten kabul ettiğiniz ücretle sınırlı. Düzeltme yalnızca EntryPoint v0.9'da
var (Kasım 2025); v0.7'nin kendisi yamalanamıyor. Çevredeki yığın — özellikle Safe'in
4337 modül hattı — v0.9 desteği ekledikçe geçmeyi bekliyoruz ve olduğunda bunu burada
not edeceğiz.

## Denetlenmemiş olanlar

- **Vela'nın kendi sözleşmeleri.** Gnosis'te kurulu, kendi yazdığımız iki küçük
  sözleşme:
  [geçiş anahtarı genel anahtar dizini](https://github.com/atshelchin/webauthnp256-publickey-index.biubiu.tools)
  (cihazlarınızın genel anahtarınızı bulmasına yardım eden, yalnızca ekleme yapılan bir
  kayıt) ve onun toplu yardımcı sözleşmesi. Bunlar denetlenmedi. Yapıları gereği fon
  tutmuyor, sahipleri yok ve yükseltilemiyorlar — bir yetkilendirme katmanı değil,
  keşif katmanı bunlar. Harcama yetkisi her zaman Safe'inizin içinde yapılandırılmış
  geçiş anahtarından gelir. En kötü gerçekçi sonuç taciz (birinin bir dizin kaydını
  kapması) olur; bu kurtarmayı zahmetli kılabilir ama para taşıyamaz. Eski bir ücret
  tasarımından kalan gaz uzlaştırma bölücü sözleşmesi artık işlem akışının parçası
  değil.
- **Multicall3.** Kendi README'si
  [açıkça söylüyor](https://github.com/mds1/multicall3): "This contract is unaudited."
  Onu tam olarak yazarlarının güvenli dediği biçimde kullanıyoruz — bakiyeler, token
  meta verileri ve fiyat sorguları için toplu salt okuma çağrıları. Vela ona hiç yetki
  vermiyor ve o hiç fon tutmuyor. Bir hatanın en kötü sonucu, yanlış okunan bir veri.
- **CREATE2 dağıtıcısı.**
  [Arachnid deterministik dağıtım proxy'si](https://github.com/Arachnid/deterministic-deployment-proxy),
  ekosistemin standart durumsuz dağıtıcısı; resmî bir denetimi yok. Ağ kontrollerimiz,
  o bir zincirde eksik ya da değiştirilmişse kapalı biçimde başarısız olur.
- **Tempo ve pathUSD.** On iki yerleşik ağımızdan biri olan Tempo'nun yerel parası yok;
  orada gaz pathUSD stabilcoiniyle ödeniyor. Ağustos 2026 itibarıyla ne Tempo'nun
  çekirdek protokolünün ne de pathUSD'nin yayımlanmış bir güvenlik denetimi veya ödül
  programı var; bağımsız bir
  [DefiLlama teminat değerlendirmesi](https://artifacts.llama.fi/md-exports/pathusd-collateral-assessment-april2026-1776332825042.md)
  (Nisan 2026) pathUSD'yi yüksek riskli olarak derecelendirdi. Bu, hiçbir cüzdanın
  hafifletemeyeceği zincir düzeyinde bir risk: Tempo'da tuttuğunuz para ve oradaki gaz
  uzlaşması bu riski devralır. Tempo'yu listedeki en yeni ve en az denenmiş zincir
  sayın ve bakiyelerinizi ona göre ayarlayın. Denetimler yayımlandıkça bu bölümü
  güncelleyeceğiz.
- **Vela'nın kendisi.** Uygulamamız ve arka uç servislerimiz üçüncü taraf denetiminden
  geçmedi. Bu sayfadaki en büyük çekince bu; site başlığında da yazıyoruz ve dürüst
  ayrıntılar [Vela alfada](/blog/vela-is-in-alpha) yazısında. Küçük tutarlarla
  başlayın. Kodu okuyun.

## Kendiniz kontrol edin

Yukarıdaki her adres, resmî kayıtlarla karşılaştırabileceğiniz kanonik ve herkese açık
bir kurulum —
[safe-deployments](https://github.com/safe-global/safe-deployments),
[safe-modules-deployments](https://github.com/safe-global/safe-modules-deployments)
ve [EntryPoint sürüm notları](https://github.com/eth-infinitism/account-abstraction/releases/tag/v0.7.0):

| Sözleşme | Adres |
| ----------------------------------- | -------------------------------------------- |
| SafeL2 singleton v1.4.1 | `0x29fcB43b46531BcA003ddC8FCB67FFE91900C762` |
| SafeProxyFactory v1.4.1 | `0x4e1DCf7AD4e460CfD30791CCC4F9c8a4f820ec67` |
| CompatibilityFallbackHandler v1.4.1 | `0xfd0732Dc9E303f09fCEf3a7388Ad10A83459Ec99` |
| MultiSend v1.4.1 | `0x38869bf66a61cF6bDB996A6aE40D5853Fd43B526` |
| SafeModuleSetup v0.3.0 | `0x2dd68b007B46fBe91B9A7c3EDa5A7a1063cB5b47` |
| Safe4337Module v0.3.0 | `0x75cf11467937ce3F2f357CE24ffc3DBF8fD5c226` |
| SafeWebAuthnSharedSigner v0.2.1 | `0x94a4F6affBd8975951142c3999aEAB7ecee555c2` |
| EntryPoint v0.7 | `0x0000000071727De22E5E9d8BAf0edAc6f37da032` |
| Multicall3 | `0xcA11bde05977b3631167028862bE2a173976CA11` |
| Geçiş anahtarı genel anahtar dizini (Gnosis) | `0xdd93420BD49baaBdFF4A363DdD300622Ae87E9c3` |
