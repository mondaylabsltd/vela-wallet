---
title: Ağlar ve ücretler
description: "Vela'ya yerleşik 24 ağ, başka bir ağın nasıl ekleneceği, bir işlemin ücretinin tam olarak nasıl hesaplandığı ve kime gittiği, bir relay'in gas'ı bittiğinde ne olduğu."
source: fdc50dbbf13a
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Ağlar ve ücretler

## Yerleşik ağlar

Vela'da **24 ağ** yerleşik olarak gelir; hepsi ana ağdır:

| Ağ | Gas ne ile ödenir | Ağ | Gas ne ile ödenir |
| --- | --- | --- | --- |
| Ethereum | ETH | Arc | USDC (yerel coin) |
| BNB Chain | BNB | X Layer | OKB |
| Polygon | POL | Stable | USDT0 (yerel coin) |
| Arbitrum | ETH | Soneium | ETH |
| Optimism | ETH | MegaETH | ETH |
| Base | ETH | Robinhood Chain | ETH |
| Avalanche | AVAX | Mantle | MNT |
| Gnosis | xDAI | Kaia | KAIA |
| Unichain | ETH | Celo | CELO |
| Tempo | pathUSD (yerel coin yok) | Ink | ETH |
| Monad | MON | Plume | PLUME |
| World Chain | ETH | XRPL EVM | XRP |

Çoğunda ücreti, relay'in o ağda kabul ettiği bir USD stabilcoiniyle de
ödeyebilirsiniz (aşağıya bakın).

Cüzdanınızın **her ağda aynı adresi** vardır, çünkü adres zincirden değil,
anahtarlarınızdan hesaplanır.

## Başka bir ağ eklemek

**Ayarlar → Ağlar** bölümünden herhangi bir EVM ağını ekleyebilirsiniz; yeter ki
ağda bir Vela cüzdanının ihtiyaç duyduğu her şey olsun: on bir standart sözleşme
(ERC-4337 EntryPoint v0.7, Safe v1.4.1 sözleşmeleri, Safe'in 4337 ve geçiş anahtarı
modülleri, MultiSend, Multicall3 ve iki deterministik dağıtıcı) ve geçiş anahtarı
imzalarını `0x100` adresinde doğrulayan **RIP-7212** ön derlemesi. Cüzdan, ağı
eklemenize izin vermeden önce bunların hepsini kontrol eder; buna ön derlemeye karşı
gerçek bir imza denemesi de dahildir.

Ön derleme kesin bir gereksinimdir. Adresi, her Vela adresinin hesaplanmasının bir
parçasıdır; bu yüzden yedek bir doğrulayıcı yoktur ve sonradan bir tane dağıtmanın
da yolu yoktur. Bir zincirde ön derleme var ama sözleşmelerin bir kısmı eksikse,
[zincir kurulumu](/tr/chain-setup) neyin eksik olduğunu gösterir ve herkesin
dağıtabileceği olanları dağıtır. Kontrolde bir boşluk var: birden fazla anahtarı olan
bir cüzdan, ağda Safe'in geçiş anahtarı imzalayıcı fabrikasına da ihtiyaç duyar ve bu
henüz kontrol edilmiyor; o fabrika yoksa orada yalnızca ilk anahtar imzalayabilir.

## Bir işlemin bedeli nasıl ödenir

Vela bir ERC-4337 cüzdanıdır: işlemi kendiniz yayınlamazsınız. Uygulama bir
**UserOperation** oluşturur, siz onu anahtarlarınızdan biriyle imzalarsınız ve bir
**relay** onu zincire gönderip gas bedelini peşin öder. (ERC-4337 bu role bundler
der.) Relay'in alacağı **işleminizin içinde** ödenir: ödeme, cüzdanınızdan relay'e
yapılan ve işleminizle aynı toplu işlemde duran bir transferdir; dolayısıyla
imzanızın kapsamındadır. Paymaster yoktur; gas ücretinizi kimse sponsorlamaz ve kimse
bir sponsorluk politikası yüzünden işleminizi reddedemez.

### Ücret nedir

<span id="fee"></span>

Onay ekranı tek bir tutar gösterir; hem ücret coininde hem de görüntüleme para
biriminizde. Şöyle hesaplanır:

- **Cüzdanın ayırdığı gas.** Cüzdan işlemi simüle eder ve kullanmayı beklediğinden
  fazla gas ayırır: doğrulama ve yürütme tahminlerinin her biri yarı yarıya artırılır
  ve alt sınırlar uygulanır (örneğin cüzdan dağıtıldıktan sonra doğrulama en az
  300.000 gas, cüzdanı dağıtan işlem için 2.000.000 gas).
- **Gas fiyatı.** Cüzdanın ağın gas fiyatı için kendi okuması ile relay'in seçtiğiniz
  hız için verdiği fiyattan yüksek olanı. Varsayılan hız *hızlı*dır; relay bunu
  yaklaşık 1,8 × taban ücret artı iki katı öncelik ücreti olarak fiyatlar.
- **Ücret = 3 × ayrılan gas × gas fiyatı**; asgari yaklaşık 0,01 dolar. Tempo'da çarpan
  2'dir ve ücret pathUSD ile ödenir.

Ayrılan miktar işlemin kullanacağının epey üstünde tutulur ve fiyata pay bırakılır;
bu yüzden ücret, işlemin zincir üstü maliyetinden fazladır — bir ağdaki ilk
işleminizde, cüzdanınızı da dağıttığı için, daha da fazladır. Gerçek maliyeti relay
öder ve gerisini kendisi tutar; hiçbir şey iade edilmez. Ucuz ağlarda bu birkaç
senttir; Ethereum ana ağında ciddi bir tutar olabilir. Tahmin yürütmeniz gerekmez:
kesin tutar, siz imzalamadan önce onay ekranındadır.

