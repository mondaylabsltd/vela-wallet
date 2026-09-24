---
title: Vela'yı kurun
description: "Vela'yı çalıştırmanın bütün yolları — web, tarayıcı uzantısı, masaüstü ve telefon — her birinin maliyeti, neler yapabildiği ve cihazınızın neye ihtiyacı olduğu."
source: fa80f5cfdb95
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Vela'yı kurun

Aynı cüzdan birkaç yerde çalışır ve hepsi aynı anahtarlarla aynı adresi açar.
İhtiyacınıza göre seçin; birden fazlasını da kullanabilirsiniz. İndirmeler
[Vela'yı edinin](/tr/get-started) sayfasında.

| | Nedir | Maliyet | Durum |
| --- | --- | --- | --- |
| **Web** | Güncel herhangi bir tarayıcıda [wallet.getvela.app](https://wallet.getvela.app/) | Ücretsiz | Yayında |
| **Tarayıcı uzantısı** | Araç çubuğunuzdaki cüzdan; dApp'lere bağlanır | Ücretsiz | İndirip elle yükleyin; henüz Chrome Web Mağazası'nda değil |
| **Masaüstü** | macOS, Windows ve Linux için yerel uygulama | Ücretsiz | Vela'yı edinin sayfasından ya da GitHub'dan indirin |
| **iPhone, Android** | Yerel uygulamalar | Mağazalarda tek seferlik satın alma | Henüz mağazalarda değil; kaynak koddan kendiniz derleyebilirsiniz |

<a href="https://wallet.getvela.app/" target="_blank" rel="noopener" style="display:inline-block;margin:4px 0 8px;padding:11px 22px;border-radius:10px;background:#e8572a;color:#fff;font-weight:600;text-decoration:none;">Web cüzdanını açın →</a>

## Web

Kurulacak bir şey yok. [wallet.getvela.app](https://wallet.getvela.app/) adresini
açın, bir cüzdan oluşturun ya da giriş yapın; cüzdanınız orada. Hesap listeniz bu
tarayıcıda tutulur; başka bir cihazda anahtarlarınızdan biriyle yeniden giriş
yapmanız yeterli.

## Tarayıcı uzantısı

Chromium tabanlı tarayıcılar: Chrome, Edge ve Brave (Chrome 116 ya da sonrası).
Cüzdanı araç çubuğuna koyar ve dApp'lerin ona doğrudan bağlanmasını sağlar. Chrome
Web Mağazası'na gelene kadar:

1. Uzantıyı [Vela'yı edinin](/tr/get-started) sayfasından indirin ve saklayacağınız
   bir klasöre çıkarın — tarayıcı onu oradan çalıştırır.
2. `chrome://extensions` adresini açın ve **Geliştirici modu**'nu etkinleştirin.
3. **Paketlenmemiş öğe yükle**'ye tıklayın ve o klasörü seçin.

Bu, aynı cüzdandır: uzantı ile web cüzdanı aynı `getvela.app` geçiş anahtarlarını
kullanır, yani aynı anahtarlar aynı adresi açar.

## Masaüstü

Bir penceredeki web sayfası değil, yerel bir uygulama: **Windows** 10 ve 11 (x64 ve
ARM), **macOS** 11 ya da sonrası ve **Linux** (.deb, .rpm ya da Flatpak; x64 ve ARM).

- **Windows**, yükleyici henüz kod imzalı olmadığı için "bilgisayarınızı korudu"
  uyarısı verir. **Ek bilgi**'yi, ardından **Yine de çalıştır**'ı seçin.
- **macOS** derlemeleri ayrı bir adımda Apple tarafından imzalanıp onaylanır
  (notarization); bu yüzden diğer platformların gerisinde kalabilir. Mac düğmesinde
  "çok yakında" yazıyorsa, onaylanmış en son Mac derlemesi GitHub sürümler
  sayfasındadır.
- **Linux**: USB güvenlik anahtarı kullanmak için sisteminizin uygulamaya o anahtara
  erişim izni vermesi gerekir — .deb ve .rpm paketleri bu kuralı sizin için kurar.

macOS ve Windows'ta masaüstü uygulamasının dApp'ler için yerleşik bir tarayıcısı var.
Her paketin sağlama toplamı
[GitHub sürümler sayfasında](https://github.com/mondaylabsltd/vela-wallet/releases) —
ve bir sağlama toplamından fazlasını da kontrol edebilirsiniz, aşağıya bakın.

## iPhone ve Android

iOS 17.4 ya da sonrası ve Android 10 ya da sonrası için yerel uygulamalar. App Store
ve Google Play'de tek seferlik satın alma olarak satılacaklar; **henüz mağazalarda
değiller**. Kod açık olduğu için onları kendiniz ücretsiz derleyebilirsiniz — tek bir
farkla: kendi imzaladığınız bir derleme, getvela.app cüzdanları için telefonunuzun
kendi geçiş anahtarlarını kullanamaz; ama başka bir telefonla QR kod okutmak ve USB
güvenlik anahtarları çalışır. Bkz.
[uygulamaları kendiniz derleyin](/tr/docs/self-hosting#web-app).

## İndirdiğinizi doğrulayın

Bir sağlama toplamı size yalnızca iki dosyanın aynı olduğunu söyler. Dosyayı kimin
yaptığını söyleyemez — üstelik sağlama toplamlarının listesi indirmeyle aynı sayfada
durur. Bu yüzden bir sürüme eklediğimiz her paket aynı zamanda **attestation**
taşır: onu derleyen iş akışı çalışması, dosyayı, commit'i ve çalışmayı adlandıran bir
beyanı imzalar ve GitHub bunu saklar. Kontrol etmek,
[GitHub CLI](https://cli.github.com) ile tek bir komut alır (`gh auth login` ile bir
kez oturum açın; kontrol ücretsizdir):

```bash
gh attestation verify vela-wallet_0.9.4_amd64.deb --repo mondaylabsltd/vela-wallet
```

Dosyayı kimin, hangi commit'ten derlediğini yazdırır ya da başarısız olur. Bu yanıt
için makinenizin bize güvenmesi gerekmez: imza GitHub'ındır, derleme anında atılmıştır
ve dosyayı bir yere yeniden yükleyen biri tarafından üretilemez.

Mac imajları Developer ID'mizle imzalanır ve Apple tarafından onaylanır
(notarization); siz bir imajı açtığınızda macOS bunu sizin için kontrol eder. Kendiniz
sormak için:

```bash
xcrun stapler validate VelaWallet-0.9.4-macos-arm64.dmg
spctl -a -t open --context context:primary-signature -v VelaWallet-0.9.4-macos-arm64.dmg
```

<Callout type="warning" title="Windows uyarısı yerinde kalıyor">
Attestation, kod imzalama değildir. Windows yükleyicisi kod imzalı olmadığı için
SmartScreen onu bir kez yine "Windows bilgisayarınızı korudu" diyerek durdurur —
<strong>Ek bilgi</strong>'yi, ardından <strong>Yine de çalıştır</strong>'ı seçin.
Dosyanın gerçekten bize ait olduğunu söyleyen kontrol, attestation doğrulamasıdır;
uyarı ise satın almadığımız bir sertifikayla ilgilidir.
</Callout>

Bu açılmadan önce yayımlanan paketler yalnızca sağlama toplamlarını taşır.

## Vela'yı dApp'lerle kullanma

<span id="dapps"></span>

dApp'ler Vela'ya, herhangi bir tarayıcı cüzdanına bağlandıkları gibi bağlanır
(EIP-1193 ve EIP-6963):

- masaüstü tarayıcıda **Vela tarayıcı uzantısı** üzerinden;
- **masaüstü uygulamasında** (macOS, Windows), **iPhone uygulamasında** ve **Android
  uygulamasında**, yerleşik tarayıcıları üzerinden.

wallet.getvela.app adresindeki web cüzdanı dApp'lere bağlanmaz ve WalletConnect
desteği yoktur. Bir dApp'in yaptığı her istek, siz imzalamadan önce çözülüp size
gösterilir — bkz. [açık imzalama](/tr/docs/clear-signing).

## Cihazınızın neye ihtiyacı var

Vela **geçiş anahtarlarıyla** imzalar; son birkaç yılın neredeyse her cihazı
bunları destekler:

| Cihaz | Desteklenen |
| --- | --- |
| iPhone, iPad, Mac | iOS / iPadOS 16+, macOS'ta güncel bir Safari ya da Chrome |
| Android | Google Play hizmetleri olan güncel bir Android ya da bir USB güvenlik anahtarı |
| Windows | Chrome ya da Edge ile Windows Hello veya bir güvenlik anahtarı |
| Linux | Bir güvenlik anahtarı ya da yakındaki bir telefon (QR kodu okutun) |

Cihazınız geçiş anahtarını kendisi tutamıyorsa başka bir telefon ya da bir donanım
güvenlik anahtarı kullanın. Hangi uygulamanın hangi tür anahtarları desteklediği
[İmzalayıcılar ve güvenlik anahtarları](/tr/docs/signers) sayfasında.

## Tek resmî adresler

- **getvela.app** — bu site ve indirmeler
- **wallet.getvela.app** — web cüzdanı
- **github.com/mondaylabsltd** — kod ve sürüm paketleri

<Callout type="warning" title="Kurmadan önce kontrol edin">
Bir şey sizi "Vela'yı kurmak" ya da "cüzdanınızı doğrulamak" için başka bir yere
yönlendiriyorsa durun. Vela hiçbir zaman kurtarma ifadesi istemez — zaten bir
kurtarma ifadesi yok.
</Callout>

Sırada: [cüzdanınızı oluşturun](/tr/docs/create-wallet).
