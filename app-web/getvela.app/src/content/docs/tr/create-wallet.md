---
title: Cüzdanınızı oluşturun
description: Kurtarma ifadesi olmadan, bir geçiş anahtarıyla yaklaşık bir dakikada kendi saklamanızda duran bir Vela cüzdanı oluşturun. Cüzdanınız her ağda aynı adrese sahip bir Safe akıllı hesabı.
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Cüzdanınızı oluşturun

Cüzdan oluşturmak yaklaşık bir dakika ve tek bir biyometrik onay alıyor. Web
cüzdanını [wallet.getvela.app](https://wallet.getvela.app/) adresinden açın ve
**Cüzdan oluştur**'u seçin.

## Adımlar

1. **Cüzdanınıza ad verin.** Hesabı sonradan ve başka bir cihazda giriş yaparken
   tanıyabilmek için bir ad seçin. Genel anahtarınızın yanında saklanır, yani
   herkese açık kabul edin — içine özel bir şey koymayın.
2. **Temel maddeleri onaylayın.** Kısa bir liste, Vela'nın kendi saklamanızda
   durduğunu ve hâlâ alfa yazılım olduğunu anladığınızı teyit eder;
   [gizlilik politikası](/privacy) ve [koşullar](/terms) bağlantılarıyla birlikte.
3. **Geçiş anahtarınızı oluşturun.** İstendiğinde **Face ID, Touch ID ya da
   parmak izinizle** kimliğinizi doğrulayın. Bu, cihazınızın tuttuğu ve Vela'nın
   hiçbir zaman görmediği bir WebAuthn (P-256) geçiş anahtarı üretir. Kurtarma
   ifadesi adımı yok, çünkü kurtarma ifadesi yok.
4. **Bitti.** Vela cüzdan adresinizi gösterir ve içeridesiniz. Adresi doğrulayıp
   giriş yaparak cüzdanınıza geçebilirsiniz.

## Cüzdanınız aslında ne

Çoğu cüzdanın anlatmadığı kısım burası — ve Vela'nın nasıl çalıştığı açısından
önemli.

Vela cüzdanınız düz bir "harici sahipli hesap" değil, bir **Safe akıllı hesabı**
(bir akıllı sözleşme). Geçiş anahtarınız hesabın sahibi; ERC-4337 kurulumu da
hesabı yalnızca yüzünüzle ya da parmak izinizle kullanmanızı sağlıyor.

<Callout type="info" title="Adresiniz her ağda aynı">
Vela adresinizi geçiş anahtarınızın genel anahtarından türetir; bu yüzden adres
Ethereum, Base, Arbitrum, Gnosis ve desteklenen diğer bütün ağlarda birebir aynı.
Her yerde tek bir adres verebilirsiniz.
</Callout>

İşe yarar bir sonuç: adres **karşıolgusal**. Zincirde hiçbir şey kurulmadan önce
hesaplanır, yani **cüzdan sözleşmeniz var olmadan önce o adrese para
alabilirsiniz**. Sözleşme, bir ağda ilk işleminizi gönderdiğinizde kendi
bakiyesinden ödeyerek kendini kurar.

## Anahtarlarınıza az önce ne oldu

- Cihazınız bir **geçiş anahtarı çifti** üretti.
- **Özel anahtar**, işletim sisteminizin geçiş anahtarı sağlayıcısında (iCloud
  Anahtar Zinciri ya da Google Şifre Yöneticisi) uçtan uca şifreli duruyor ve
  cihazlarınız arasında eşitleniyor — Vela dahil hiçbir uygulama onu görmüyor.
- **Genel anahtar ve seçtiğiniz ad**, Vela'nın Geçiş Anahtarı Dizini'ne
  gönderiliyor; dizin anahtarı ayrıca Gnosis Chain üzerinde herkesin okuyabildiği
  bir kayda yazıyor, böylece hesabınız yeni bir cihazda yeniden bulunabiliyor.
  Bkz. [kurtarma ve giriş](/tr/docs/recovery).

## Sırada

- [İlk tokenlarınızı alın](/tr/docs/send-and-receive)
- [Ağları ve ücretleri anlayın](/tr/docs/networks-and-fees)
- [Geçiş anahtarları bunu nasıl güvende tutuyor, okuyun](/tr/docs/passkeys)
