---
title: Ağlar ve ücretler
description: Vela'nın desteklediği 12 ağ, hesap soyutlamasında gaz ücretlerinin nasıl işlediği, relay'i kimin çalıştırıp ücretleri kimin aldığı, gaz hesabı etkinleştirmesini ne zaman kendiniz ödediğiniz ve Vela'nın RPC uç noktalarını nasıl seçtiği.
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Ağlar ve ücretler

## Desteklenen ağlar

Vela, içinde **12 EVM ağıyla** geliyor:

| Ağ | Yerel ücret tokenı |
| ----------- | ---------------- |
| Ethereum | ETH |
| BNB Chain | BNB |
| Polygon | POL |
| Arbitrum | ETH |
| Optimism | ETH |
| Base | ETH |
| Avalanche | AVAX |
| Gnosis | xDAI |
| Unichain | ETH |
| Tempo | USD |
| Monad | MON |
| World Chain | ETH |

Cüzdanınızın **hepsinde aynı adresi** var; yani paylaşacağınız tek bir adres
oluyor.

Ayrıca **kendi ağlarınızı da ekleyebilirsiniz** (Ayarlar → Ağlar). Vela bir akıllı
hesap cüzdanı olduğu için, bir ağın Vela'nın dayandığı sözleşmeleri sunması gerekir:
ERC-4337 EntryPoint, Safe sözleşmeleri ve geçiş anahtarınızı zincir üstünde
doğrulayan **P-256 (RIP-7212)** imza ön derlemesi. Vela bir ağ eklemenize izin
vermeden önce bunu otomatik olarak kontrol eder.

<Callout type="info" title="Gnosis neden bu kadar çok geçiyor">
12 ağdan biri olmasının ötesinde, Gnosis Chain Vela'nın <strong>Geçiş Anahtarı
Dizini</strong>'ni barındırıyor — cihazlar arası kurtarma için genel anahtarınızı
ve hesap adınızı saklayan sözleşmeyi. Bu, hangi ağda işlem yaptığınızdan bağımsız.
</Callout>

## Ücretler nasıl işliyor (hesap soyutlaması)

Vela **ERC-4337 hesap soyutlaması** kullanıyor; yani işlemi doğrudan siz yayınlamış
olmuyorsunuz — işlem, bir **relay**'e teslim edilen bir **UserOperation** ve relay
onu zincire gönderip gaz masrafını geri alıyor. (ERC-4337 spesifikasyonu bu role
*bundler* diyor. Vela'nınkine relay deniyor, çünkü paketlemekten fazlasını yapıyor:
ücreti kanal içinde bildiriyor ve aşağıdaki gaz hesabı protokolünü işletiyor; ikisi
de standardın parçası değil.) Bundan birkaç sonuç çıkıyor:

- **Gaz, kendi cüzdan bakiyenizden ödenir** — varsayılan olarak ağın yerel
  tokenıyla (ETH, BNB, xDAI…) ya da relay'in sunduğu yerlerde desteklenen bir
  stabilcoinle; ücret varlığını onay ekranında siz seçersiniz. Tempo'nun yerel
  parası yok, orada gaz her zaman dolar stabilcoinleriyle ödenir. Her işlemi
  sponsorlayan — ya da kapılayan — bir ERC-4337 **paymaster**'ı yok. (Vela, yeni
  kullanıcılar için tek seferlik _gaz hesabı etkinleştirmesini_ üstlenebilir; o
  ayrı bir konu ve aşağıda anlatılıyor.)
- **Gaz fiyatını relay bildirir** — tek doğruluk kaynağı odur; cüzdan da o fiyatı
  gösterir ve tam olarak gösterdiğini imzalar. Hız seçici yoktur: her işlem yüksek
  öncelikle gönderilir.
- Toplam tutar, **ağ maliyeti artı relay'in hizmet ücretidir**; çok ucuz işlemlerde
  küçük bir asgari ücretle. Relay'in bildirdiği fiyat, fiyatın kendisidir — bakılacak
  ayrı bir ücret tarifesi yoktur. Bir kısmı ağın doğrulayıcılarına gider; kalanı gazı
  peşinen ödeyen ve altyapıyı çalıştıran relay'e.
- Onay ekranı, siz imzalamadan önce **tahmini ücreti** hem ücret varlığında hem de
  görüntüleme para biriminizde gösterir. Bildirilen tutar ve alıcısı, imzaladığınız
  şeyin parçasıdır; yani relay'e tam olarak gösterilen kadar ödenir — değişen bir
  rakam imzanızı geçersiz kılardı.

## Relay'i kim çalıştırıyor — ve ücretleri kim alıyor

