---
title: Trusted Signer
description: "sign.getvela.app adresinde, bir isteği kendi başına çözen ve geçiş anahtarınızla imzalayan tek dosyalık bir sayfa — neyi kontrol ettiği, hangi uygulamaların onu kullandığı ve onu nasıl yeniden derleyeceğiniz ya da kendi kopyanızı nasıl çalıştıracağınız."
source: 43a1af6ffdcd
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Trusted Signer

Vela her işlemi siz onaylamadan önce çözer ve bu çözümleme dürüst bir iştir — ama
işlemi oluşturan uygulamanın kendisinin yaptığı bir iştir. Uygulamaya ya da size
ulaşma yoluna müdahale edilmişse, size bir şey gösterip başka bir şey imzalayabilir.
[Bybit'te](/tr/docs/bybit-attack) tam olarak bu oldu.

Trusted Signer bunu ikiye bölmek için var: uygulama yalnızca isteği teslim eder;
kontrol ve imza ise ayrı bir sayfada gerçekleşir — baştan sona okuyabileceğiniz, bayt
bayt yeniden derleyebileceğiniz ya da kendiniz çalıştırabileceğiniz bir sayfada.

## Nerede çalışır

Resmî sayfa **sign.getvela.app** adresinden sunulur. Masaüstü (macOS, Windows, Linux),
iPhone ve Android uygulamaları ona bir istek gönderebilir: uygulama sayfayı, istek
bağlantının içinde olacak şekilde bir tarayıcı sekmesinde açar; isteği orada kontrol
eder ve geçiş anahtarınızla orada imzalarsınız; sayfa da imzayı bir `velawallet://`
bağlantısıyla uygulamaya geri verir. Web cüzdanı onu kullanamaz.

Kullanımı isteğe bağlıdır. Bir cüzdan oluştururken ya da giriş yaparken onu imzalama
yönteminiz olarak seçersiniz; o andan itibaren o cüzdan için o cihazda atılan her imza
ondan geçer. Cüzdanın anahtarlarını da oluşturabilir. `sign.getvela.app` üzerinde
uygulamalarla aynı `getvela.app` geçiş anahtarlarını kullanır.

<Callout type="info" title="Şimdiye kadar test edilenler">
Yayımlanmış sayfaya karşı kaydı tutulmuş uçtan uca çalıştırmalar: Android ve Windows 11
(cüzdan oluşturma ve giriş yapma). macOS, Linux ve iPhone uygulamaları aynı düzeneği
kullanıyor; hiçbirinin henüz kaydı tutulmuş eksiksiz bir çalıştırması yok.
</Callout>

## İmzalamadan önce ne yapar

- **İsteği kendisi çözer.** Çağrının ne yaptığını, kime ve ne kadar olduğunu
  calldata'dan okur — bir toplu işlemin içine yerleştirilmiş çağrılar dahil.
- **Yalnızca kendi hesapladığı bir özeti imzalar.** EIP-191, EIP-712, SafeOp ve
  SafeMessage özetleri sayfanın içinde hesaplanır, asla isteyen taraftan alınmaz;
  testler SafeOp ve SafeMessage özetlerini cüzdanın kullandığı kod olan `vela-core` ile
  karşılaştırır ve uygulama, kendi hesapladığı özetten başka herhangi bir özet
  üzerindeki imzayı reddeder.
- **İşlemin istenen işlem olduğunu kontrol eder.** Sitenin istediği çağrının,
  imzalanan işlemin içinde gerçekten bulunması gerekir; yoksa sayfa reddeder.
- **Bir onayın sınırsız olduğunu söyler.** Bir tutarı değiştiremez — gelen baytları
  imzalar ya da hiçbir şey imzalamaz — bu yüzden sınırsız bir onay ya da izin (bu
  sayfada 2^128 ya da daha fazlası) bu gerekçeyle birlikte kırmızıyla gösterilir ve
  olduğu haliyle imzalanabilir; zincir üstü bir üst sınır, istek buraya ulaşmadan önce
  cüzdanın kendi onay ekranında seçilir. Bütün bir NFT koleksiyonu için verilen bir onay
  reddedilir.
- **Arkasında duramayacağı şeyleri reddeder:** `eth_sign`, tanımadığı bir yöntem,
  token'ın kendi sözleşmesine gönderilen bir token, okuyamadığı bir işlem ve sınaması
  (challenge) isteyen tarafça verilmiş bir giriş.
- **Hesabını başkasına teslim edecek şeyleri reddeder,** uygulamaların uyguladığı
  kuralla: hesabının kendi sahip, modül, guard veya fallback işlevlerinden birini
  çağırması (toplu bir işlemin içinde de); Safe'in, bir işlemin çağrılarını bir araya
  getiren MultiSend sözleşmesine yapılan dışında her `delegatecall`; ve bir `SafeTx`
  imzası. Yalnızca sitenin istediği çağrıları değil, uygulamanın oluşturduğu işlemdeki
  her çağrıyı kontrol eder.
- **Hesabın adresini ve sayfada hesaplanan bir identicon'u gösterir.** Alıcılar ve
  sözleşmeler asla istekten gelen bilgiyle adlandırılmaz — bir sözleşmeye ad
  verebilecek tek şey sayfanın kendi incelenmiş tablosudur. Uygulamanın, doğru geçiş
  anahtarını seçebilmeniz için gönderdiği hesap adı, adresin yanında gösterilir.
- **Her imzada kullanıcı doğrulaması ister** (parmak iziniz, yüzünüz ya da PIN'iniz).

## Bilerek sahip olmadığı şeyler

- **Düzenleyici yok.** İstek geldiği anda sabittir: ya imzalarsınız ya imzalamazsınız.
  Bir ücret seçici ya da harcama izni düzenleyicisi calldata'yı yeniden yazardı; bu
  sayfanın önlemek için var olduğu hastalık da tam olarak bu.
- **Ağ erişimi yok.** Sayfa, içerik güvenliği politikası (`default-src 'none'`) kendi
  baytlarının içinde olan tek bir dosyadır; bu yüzden hiçbir şey çekemez, bağlantı
  açamaz ya da görsel yükleyemez. Ondan dışarı çıkan tek şey, istekteki geri çağrı
  bağlantısını izlediğinde verdiği yanıttır (isteyen bir Vela uygulamasıysa
  `velawallet://`). Token logoları harf olarak çizilir.

## Uygulama karşılığında neyi kontrol eder

Uygulama da sayfaya güvenmez. Bir imzayı ancak imzalanan sınama **uygulamanın
hesapladığı** özetse, kullanıcı doğrulaması yapılmışsa, anahtar cüzdanınızın
anahtarlarından biriyse ve P-256 imzası o anahtarla doğrulanıyorsa kabul eder.

## Yayımlanan her sürüm, kontrol edilebilir

Her sürüm `app-web/trusted-signer/src/` içinden tek bir dosyaya, yeniden üretilebilir
biçimde derlenir — Bun ve Node aynı baytları üretir — ve önceki bütün sürümlerin
yanında kendi adresinde yayımlanır: `sign.getvela.app/b/<sha256>/sign.html`. Liste
`sign.getvela.app/index.json` adresindedir.

```sh
cd app-web/trusted-signer
node samples/build-single.mjs --check   # rebuilds a version listed in dist/
curl -sL https://sign.getvela.app/b/<sha256>/sign.html | shasum -a 256
```

Masaüstü uygulaması açılırken, açacağı yayımlanmış sürümü indirir, özetini (hash)
çıkarır ve kendi içine yerleşik sürümlerle karşılaştırır. Sonuç yalnızca günlüğe
yazılır ve eşleşmeyen bir sayfa yine de açılır. Telefon uygulamaları henüz bu kontrolü
yapmıyor.

## Kendi kopyanızı çalıştırın

Ayarlar, uygulamalarınızın açtığı sayfanın adresini saklar; böylece onu kendi
dağıtımınıza yönlendirebilirsiniz: herhangi bir HTTPS adresi ya da test için
`localhost`. Sayfayı `bun samples/build-single.mjs` (ya da `node`) ile derleyin ve
`dist/` klasörünü sunucunuza kopyalayın.

Kendi alan adınızdaki bir kopya, `getvela.app` geçiş anahtarlarıyla değil, **o** alan
adı için oluşturulmuş geçiş anahtarlarıyla imzalar — yani anahtarları sizin alan
adınız altında duran bir cüzdan oluşturup kullanmanın bir yoludur, mevcut bir
`getvela.app` cüzdanı için imzalamanın yolu değildir. Bir cüzdanın bütün anahtarları
tek bir alan adını paylaşır.

Kod ve onu derleyen ve kontrol eden betikler `app-web/trusted-signer/` içindedir.
