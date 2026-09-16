---
title: Geçiş anahtarları nasıl çalışır
description: "Vela'nın arkasındaki güvenlik modeli: geçiş anahtarı nedir, anahtarınız nerede durur ve ortada neden oltalanacak bir şey yoktur."
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Geçiş anahtarları nasıl çalışır

Vela'nın bütün güvenlik modeli tek bir fikre dayanıyor: cüzdanınızı kontrol eden
anahtar, cihazınızın oluşturduğu ve işletim sisteminizin tuttuğu bir **geçiş
anahtarı** — Vela dahil hiçbir uygulama onu okuyamaz — ve yalnızca yüzünüzle ya da
parmak izinizle kullanılır.

## Geçiş anahtarı aslında nedir

Geçiş anahtarı, cihazınızın ürettiği bir genel/özel anahtar çifti. **Özel
anahtar** işletim sisteminizin geçiş anahtarı sağlayıcısında durur — Apple'da
genellikle iCloud Anahtar Zinciri, Android'de Google Şifre Yöneticisi — ve uçtan
uca şifreli saklanır; yani hiçbir uygulama onu okuyamaz veya kopyalayamaz.
Uygulamalar anahtarı almaz; kimliğinizi doğruladıktan sonra yalnızca *cihazınızdan
bir şeyi imzalamasını isteyebilirler*.

Bu, Apple Pay'i ve biyometrik kilidinizi koruyan teknolojinin aynısı.

<Callout type="info" title="Can alıcı nokta">
Bir uygulama — Vela dahil — imza isteyebilir, ama özel anahtarınızı asla görmez.
Yüzünüz ya da parmak iziniz cihazınıza imzalama yetkisi verir; anahtarın kendisi
uçtan uca şifreli olarak işletim sisteminizde kalır.
</Callout>

## Ortada neden oltalanacak bir şey yok

Oltalama, bir sırrı size teslim ettirerek işler. Kurtarma ifadesinde o sır, sahte
bir sayfaya yazabileceğiniz on iki kelimedir. Geçiş anahtarında ise
**yazabileceğiniz bir sır yoktur**. Dolandırıcı bir site sizden "geçiş
anahtarınızı girmenizi" isteyemez, çünkü geçiş anahtarı girilebilen bir şey değil
— biyometrinizle kapılanmış bir donanım işlemi.

Bu, insanların kendi sakladıkları parayı kaybetmesinin en yaygın yolunu ortadan
kaldırır.

## Bir işlemi imzalamak nasıl hissettirir

1. Vela'da bir işlemi onaylarsınız.
2. Cihazınız Face ID / Touch ID ister.
3. Cihazınız işlemi geçiş anahtarınızla imzalar.
4. Vela imzalı işlemi ağa yayınlar.

Telefonunuzun kilidini açmakla aynı hareket — çünkü cihazınızın zaten her yerde
kullandığı geçiş anahtarı mekanizmasının aynısı.

<Callout type="warning" title="Cihaz güvenliği yine de önemli">
Geçiş anahtarı, uzaktan saldırılara ve oltalamaya karşı fazlasıyla iyi koruma
sağlar. Cihazınız kilidi açık hâldeyken eline geçiren ve biyometrik kontrolünüzü
geçebilen birine karşı korumaz. Cihazınızda bir parola kodu tanımlı tutun ve
kilidi açık telefonu güvenmediğiniz birine vermeyin.
</Callout>

## Geri kalanı nerede

Geçiş anahtarınızın **genel** anahtarı, cüzdanınızın yeni bir cihazda geri
getirilebilmesi için küçük bir zincir üstü dizine yazılır. Sonraki sayfanın konusu
bu: [kurtarma ve giriş](/tr/docs/recovery).