Her ağ bir relay'e işaret eder. Varsayılan olarak bu **Vela'nın kendi relay'i**dir
ve uç noktayı _Ayarlar → Gelişmiş → Servis Uç Noktaları_ altından
değiştirebilirsiniz. Tek bir uç nokta bütün yerleşik ağlar için geçerlidir;
eklediğiniz özel bir ağ ise eklerken verdiğiniz relay adresini korur.

Uyumluluk konusunda dürüst bir uyarı: uygulama ücreti Vela'ya özgü bir RPC
yöntemiyle (`vela_getInBandGasQuote`) alıyor ve gönderim akışı o yöntem olmadan
çalışmıyor. Yani işaret ettiğiniz uç noktanın
[vela-relay](https://github.com/mondaylabsltd/vela-relay) çalıştırıyor olması
gerekir — Vela'nın örneği ya da sizin barındırdığınız bir örnek. **Pimlico** veya
**Alchemy** gibi genel bir ERC-4337 bundler'ı bu yöntemi uygulamıyor, dolayısıyla
mevcut sürümde baştan sona çalışmaz.

Bir ağın relay'ini kim işletiyorsa **o ağın ücretlerini de o alır**: her işlemdeki
relay marjını ve gaz hesabı etkinleştirme depozitosunu. Kendi vela-relay'inizi
çalıştırın, o ücretler Vela'nın değil sizin altyapınızı finanse etsin; başka yere
yönlendirdiğiniz trafikten Vela pay almaz.

<Callout type="warning" title="Gaz hesabı, vela-relay protokolünün bir parçası">
<strong>Gaz hesabı etkinleştirme</strong> adımı, her ağda cüzdanınız için ayrılmış
bir relay hesabını fonluyor. Uç noktayı kendi barındırdığınız bir vela-relay'e
yönlendirirseniz depozito Vela'nın değil, kendi relay'inizin hesabını fonlar.
</Callout>

### Gaz hesabını etkinleştirmek (Vela Relay)

Vela'nın relay'inde, her ağdaki ilk işleminiz **ayrılmış bir gaz hesabını
etkinleştirir**. Uygulama önce relay'in kasasından bunu sizin için karşılamasını
ister — bu, gönderim akışının içinde sessizce olur ve sponsorlanan bir cüzdan hiç
fonlama ekranı görmez. Yalnızca sponsorluk reddedildiğinde uygulama bir yükleme
isteği gösterir: gösterdiği gaz hesabı adresine az miktarda yerel token
gönderirsiniz ve uygulama size sponsorluğun neden verilmediğini söyler.

**Ücretsiz sponsorluk sunulmadığında etkinleştirme ücretini siz ödersiniz**; yani
şu durumlarda:

- **Vela'nın o ağdaki kasası boş ya da azalmış** — o zincirdeki ücretsiz fon geçici
  olarak tükenmiş.
- **Ücretsiz kotanızı kullanmışsınız** — sponsorluk cüzdan başına sınırlıdır, ilk
  birkaçtan sonrası size aittir.
- **Vela'nın relay'i o ağı hiç fonlamıyor** — örneğin **kendi eklediğiniz özel ya da
  test ağları**; Vela bunlar için kasa tutmaz. (Etkinleştirmeyi tümden atlamak
  isterseniz bunları kendi relay'inize yönlendirin.)

Etkinleştirme depozitosu **iade edilmez** — relay'in başlangıç bakiyesidir ve zamanla
gaz iadelerinden kendini besler; yine de tükenip sonradan **yeniden
etkinleştirme** gerektirebilir. Servis yükseltmesinde relay adresi de değişebilir,
bu da yeni bir etkinleştirme ister.

Ücret, seçtiğiniz **ücret varlığındaki** bakiyenizden düşer — varsayılan olarak yerel
token. Bir gönderim gaz yüzünden engelleniyorsa, o ücret varlığındaki bakiyeniz
ücreti karşılamıyor demektir; relay'in stabilcoinle gaz sunduğu yerlerde onay
ekranında ücret varlığını değiştirmek engeli kaldırabilir.

Bir yerel tokenın **azami** tutarını gönderdiğinizde Vela, işlem başarısız olmasın
diye gaz için gerekeni otomatik olarak ayırır.

## Vela her ağla nasıl konuşuyor

Vela bakiyeleri okumak ve işlem göndermek için tek bir sağlayıcıyı değil, bir **RPC
uç noktası havuzunu** kullanır. Uç noktaları birkaç kaynaktan toplar, gecikme ve
güvenilirliğe göre puanlar ve biri yavaşladığında ya da düştüğünde **otomatik olarak
devreder** — kötü uç noktaları geçici olarak kenara alır — böylece tek bir aksak
düğüm uygulamayı asla devre dışı bırakmaz.

Sırada: [geçiş anahtarları nasıl çalışır](/tr/docs/passkeys).
