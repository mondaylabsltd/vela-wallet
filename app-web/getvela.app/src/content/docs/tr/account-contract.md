---
title: Hesap sözleşmesi
description: "Vela cüzdanınız değiştirilmemiş bir Safe v1.4.1. Paranıza giden yoldaki hiçbir sözleşmeyi Vela yazmadı — burada tam olarak hangi sözleşmeler olduğu, bunun size ne kazandırdığı ve neye mal olduğu anlatılıyor."
source: 17fbc25a3149
---

# Hesap sözleşmesi

Cüzdanınız bir uygulamanın özel veri yapısı değil. Bir **Safe v1.4.1** akıllı
hesabıdır — zincir üstündeki pek çok büyük hazinenin kullandığı sözleşme — ve Safe'in
yayımladığı hâliyle, hiçbir değişiklik yapılmadan dağıtılır.

## Yoldaki hiçbir şey bizim değil

Paranıza dokunabilecek her sözleşmeyi Safe ya da ERC-4337'nin yazarları yazdı:

| Sözleşme | Cüzdanınızdaki rolü | Yazan |
| --- | --- | --- |
| [Safe v1.4.1](https://github.com/safe-fndn/safe-smart-account/tree/v1.4.1) (SafeL2, bir proxy üzerinden) | Hesabın kendisi; sahipler, eşik, yürütme | Safe |
| [Safe 4337 Module v0.3.0](https://github.com/safe-global/safe-modules/tree/4337/v0.3.0/modules/4337) | EntryPoint'in Safe'i çalıştırmasını sağlar; aynı zamanda yedek işleyicisi (fallback handler) | Safe |
| [SafeWebAuthnSharedSigner v0.2.1](https://github.com/safe-global/safe-modules/tree/passkey/v0.2.1/modules/passkey) | İlk anahtarın P-256 imzalarını doğrular | Safe |
| [SafeWebAuthnSignerFactory v0.2.1](https://github.com/safe-global/safe-modules/tree/passkey/v0.2.1/modules/passkey) ve oluşturduğu imzalayıcılar | Her ek anahtar için küçük bir imzalayıcı sözleşme | Safe |
| [ERC-4337 EntryPoint v0.7](https://github.com/eth-infinitism/account-abstraction/releases/tag/v0.7.0) | İmzaladığınız işlemi yürütür | ERC-4337 yazarları |

Vela'nın kendi sözleşmeleri bu listede yok: yeni bir cihazın cüzdanı bulabilmesi için
her cüzdanın anahtarlarını kaydeden **açık anahtar kayıt defteri**
([kurtarma](/tr/docs/recovery)), ona eşlik eden alan adı kayıt defteri ve onların
yerini aldığı eski dizin. Bunlar para tutmaz ve Safe'inizde hiçbir rolleri yoktur.

Cüzdan deposunda hiç Solidity yok — bunu tek bir komutla kontrol edebilirsiniz:

```bash
git clone https://github.com/mondaylabsltd/vela-wallet
find vela-wallet -name '*.sol'   # hiçbir şey yazdırmaz
```

Vela'nın hesabınızda ayrıcalıklı bir rolü yok: yönetici anahtarı yok, yükseltme yolu
yok, ekleyebileceği bir modül yok. Safe'inizi yalnızca sizin anahtarlarınız
değiştirebilir.

## "Değiştirilmemiş" neden asıl önemli kelime

Pek çok cüzdan "bir Safe", bir Safe çatalı ya da Safe'ten esinlenmiş bir hesap üzerine
kuruludur. Aradaki fark üç açıdan önemlidir.

**Denetimler gerçekten kullandığınız şeyi kapsar.** Safe'in denetimleri bu sürümleri ya
da bunlardan yalnızca küçük, belgelenmiş değişikliklerle ayrılan önceki sürümleri
kapsar (ayrıntılar [denetimler sayfasında](/tr/docs/security-audits)). Bir çatalın
denetimleri çataldan önceki kodu kapsar; değişiklik ise kimsenin denetlemediği
kısımdır.

**Ekosistem hesabınıza Safe gibi davranır, çünkü o bir Safe.** Blok gezginleri onu
çözer; Safe'in araçları onu okuyabilir ve onun için işlem oluşturabilir. Ama bu
işlemleri *imzalamak* için bir programın, anahtarınızdan geçiş anahtarlarınızın ait
olduğu alan adı olan `getvela.app` için imza isteyebilmesi gerekir — bu yüzden başka bir
alan adından sunulan Safe'in kendi web uygulaması sizin adınıza imzalayamaz. Neyin
imzalayabildiğini
[kendi sunucunuzda barındırma kılavuzu](/tr/docs/self-hosting#if-getvela-app-disappears)
listeliyor.

**Saldırı yüzeyi, başka pek çok kişinin de izlediği bir yüzey.** Kendine özgü bir hesap
sözleşmesini çoğunlukla yalnızca yazarı izler. Safe'in çekirdek sözleşmelerini bir
Safe'te parası olan herkes izler; 4337 ve geçiş anahtarı modüllerinin izleyicisi daha
az, ama gerçek.

## Bedeli ne

Standart olmak bedava değil:

- **Gas.** İmzanız zincir üstünde doğrulanır ve işlem EntryPoint üzerinden yürür.
  Dağıtılmış bir Vela cüzdanından yapılan basit bir gönderim, Gnosis üzerindeki
  ölçümlerimizde (Eylül 2026) zincir üstünde kabaca 140.000–170.000 gas kullandı;
  sıradan bir hesaptan yapılan düz bir ETH transferi 21.000 kullanır. Gas'ın üstüne
  relay kendi ücretini alır — bkz. [ağlar ve ücretler](/tr/docs/networks-and-fees).
- **Hesabın dağıtılması gerekir.** Adresiniz zincirde hiçbir şey yokken `CREATE2` ile
  hesaplanır, yani o adrese hemen para alabilirsiniz; her ağdaki ilk giden işleminiz
  sözleşmeyi dağıtmanın bedelini öder.
- **Her zincir uygun değil.** Geçiş anahtarı imzaları **EIP-7951 / RIP-7212** ön derlemesiyle
  doğrulanır ve onun adresi her cüzdanın kurulum verisinin bir parçasıdır; bu yüzden
  bu ön derlemesi olmayan bir ağda Vela hiç çalışamaz.
- **Safe'in riski artık sizin riskiniz.** Yaygın kullanılan bir sözleşmeye güvenmek de
  sonuçta bir sözleşmeye güvenmektir. Vela, paranıza giden yola güvenmeniz gereken
  ikinci bir kendi sözleşmesini eklemedi.

## Ne denetlendi, ne denetlenmedi

Safe'in sözleşmelerinin, 4337 ve geçiş anahtarı modüllerinin ve EntryPoint v0.7'nin
yayımlanmış üçüncü taraf denetimleri var. **Vela'nın kendi kodu — uygulamalar, arka uç
servisleri ve kayıt defteri sözleşmesi — üçüncü taraf denetiminden geçmedi ve
takvime alınmış bir denetim de yok**; bu, tarihi olan bir taahhüt değil, proje bir
denetimi finanse edebildiğinde ulaşılacak bir hedef. Her sözleşme, denetim raporu ve
takip ettiğimiz sorunlar [denetimler ve bilinen sorunlar](/tr/docs/security-audits)
sayfasında.

## Kendiniz bakın

Hesabınız zincir üstünde. Dağıtıldıktan sonra adresinizi bir blok gezgininde açın:
uygulaması (implementation) Safe'in kanonik SafeL2 v1.4.1 dağıtımı olan bir Safe
proxy'sidir; her ağda.

Sırada: [denetimler ve bilinen sorunlar](/tr/docs/security-audits).
