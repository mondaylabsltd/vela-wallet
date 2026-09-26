---
title: Denetimler ve bilinen sorunlar
description: "Vela'nın bağlı olduğu her sözleşme, hangi sürümü kimin denetlediği, denetlenen sürümün dağıtılan sürüm olup olmadığı, izlediğimiz açık bulgular ve hiç denetlenmemiş olanlar."
source: a9c5e58e7ed3
---

"Denetlendi", belirli bir kodun belirli bir sürümü hakkında bir iddiadır; bu yüzden bu
sayfa raporları, commit'leri ve dağıtılmış adresleri kaynak olarak veriyor — ve
**denetlenmemiş** olanları da listeliyor; bu en az o kadar önemli.

Son gözden geçirme: 22 Eylül 2026. Bir hata bulursanız bize söyleyin, düzeltelim.

## Paranın geçtiği yol

Paranıza dokunabilecek her sözleşme, yayımlanmış incelemeleri olan üçüncü taraf
kodunun kanonik bir dağıtımıdır.

### Safe v1.4.1 — hesabın kendisi

Cüzdanınız, SafeL2 singleton'ını ve SafeProxyFactory'yi kullanan bir
[Safe](https://github.com/safe-fndn/safe-smart-account/tree/v1.4.1) proxy'sidir. Toplu
işlemler MultiSend üzerinden geçer.

[Ackee Blockchain, Safe v1.4.0'ı denetledi](https://github.com/safe-global/safe-smart-account/blob/main/docs/audit_1_4_0.md)
(nihai rapor 16 Mart 2023, düzeltme incelemesi 28 Mart): 11 bulgu, hiçbiri kritik ya
da yüksek değil; iki orta düzey bulgu değiştirilmek yerine kabul edildi. Kapsam SafeL2,
SafeProxyFactory, CompatibilityFallbackHandler, MultiSendCallOnly ve SignMessageLib
idi. v1.4.1, v1.4.0'dan tek bir işlevsel satırla ayrılıyor: modül kurulumunda bir
ERC-4337 uyumluluk düzeltmesi
([PR #572](https://github.com/safe-global/safe-smart-account/pull/572)); Safe, Ackee'ye
danıştı ve yeniden denetime gerek olmadığı sonucuna vardı. MultiSend'in mantığı,
[G0 Group'un denetlediği](https://github.com/safe-global/safe-smart-account/tree/main/docs)
v1.3.0'dan bu yana değişmedi. Bütün adresler
[safe-deployments](https://github.com/safe-global/safe-deployments) ile eşleşiyor.
Çekirdek sözleşmeler, en yüksek kademesi 1.000.000 dolara kadar ödeyen
[Safe Foundation hata ödül programının](https://docs.safefoundation.org/security/bug-bounty)
kapsamında.

2025'teki Bybit olayı bir sözleşme bulgusu değil: saldırganlar Safe'in web arayüzüne
sunulan JavaScript'i değiştirdi ve Safe'in
[adli inceleme açıklaması](https://safefoundation.org/blog/safe-ecosystem-foundation-statement)
sözleşmelerde bir açık bulmadı. [Bu konudaki sayfamız](/tr/docs/bybit-attack), aynı
saldırı sınıfının neden bizimki dahil her cüzdan arayüzünü ilgilendirdiğini anlatıyor.

### Safe4337Module v0.3.0 — ERC-4337 adaptörü

`0x75cf11467937ce3F2f357CE24ffc3DBF8fD5c226` adresinde dağıtılmış (Sourcify'da tam
eşleşme) ve Safe'inizin yedek işleyicisi (fallback handler) olarak da ayarlı. Üç kez
incelendi —
[raporlar burada](https://github.com/safe-global/safe-modules/blob/main/modules/4337/docs/v0.3.0/audit.md):

- **Ackee Blockchain**, nihai rapor Mart 2024: bir uyarı (derleyici optimizasyonunun
  kullanımı) kabul edildi; bundan daha ciddi açık bir bulgu yok.
- **Certora**, Ağustos 2026: bir **orta** düzey bulgu; kabul edildi ve v0.3.0'da
  **düzeltilmedi** — *yetki değişiklikleri, aynı paket (bundle) içinde zaten doğrulanmış
  sonraki UserOperation'ları geçersiz kılmaz*. Aşağıdaki "Bilinen sorunlar" bölümüne
  bakın.
- **Nethermind**, Ağustos 2026: bulgu yok.

Bir cüzdan dağıtılırken modülü etkinleştiren SafeModuleSetup v0.3.0 (`0x2dd6…5b47`),
Certora ve Nethermind incelemelerinin kapsamındaydı.

Modülün geçmişinde açıklanmış bir sorun var: v0.1.0 `initCode` ve `paymasterAndData`
alanlarını imzalamıyordu; bu bir gas taciz (griefing) vektörüydü ve
[v0.2.0'da düzeltildi](https://safefoundation.org/blog/strengthening-security-addressing-the-incident-of-the-canonical-4337-module);
Safe, v0.1.0'ın test ağları dışında kullanılmadığını bildiriyor. Vela, v0.3.0'ı
EntryPoint v0.7 ve Safe 1.4.1 ile kullanıyor; modülün sürüm notlarında anlatılan
yapılandırma bu.

### Safe geçiş anahtarı modülü v0.2.1 — imzalayıcılar

İlk anahtarınızı `0x94a4F6affBd8975951142c3999aEAB7ecee555c2` adresindeki
**SafeWebAuthnSharedSigner** doğrular. "Paylaşılan", tıpkı Safe singleton'ı gibi
sözleşme dağıtımının paylaşıldığı anlamına gelir; anahtarınız paylaşılmaz. Her Safe
kendi P-256 açık anahtarını kendi deposunda tutar.

Her ek anahtarın kendi imzalayıcı sözleşmesi vardır; bu sözleşmeyi
`0x1d31F259eE307358a26dFb23EB365939E8641195` adresindeki
**SafeWebAuthnSignerFactory**, `0x4E27b51350e6c2083EE19011120F50DAfEc5CA50` adresindeki
**SafeWebAuthnSigner singleton**'ına işaret eden bir proxy olarak oluşturur.

Bu sözleşmeleri v0.2.1 sürümünde kapsayan incelemeler
([raporlar](https://github.com/safe-global/safe-modules/blob/main/modules/passkey/docs/v0.2.1/audit.md)):

- Bir [Hats Finance denetim yarışması](https://github.com/safe-global/safe-modules/blob/main/modules/passkey/docs/v0.2.1/audit-competition-report-hats.md)
  (Haziran–Temmuz 2024): yüksek ya da orta düzey bulgu yok; üç düşük düzey bulgu, hepsi
  düzeltildi.
- **Certora**'nın sürüm commit'ini incelemesi: yeni bulgu yok. (Daha önceki v0.2.0
  denetimi, paylaşılan imzalayıcının henüz denetlenmediğini belirtiyor — sözleşme o
  denetimden sonra eklendi.)
- **Nethermind**, Ağustos 2026: bulgu yok.

Sürümden bu yana sözleşme düzeyinde bir açık açıklanmadı ve geçiş anahtarı
sözleşmeleri Safe Foundation ödül programının kapsamında.

Geçiş anahtarı imzaları zincirin **EIP-7951 / RIP-7212** ön derlemesiyle, yedek bir doğrulayıcı
olmadan doğrulanır. Uygulama bir ağı etkinleştirmeden önce ön derlemeyi gerçek bir
imzayla kontrol eder. İki çekince: özgün RIP-7212 spesifikasyonunda
[EIP-7951](https://eips.ethereum.org/EIPS/eip-7951)'in düzelttiği uç durum kusurları
var (yalnızca zaten başarısız olması gereken girdileri etkilerler, düzgün
biçimlendirilmiş WebAuthn imzalarını değil) ve bir yoklama, bir zincirin
uygulamasının sapabileceği her yolu yakalayamaz.

### EntryPoint v0.7 — işleminizi yürütür

`0x0000000071727De22E5E9d8BAf0edAc6f37da032` adresinde dağıtılmış;
[kanonik v0.7.0 sürümü](https://github.com/eth-infinitism/account-abstraction/releases/tag/v0.7.0).
Ethereum Foundation için
[OpenZeppelin tarafından denetlendi](https://www.openzeppelin.com/news/erc-4337-account-abstraction-incremental-audit)
(Ocak 2024): kritik ya da yüksek bulgu yok, beş orta düzey bulgu; 24 bulgunun hepsi
çözüldü ve düzeltme incelemesi commit'i sürümle eşleşiyor. Ethereum Foundation'ın
[ERC-4337 hata ödül programının](https://docs.erc4337.io/community/bug-bounty)
kapsamında (250.000 dolara kadar).

## İzlediğimiz bilinen sorunlar

### Tek bir paket içindeki yetki değişiklikleri (Safe4337Module, Certora M-01)

EntryPoint, bir paketteki (bundle) işlemlerin hiçbirini yürütmeden önce hepsini
doğrular. Dolayısıyla bir işlem bir sahibi kaldırırsa, o sahibin imzaladığı ve aynı
paketin ilerisine yerleştirilmiş bir işlem yine de doğrulamadan geçer ve yürütülür.
Safe bunu kabul etti ve v0.3.0'ı değiştirmedi.

Vela'nın uygulamaları hiçbir zaman sahip değişikliği oluşturmaz ve bunu isteyen bir dApp
doğrudan reddedilir; yani Vela bunu kendi başına hiçbir zaman tetiklemez. Yine de ele
geçirilmiş bir anahtarı başka Safe araçlarıyla kaldıran herkes için önemlidir: o
anahtarın aynı paket içinde devre dışı kalacağına güvenemezler.

### İmzalı bir işlemin araya girilerek yürütülmesi (v0.9 öncesi EntryPoint)

Şubat 2026'da araştırmacılar, v0.7 dahil v0.9 öncesindeki bütün EntryPoint
sürümlerini etkileyen bir taciz ve sansür vektörünü
[açıkladı](https://erc4337.substack.com/p/improving-useroperation-execution). İmzalı bir
işlemi bloğa girmeden önce ele geçiren biri, onu kendi kontrol ettiği bir çağrının
içinde yürütüp iç yürütmenin geri alınmasını (revert) sağlayabilir: işlem başarısız
olur ve yeniden imzalanması gerekir. (Vela'nın işlem içi ücretinde ücret transferi de
onunla birlikte geri alınır; bu yüzden gas bedelini siz değil relay üstlenir.)
Yeniden girişe (reentrancy) karşı korunan sözleşmeleri çağıran ya da geçici bir durumla
geri alınmaya zorlanabilen işlemleri etkiler; basit transferler etkilenmez. Para
çekme akışlarına karşı tekrar tekrar kullanılırsa paraları bir süre erişilemez
tutabilir. İmza taklit edemez ya da parayı başka yere yönlendiremez.

Vela'nın relay'i işlemleri paylaşılan bir mempool yerine doğrudan gönderir; ama
bekleyen bir `handleOps` işlemi herkese açık mempool'da yine de görünür, yani bu,
maruziyeti ortadan kaldırmaz, daraltır. Düzeltme yalnızca EntryPoint v0.9'da var
(Kasım 2025); v0.7 yamalanamaz. Geçiş, Safe'in 4337 modülünün v0.9'u desteklemesine
bağlı; olduğunda bu sayfa bunu yazacak.

### Vela'nın kendi savunmalarındaki eksikler

Bunlar sözleşme bulguları değil, cüzdanın sizi sandığınızdan daha az koruduğu yerler.
Her biri düzeltilmek üzere takip ediliyor; bilinçli bir ödünleşim olduğu belirtilenler
hariç:

- **Sınırsız bir onay, olduğu gibi bırakırsanız gönderilir** — bilinçli bir ödünleşim,
  çünkü üst sınır konmuş bir onay Permit2'yi ve toplu takasları bozar. "Sınırsız" bir
  onay (2^200 ya da daha fazla; Permit2 için 2^152) kırmızıyla gösterilir ve siz üst
  sınır koymadıkça dApp'in istediği gibi gönderilir. Web ve masaüstünde toplu işlemin
  içindeki bir onaya henüz üst sınır konamaz; imzalı izinlere (permit) ise hiçbir yerde
  üst sınır konamaz.
- **Bağımsız imza sayfası** henüz hiçbir uygulamaya **bağlı değil**.
- **Web sitesi, geçiş anahtarlarıyla aynı alan adında üçüncü taraf bir analitik
  betiği yükler.** Site kendi sayfalarının geçiş anahtarı kullanmasını yasaklar (bir
  Permissions-Policy başlığıyla) ve betiği anahtar barındıran sayfanın dışında tutar.

## Denetlenmemiş olanlar

- **Vela'nın kendi sözleşmeleri.** `0x94fD1A891EB6c5F340622Baf2F3A0cb70A941EA9`
  adresindeki [açık anahtar kayıt defteri](https://github.com/mondaylabsltd/p256-index/tree/main/contracts)
  (Gnosis; Ethereum ve Base'de de aynı adres), `0x5266DfF591B9F9EecfEdb8E7EfEf6c687854edaf`
  adresindeki ilk kayıt defteri dağıtımı (adresi her kaydın imza alanının bir
  parçasıdır) ve onların yerini aldığı eski dizin
  (`0xdd93420BD49baaBdFF4A363DdD300622Ae87E9c3`, salt okunur geçmiş). Bunlar
  denetlenmedi. Para tutmazlar, sahipleri yoktur ve yükseltilemezler; bir yetkilendirme
  katmanı değil, bir keşif katmanıdırlar. Harcama yetkisi yalnızca Safe'inizde
  tanımlı anahtarlardan gelir. Gerçekçi en kötü sonuç, paranın taşınması değil, bir
  cüzdanın yeni bir cihazda daha zor bulunmasıdır.
- **Multicall3.** README'si
  [açıkça](https://github.com/mds1/multicall3) "This contract is unaudited." diyor. Vela
  onu yalnızca toplu okumalar için kullanır — bakiyeler, token ayrıntıları, fiyat
  teklifleri — asla onaylar ya da parayla değil.
- **Deterministik dağıtıcılar** (Arachnid'in CREATE2 proxy'si ve Safe'in singleton
  fabrikası) — ekosistem standardı ve durumsuz, resmî denetimleri yok. Eksiklerse
  Vela'nın ağ kontrolü güvenli tarafta kalarak başarısız olur; adreste kodun var olup
  olmadığını kontrol eder, bayt bayt eşleşip eşleşmediğini değil.
- **Tempo.** 24 yerleşik ağdan biri; yerel coini yok, Vela orada gas bedelini pathUSD
  stabilcoiniyle öder. Eylül 2026 itibarıyla Tempo'nun
  [güvenlik politikası](https://github.com/tempoxyz/.github/blob/main/SECURITY.md),
  protokolün hâlâ denetim sürecinde olduğunu ve etkin bir hata ödül programı
  bulunmadığını söylüyor. Tempo'da tutulan paralar ve orada ödenen gas bu zincir
  düzeyindeki riski taşır; Tempo'yu listedeki en yeni ve en az kanıtlanmış zincir
  sayın.
- **Vela'nın kendisi.** Uygulamalar, arka uç servisleri ve yukarıdaki sözleşmeler
  üçüncü taraf denetiminden geçmedi ve takvime alınmış bir denetim de yok. Bu sayfadaki
  en büyük çekince bu. Ayrıntılar [Vela is in alpha](/blog/vela-is-in-alpha)
  yazısında. Küçük tutarlarla başlayın ve kodu okuyun.

## Kendiniz kontrol edin

Aşağıdaki her adres kanonik, herkese açık bir dağıtımdır. Bunları
[safe-deployments](https://github.com/safe-global/safe-deployments),
[safe-modules-deployments](https://github.com/safe-global/safe-modules-deployments)
ve [EntryPoint sürümü](https://github.com/eth-infinitism/account-abstraction/releases/tag/v0.7.0)
ile karşılaştırın:

| Sözleşme                                  | Adres                                        |
| ----------------------------------------- | -------------------------------------------- |
| SafeL2 singleton v1.4.1                   | `0x29fcB43b46531BcA003ddC8FCB67FFE91900C762` |
| SafeProxyFactory v1.4.1                   | `0x4e1DCf7AD4e460CfD30791CCC4F9c8a4f820ec67` |
| MultiSend v1.4.1                          | `0x38869bf66a61cF6bDB996A6aE40D5853Fd43B526` |
| CompatibilityFallbackHandler v1.4.1 ¹     | `0xfd0732Dc9E303f09fCEf3a7388Ad10A83459Ec99` |
| SafeModuleSetup v0.3.0                    | `0x2dd68b007B46fBe91B9A7c3EDa5A7a1063cB5b47` |
| Safe4337Module v0.3.0                     | `0x75cf11467937ce3F2f357CE24ffc3DBF8fD5c226` |
| SafeWebAuthnSharedSigner v0.2.1           | `0x94a4F6affBd8975951142c3999aEAB7ecee555c2` |
| SafeWebAuthnSignerFactory v0.2.1          | `0x1d31F259eE307358a26dFb23EB365939E8641195` |
| SafeWebAuthnSigner singleton v0.2.1       | `0x4E27b51350e6c2083EE19011120F50DAfEc5CA50` |
| EntryPoint v0.7                           | `0x0000000071727De22E5E9d8BAf0edAc6f37da032` |
| Multicall3                                | `0xcA11bde05977b3631167028862bE2a173976CA11` |
| Açık anahtar kayıt defteri (Vela, denetlenmedi) | `0x94fD1A891EB6c5F340622Baf2F3A0cb70A941EA9` |

¹ Bir ağ eklenirken kontrol edilir; Safe'iniz bunun yerine 4337 modülünü yedek
işleyici olarak kullanır.
