---
title: Geçiş anahtarları nasıl çalışır
description: "Geçiş anahtarı nedir, her anahtar türünde özel anahtar nerede durur, neden oltalanacak bir sır yoktur ve bir geçiş anahtarı sizi neye karşı korumaz."
source: b23999b2ed69
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Geçiş anahtarları nasıl çalışır

Bir Vela cüzdanını kontrol eden anahtarlar **geçiş anahtarlarıdır**: P-256 eğrisi
üzerindeki WebAuthn kimlik bilgileri. Her birini cihazınız ya da güvenlik anahtarınız
oluşturur, özel anahtarı saklar ve onu yalnızca siz Face ID, parmak izi, cihaz
PIN'iniz ya da güvenlik anahtarına dokunup PIN girerek onayladıktan sonra kullanır.
Vela özel anahtarı hiçbir zaman almaz; bir parola yöneticisi onu eşitliyorsa, onu
sizin adınıza şifreli olarak o parola yöneticisi tutar.

## Geçiş anahtarı nedir

Geçiş anahtarı, tek bir web sitesi için oluşturulan bir açık/özel anahtar çiftidir —
Vela için bu site `getvela.app`. Bir uygulama özel anahtarı hiçbir zaman almaz; yalnızca
kimlik doğrulayıcıdan bir şeyi imzalamasını isteyebilir, kimlik doğrulayıcı da önce size
sorar.

Özel anahtarın nerede durduğu anahtarın türüne bağlıdır:

| Anahtar türü | Özel anahtar nerede durur | Diğer cihazlara eşitlenir mi? |
| --- | --- | --- |
| **Bu cihaz** — Face ID, Touch ID, parmak izi, Windows Hello | Platformunuzun parola yöneticisinde (iCloud Anahtar Zinciri, Google Şifre Yöneticisi) ya da 1Password gibi bir parola yöneticisinde | Eşitleme açıksa genellikle evet, uçtan uca şifreli. Windows Hello anahtarları bilgisayarda kalır |
| QR kod okutarak bağlanan **başka bir telefon** | O telefonun parola yöneticisinde | Yukarıdaki gibi |
| **Donanım güvenlik anahtarı** (YubiKey ve diğer FIDO2 anahtarları, USB ya da NFC ile) | Güvenlik anahtarının içinde | Asla |

Bir Vela cüzdanı, cüzdanı oluştururken seçilen, istediğiniz türlerden en fazla yedi
anahtar kullanabilir; bu seçimi [İmzalayıcılar ve güvenlik anahtarları](/tr/docs/signers)
sayfası anlatıyor.

## Oltalanacak bir sır yok

Oltalama, bir sırrı size teslim ettirerek işler. Kurtarma ifadesi, ikna edilip bir
yere yazabileceğiniz on iki kelimedir. Geçiş anahtarının ise **yazabileceğiniz bir
sırrı yoktur**: ifşa edilecek, yapıştırılacak bir şey yoktur ve sahte bir site onu
isteyemez. Üstelik geçiş anahtarı tek bir web sitesi için oluşturulduğundan,
tarayıcınız bir `getvela.app` geçiş anahtarını yalnızca getvela.app ve alt alan
adlarındaki sayfalara sunar.

Bu da kendi saklamada sık görülen bir kayıp türünü — çalınan kurtarma ifadesini —
bütünüyle ortadan kaldırır.

## Geçiş anahtarının sizi korumadığı şeyler

<Callout type="warning" title="Geçiş anahtarı onayladığınız her şeyi imzalar">
Telefonunuzun ya da tarayıcınızın gösterdiği istem, <em>hangi</em> anahtarın
kullanıldığını söyler, <em>neyin</em> imzalandığını değil. Onaylarsanız bir geçiş
anahtarı zararlı bir işlemi de iyi bir işlem kadar kolay imzalar. Bu yüzden Vela,
siz imzalamadan önce her işlemi çözer (<a href="/tr/docs/clear-signing">açık
imzalama</a>); işlemi gösteren sayfanın kendisi de bu yüzden önemlidir
(<a href="/tr/docs/bybit-attack">Bybit saldırısı</a>).
</Callout>

Kilidi açık telefonunuzu ele geçirip onun kontrolünü geçebilen birine ya da geçiş
anahtarınızın eşitlendiği hesabı kontrol eden birine karşı da korumaz. Cihazınızda bir
parola kodu tanımlı tutun, Apple ya da Google hesabınızı güvenceye alın ve hiçbir yere
eşitlenmeyen bir donanım güvenlik anahtarını değerlendirin.

## İmzalamak nasıl bir his

1. Bir işlemin ne yaptığını okuduktan sonra onu Vela'da onaylarsınız.
2. Cihazınız ya da güvenlik anahtarınız Face ID, parmak izi, PIN'iniz ya da dokunma
   artı PIN ister.
3. İmzalar ve uygulamaya yalnızca imza geri döner.
4. Uygulama imzalı işlemi relay'e verir, relay de onu zincire gönderir; cüzdan
   sözleşmeniz herhangi bir şey yapmadan önce geçiş anahtarı imzasını zincir üstünde
   kontrol eder.

## Açık anahtar nereye gider

Anahtarlarınızın **açık** yarıları, yeni bir cihazın cüzdanınızı bulabilmesi için
Gnosis Chain üzerindeki herkese açık bir kayıt defterine yazılır. Bu,
[kurtarma ve giriş](/tr/docs/recovery) sayfasının konusu.

Sırada: [imzalayıcılar ve güvenlik anahtarları](/tr/docs/signers).
