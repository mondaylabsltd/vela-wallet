---
title: Kenapa kami membuat Vela
description: Versi panjangnya — dua belas kata itu sebenarnya harus disimpan di mana, apa yang diubah passkey, apa yang tidak bisa kami terima pada dompet yang sudah kami pakai, dan pertukaran apa yang kami pilih sebagai gantinya.
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Kenapa kami membuat Vela

Kami tidak berniat membuat satu dompet lagi. Kami mulai dari pertanyaan yang tidak
pernah bisa kami jawab dengan rapi:

> Dua belas kata itu sebenarnya harus disimpan di mana?

## Jawaban jujurnya adalah tangkapan layar

Taruh di Catatan, dan Anda hanya berjarak satu ponsel hilang dari masalah. Tulis di
kertas, dan sekarang Anda memikirkan kebakaran, air, pindah rumah, teman sekamar,
kantong sampah, dan apakah Anda-di-masa-depan masih ingat di mana "tempat aman" itu.

Bagi banyak orang, jawaban jujurnya adalah tangkapan layar di galeri foto. Semua orang
tahu itu salah. Mereka tetap melakukannya — karena jawaban yang "benar" terlalu berat
untuk dijalani.

Frasa pemulihan adalah rahasia yang harus bertahan melewati puluhan tahun kehidupan
biasa tanpa pernah sekali pun disalin, difoto, diketik ke kotak yang salah, atau
dibacakan kepada seseorang yang "membantu" lewat telepon. Bagi orang yang sangat
berhati-hati itu bukan masalah sulit. Bagi seorang manusia, itu masalah sulit.

## Lalu passkey mengubah rasanya memakai dompet

Kami memakai [Base Account](https://account.base.app) setiap hari, dan menandatangani
dengan Face ID terasa masuk akal dengan cara yang tidak pernah dicapai frasa pemulihan
— kurang seperti memegang bahan berbahaya, lebih seperti memakai internet pada
umumnya.

Tetapi makin sering dipakai, makin sering kami menabrak batas yang tidak bisa
diabaikan:

- **kunci pemulihan yang dibuat di dalam browser** dan hanya bisa Anda percayai begitu
  saja,
- **tidak bisa menambah jaringan sendiri**,
- **tidak ada bagian yang bisa kami hosting sendiri**,
- dan masalah senyap yang paling besar: **kalau layanannya berhenti, dompetnya ikut
  berhenti.**

Jadi kami membuat versi yang kami sendiri mau bergantung padanya.

## Vela itu sebenarnya apa

Vela adalah **dompet passkey yang bisa sepenuhnya Anda miliki.**

Passkey Anda tetap berada di tempat perangkat Anda sudah melindunginya — iCloud
Keychain, Google Password Manager, atau kunci keamanan fisik yang Anda pegang. Saat
Anda menandatangani transaksi, Vela mengirim tantangan ke perangkat Anda; perangkat
Anda menandatanganinya dan hanya mengirim balik tanda tangannya. Vela tidak pernah
melihat kuncinya sendiri.

Kebanyakan dompet masih punya satu momen berbahaya, meski sebentar: kata-kata di layar,
frasa pemulihan di memori, kunci pemulihan yang nongkrong di tab browser. Vela dirancang
agar momen itu tidak pernah ada.

<Callout type="info" title="Bukan janji — melainkan arsitektur">
Kami tidak bisa mengakses kunci Anda. Bukan "kami berjanji tidak akan" — memang tidak
ada jalur kode di Vela yang bisa. Dompetnya adalah
<a href="/id/docs/security-audits">akun pintar Safe</a> yang dijalankan oleh tanda
tangan yang dibuat perangkat Anda dan hanya kami terima.
</Callout>

Kami membuat Vela **bersumber terbuka** supaya Anda bisa memeriksanya sendiri, dan
**bisa di-hosting sendiri** supaya dompet Anda tidak pernah bergantung pada perusahaan
kami tetap online. Dan kami membangunnya di atas
[kontrak Safe](https://github.com/safe-fndn/safe-smart-account/tree/release/v1.4.1)
tanpa modifikasi, karena jalur yang membosankan dan sudah teruji adalah jalur yang
benar ketika menyangkut uang orang — kontrak yang sama yang sudah mengamankan miliaran
di rantai.

## Pertukaran yang kami pilih

Tetap ada pertukarannya, dan menguburnya tidak akan jujur.

Dengan Vela, akun Apple atau Google Anda jadi penting, karena di situlah passkey yang
tersinkron berada. Kehilangan akun itu, atau menghapus passkey-nya, berarti tidak ada
frasa pemulihan, tidak ada reset dari dukungan, tidak ada pintu belakang.

Tetapi setiap dompet swakelola meminta Anda memilih risiko mana yang lebih siap Anda
tanggung. Frasa pemulihan bisa disalin, ditangkap layar, di-phishing, atau diketik ke
situs yang salah pada pukul satu pagi. Passkey berbeda: tidak ada kata yang bisa
dibocorkan, tidak ada rahasia yang bisa ditempel, dan tidak ada situs palsu yang bisa
mengelabui Anda untuk menyerahkannya. Perangkat Anda menandatangani untuk domain yang
asli, atau ia tidak menandatangani sama sekali.

Dan pilihannya bukan biner. Sebuah dompet bisa dibuat dengan **sampai tujuh kunci
penanda tangan**, salah satunya saja sudah bisa menandatangani — passkey di beberapa
perangkat, ponsel di dekat Anda yang Anda pindai, atau kunci keamanan USB/NFC. Kalau
Anda lebih suka dompet Anda sama sekali tidak bergantung pada akun platform, Anda bisa
menjadikan kunci pertama sebagai kunci keamanan fisik. Satu-satunya syarat adalah
waktunya: alamat Anda diturunkan dari keseluruhan kumpulan kunci, jadi kuncinya dipilih
saat Anda membuat dompet.

<Callout type="warning" title="Yang tidak Anda dapatkan dari ini">
Kunci tambahan adalah jalan masuk kembali, bukan gembok kedua. Karena satu kunci mana
pun bisa menandatangani, menambah kunci perangkat keras melindungi Anda dari
<em>kehilangan</em> akses — ia tidak menghentikan orang yang sudah menguasai salah satu
kunci Anda. Begitulah bentuk jujur dari 1-of-n.
</Callout>

## Itulah kenapa Vela ada

Dompet tanpa frasa pemulihan yang harus disembunyikan, tanpa kunci pemulihan yang harus
dipercaya, dan tanpa perusahaan yang harus Anda harapkan bertahan selamanya.

Kalau Anda ingin memeriksa klaimnya alih-alih menelannya:
[whitepaper](/id/docs/whitepaper) memuat arsitekturnya,
[Audit & masalah yang diketahui](/id/docs/security-audits) memuat setiap kontrak yang
kami andalkan beserta apa yang sudah dan belum diaudit, dan seluruh kodenya ada
[di GitHub](https://github.com/mondaylabsltd/vela-wallet).

Berikutnya: [memasang Vela](/id/docs/install).