**Ücret kime gider.** Ücret, cüzdanın ayarlı olduğu relay'i kim çalıştırıyorsa ona
gider — siz değiştirmedikçe Vela'ya. Herhangi bir vela-relay dağıtımı kullanılabilir,
[kendi çalıştırdığınız](/tr/docs/self-hosting#relay) da dahil; hangi relay'i seçerseniz
seçin cüzdan aynı formülü kullanır.

<Callout type="info" title="Ne görüyorsanız onu ödersiniz">
Ücret tutarı ve gittiği adres, imzaladığınız işlemin parçasıdır. Bunlardan birini
değiştiren bir relay imzanızı geçersiz kılar; bu yüzden tam olarak gösterilen tutarı
ödersiniz — işlem zincire girmeden gas yükselse bile fazlasını değil. Relay'in gas
fiyatı teklifi cüzdanın kendi okumasının üç katından fazlaysa reddedilir.
</Callout>

### Ne ile ödeyebilirsiniz

- Ağın **yerel coini** ile, her zaman.
- Relay yerel coini fiyatlayabildiğinde, relay'in o ağ için listesindeki bir **USD
  stabilcoini** ile. Hiç tutmadığınız stabilcoinler gizlenir.
- Yerel coini olmayan **Tempo**'da yalnızca **pathUSD** ile.

Ücret coinini ve hızı (*yavaş*, *standart* ya da *hızlı*) onay ekranında ve
Ayarlar'da seçersiniz.

### Bir ağdaki ilk işleminiz

Cüzdanınız bir ağda var olmadan önce de o ağda para alabilirsiniz. Bir ağdan ilk kez
gönderim yaptığınızda o işlem cüzdan sözleşmenizi de dağıtır (ve her ek anahtar için
küçük birer imzalayıcı sözleşme). Dağıtımın gas bedeli o işlemin ücretine dahildir; bu
yüzden her ağdaki ilk gönderim, sonrakilerden daha pahalıdır.

Bir yerel coinin **azami** tutarını gönderdiğinizde Vela ücret için yetecek kadarını
ayırır.

## Relay'i kim çalıştırıyor — ve ücreti kim alıyor

Varsayılan olarak her ağ **Vela'nın relay'ini** kullanır ve ücret Vela'ya gider.
Cüzdanı **Ayarlar → Gelişmiş → Servis Uç Noktaları** bölümünden başka bir relay'e
yönlendirebilirsiniz; tek bir adres bütün yerleşik ağlara hizmet verir, özel bir ağ
ise eklendiği sıradaki relay adresini korur. Relay'in
[vela-relay](https://github.com/mondaylabsltd/vela-relay) olması gerekir — Vela'nınki
ya da sizin çalıştırdığınız — çünkü cüzdan ücret teklifini Vela'ya özgü bir yöntemle
ister ve Pimlico ya da Alchemy gibi genel bundler'lar bu yöntemi uygulamaz.
Kullandığınız relay'i kim çalıştırıyorsa ücreti o alır; nasıl çalıştırılacağını
[kendi sunucunuzda barındırma kılavuzu](/tr/docs/self-hosting#relay) anlatıyor.

Relay, zaten imzalanmış bir işlem alır. Alıcıyı, tutarı, ücreti ya da başka herhangi
bir şeyi değiştiremez. İşlemi geciktirebilir ya da reddedebilir ve zincire ne zaman
gireceğini o seçer — dolayısıyla bir takasta ilke olarak, kayma toleransınız içinde
sizden önce işlem yapabilir.

### Bir relay'in gas'ı bittiğinde

Bir relay, gas bedelini her ağdaki kendi **kasasından** öder. Kasa boşsa, gönderme
ekranı bunu siz imzalamadan önce söyler:

- Vela'nın relay'inin hizmet verdiği bir ağda kasayı relay'in işletmecisinin (Vela)
  doldurması gerekir; durumu bildirebilirsiniz. Bekleyemiyorsanız, **isteğe bağlı
  olarak** kasaya kendiniz az miktarda yerel coin gönderebilirsiniz. Bu katkı **iade
  edilmez** ve kendi işleminizin ücretini **karşılamaz**.
- Özel bir ağda relay'i fonlamak, onu kim çalıştırıyorsa ona kalmıştır — bu siz de
  olabilirsiniz.

Cüzdan başına bir gas hesabı ya da etkinleştirme depozitosu yoktur: Vela'nın eski bir
sürümünde vardı, artık yok.

## Vela her ağı nasıl okur

Vela bakiyeleri okur ve işlemleri simüle ederken her ağ için bir **RPC uç noktası
havuzu** kullanır — yerleşik uç noktalar, herkese açık yedekler ve sizin eklediğiniz
sağlayıcı anahtarları ya da uç noktalar — ve bir uç nokta yavaşladığında ya da
çöktüğünde bir sonrakine geçer. Her ağ için kendi uç noktanızı **Ayarlar → Ağlar**
bölümünden tanımlayabilirsiniz. (Android uygulaması şu anda her ağ için tek bir uç
nokta kullanıyor ve otomatik geçiş yapmıyor; iPhone uygulaması ise bunu değiştirmenize
henüz izin vermiyor.)

Sırada: [geçiş anahtarları nasıl çalışır](/tr/docs/passkeys).
