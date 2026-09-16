---
title: Hesap sözleşmesi
description: Vela cüzdanınız, değiştirilmemiş bir Safe v1.4.1. Sözleşme yolundaki hiçbir satır bizim elimizden çıkmadı — bunun size kazandırdıkları ve maliyeti burada.
---

# Hesap sözleşmesi

Cüzdanınız bir uygulamanın özel veri yapısı değil. **Safe v1.4.1** akıllı hesabı —
Vela'nın hiç göremeyeceği kadar büyük hazineleri tutan sözleşmenin aynısı — Safe'in
yayımladığı hâliyle, hiçbir değişiklik yapılmadan kuruluyor.

Bu cümle kısa, sonuçları değil; o yüzden bu sayfa onları tek tek yazıyor.

## Yoldaki hiçbir şey bize ait değil

Sizinle paranız arasında dört sözleşme duruyor. Vela bunların hiçbirini yazmadı:

| Sözleşme | Yazan |
| --- | --- |
| [Safe v1.4.1](https://github.com/safe-fndn/safe-smart-account/tree/release/v1.4.1) (hesabın kendisi, bir proxy) | Safe |
| [Safe 4337 Module](https://github.com/safe-global/safe-modules/tree/main/modules/4337) | Safe |
| [SafeWebAuthnSharedSigner](https://github.com/safe-global/safe-modules/tree/main/modules/passkey) (P-256 anahtarınızı doğrular) | Safe |
| [ERC-4337 EntryPoint v0.7](https://eips.ethereum.org/EIPS/eip-4337) | ERC-4337 yazarları |

Ortada bir Vela sözleşmesi yok. Depo hiç Solidity içermiyor — bunu tek komutla
kontrol edebilirsiniz:

```bash
git clone https://github.com/mondaylabsltd/vela-wallet
find vela-wallet -name '*.sol'   # hiçbir şey yazdırmaz
```

Vela bir ağ eklediğinde **o** sözleşmeleri kanonik adreslerine kuruyor. Kendi
tasarladığı bir sözleşmeyi kurmuyor ve sizinkinde ayrıcalıklı bir rolü yok: yönetici
anahtarı yok, yükseltme yolu yok, ekleyebileceğimiz bir modül yok.

## "Değiştirilmemiş" neden asıl önemli kelime

Bir sürü cüzdan "bir Safe" ya da "Safe'in bir çatalı" ya da "Safe'ten esinlenmiş bir
hesap" üzerine kurulu. Çatal, eski bir itibara sahip yeni bir sözleşmedir. Pratikte
farklar şunlar:

**Denetimler gerçekten kullandığınız şeyi kapsıyor.** Safe'in denetim raporları tam
olarak bu sürümlerdeki byte kodunu kapsıyor. Bir çatalın denetimleri ise çataldan
önceki kodu kapsar. Bir cüzdan hesap sözleşmesini değiştirmişse, andığı her denetim
başka bir şeyin denetimidir ve değişiklik de kimsenin bakmadığı kısımdır.

**Ekosistem hesabınıza Safe muamelesi yapıyor, çünkü o bir Safe.** Blok
gezginleri onu çözer. Safe'in kendi işlem araçları onu anlar. Vela yarın ortadan
kalkarsa cüzdanınız öksüz bir biçim olmaz — Ethereum'un en çok araç desteğine sahip
akıllı hesabıdır ve Safe uyumlu her arayüz onu sürebilir.
["Vela ortadan kalkarsa cüzdanınız kalkmaz"](/tr/docs/why-vela) cümlesini
niyetimizle ilgili değil, sözleşmelerle ilgili bir ifade yapan şey de budur.

**Saldırı yüzeyi, herkesin izlediği bir yüzey.** Kendine özel bir hesap sözleşmesi,
yalnızca yazarının baktığı bir sözleşmedir. Buna ise Safe'te parası olan herkes
bakıyor.

## Maliyeti ne

Standart olmak bedava değil ve takaslar gerçek:

- **Gaz.** Akıllı hesap, imzayı zincir üstünde doğrular. Zincire göre, düz bir EOA
  transferinin kabaca 1,5–3 katı gaz bekleyin. Bkz.
  [ağlar ve ücretler](/tr/docs/networks-and-fees).
- **Hesabın kurulması gerekir.** Adresiniz zincirde hiçbir şey yokken `CREATE2` ile
  hesaplanır, yani hemen para alabilirsiniz; ama giden ilk işlem sözleşmenin kurulum
  bedelini öder.
- **Her zincir uygun değil.** WebAuthn imzalayıcısı P-256 imzasını zincir üstünde
  doğrular, bunun için de **RIP-7212** ön derlemesi gerekir. Vela, zayıf bir
  doğrulayıcıya düşmektense o ön derlemesi olmayan bir ağı açmayı reddeder.
- **Safe'in riski artık sizin riskiniz.** Yaygın kullanılan bir sözleşmeye güvenmek
  de sonuçta bir sözleşmeye güvenmektir. Vela'nın söyleyebileceği şey, bunun üstüne
  güvenmeniz gereken ikinci bir şey eklememiş olmasıdır.

## Ne denetlendi, ne denetlenmedi

Safe'in sözleşmeleri ve WebAuthn imzalayıcı modülü üçüncü taraflarca denetlendi ve
o raporlar herkese açık. **Vela'nın kendi uygulama kodu bağımsız bir denetimden
geçmedi** ve planlanmış bir denetim de yok — bu, tarihi olan bir taahhüt değil,
proje karşılayabildiğinde ulaşılacak bir hedef. Vela'nın bağlı olduğu her sözleşme,
denetim raporu ve takip ettiğimiz sorunlar
[denetimler ve bilinen sorunlar](/tr/docs/security-audits) sayfasında.

## Kendiniz bakın

Hesabınız zincir üstünde. Bir blok gezgininde açıp uygulama adresini okuyun:
Vela'nın desteklediği her ağda, byte byte, Safe'in v1.4.1 kanonik kurulumu olacak.
