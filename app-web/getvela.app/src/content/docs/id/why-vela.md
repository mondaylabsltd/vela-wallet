---
title: Kenapa kami membuat Vela
description: "Versi panjangnya — dua belas kata itu sebenarnya harus disimpan di mana, apa yang diubah passkey, apa yang tidak bisa kami terima dari dompet yang sudah kami pakai, dan kompromi yang kami pilih sebagai gantinya."
source: 06307425f631
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Kenapa kami membuat Vela

Kami tidak berniat membuat satu dompet lagi. Kami berangkat dari satu pertanyaan yang
tidak pernah bisa kami jawab dengan tuntas:

> Dua belas kata itu sebenarnya harus disimpan di mana?

## Jawaban jujurnya: tangkapan layar

Simpan di aplikasi Catatan, dan satu ponsel yang dicuri sudah cukup untuk mendatangkan
masalah. Tulis di kertas, dan sekarang Anda memikirkan kebakaran, banjir, pindah rumah,
teman sekamar, kantong sampah, dan apakah Anda di masa depan masih ingat di mana "tempat
aman" itu.

Bagi banyak orang, jawaban jujurnya adalah tangkapan layar di galeri foto. Semua orang
tahu itu salah. Tetap saja dilakukan — karena jawaban yang "benar" terlalu berat untuk
dijalani sehari-hari.

Frasa pemulihan adalah rahasia yang harus bertahan melewati puluhan tahun kehidupan
biasa tanpa sekali pun disalin, difoto, diketik di kolom yang salah, atau dibacakan
kepada seseorang yang "membantu" lewat telepon. Bagi orang yang sangat teliti, itu
bukan masalah sulit. Bagi manusia biasa, itu masalah sulit.

## Lalu passkey mengubah rasanya memakai dompet

Kami memakai [Base Account](https://account.base.app) setiap hari, dan menandatangani
dengan Face ID terasa wajar dengan cara yang tidak pernah bisa diberikan frasa
pemulihan — bukan seperti menangani bahan berbahaya, melainkan seperti memakai layanan
internet lainnya.

Tetapi makin sering dipakai, makin sering kami membentur batas yang tidak bisa
diabaikan:

- **kunci pemulihan yang dibuat di dalam browser**, yang hanya bisa Anda percayai begitu
  saja,
- **tidak bisa menambah jaringan sendiri**,
- **tidak bisa kami hosting sendiri**,
- dan masalah diam-diam yang justru paling besar: **kalau layanannya hilang, dompetnya
  ikut hilang.**

Jadi kami membuat versi yang kami sendiri mau andalkan.

## Vela itu sebenarnya apa

Vela adalah **dompet passkey yang bisa sepenuhnya Anda miliki.**

Passkey Anda tetap berada di tempat yang memang sudah dilindungi perangkat Anda —
Rantai Kunci iCloud, Pengelola Sandi Google, atau kunci keamanan fisik yang Anda pegang.
Saat Anda menandatangani transaksi, Vela meminta perangkat Anda menandatanganinya;
perangkat Anda menandatangani dan hanya mengirim balik tanda tangannya. Vela tidak
pernah melihat kuncinya sendiri.

Kebanyakan dompet masih punya satu momen berbahaya, walau singkat: kata-kata di layar,
frasa pemulihan di memori, kunci pemulihan yang tergeletak di tab browser. Vela
dirancang agar momen itu tidak pernah ada.

<Callout type="info" title="Bukan janji — melainkan arsitektur">
Kami tidak bisa mengakses kunci Anda. Bukan "kami berjanji tidak akan" — di Vela memang
tidak ada jalur kode yang bisa melakukannya; WebAuthn tidak mengizinkannya. Dompetnya
adalah <a href="/id/docs/account-contract">akun pintar Safe</a> yang dijalankan oleh
tanda tangan yang dibuat perangkat Anda dan hanya kami terima. Yang memang ditentukan
oleh aplikasi yang Anda pakai untuk menandatangani adalah <em>apa</em> yang diminta
untuk ditandatangani kunci Anda — itulah sebabnya
<a href="/id/docs/whitepaper">model ancaman</a> membahasnya panjang lebar.
</Callout>

Kami membuat Vela **open source** supaya Anda bisa memeriksanya sendiri, dan **bisa
di-hosting sendiri** supaya dompet yang sudah ada tetap berfungsi tanpa server
perusahaan kami — dengan satu batasan, yaitu domain tempat passkey Anda terikat, yang
dijelaskan di [panduan hosting sendiri](/id/docs/self-hosting) beserta cara
menyiasatinya. Dan kami membangunnya di atas
[kontrak Safe](https://github.com/safe-fndn/safe-smart-account/tree/release/v1.4.1)
tanpa modifikasi, karena jalan yang membosankan dan sudah teruji adalah jalan yang
benar ketika menyangkut uang orang — kontrak yang sama yang sudah mengamankan miliaran
dolar on-chain.

## Kompromi yang kami pilih

Kompromi itu tetap ada, dan menyembunyikannya berarti tidak jujur.

Dengan Vela, akun Apple atau Google Anda jadi penting, karena di sanalah passkey yang
tersinkron disimpan. Kehilangan akun itu, atau menghapus passkey-nya, berarti tidak ada
frasa pemulihan, tidak ada reset dari layanan pelanggan, dan tidak ada pintu belakang.

Tetapi setiap dompet non-kustodial meminta Anda memilih risiko mana yang lebih sanggup
Anda tanggung. Frasa pemulihan bisa disalin, di-screenshot, dicuri lewat phishing, atau
diketik di situs yang salah pukul satu dini hari. Passkey berbeda: tidak ada kata-kata
yang bisa bocor, tidak ada rahasia untuk ditempel, dan tidak ada situs palsu yang bisa
menipu Anda untuk menyerahkannya. Browser Anda hanya menawarkannya ke halaman di domain
yang asli.

Dan pilihannya tidak hitam-putih. Sebuah dompet bisa dibuat dengan **hingga tujuh
kunci penanda tangan**, dan salah satunya saja sudah bisa menandatangani — passkey di
beberapa perangkat, ponsel di dekat Anda yang dihubungkan lewat pindai kode QR, atau
kunci keamanan USB/NFC. Kalau Anda lebih suka dompet Anda sama sekali tidak bergantung
pada akun platform, Anda bisa memakai kunci keamanan fisik saja — dua buah, karena
dompet tidak boleh bertumpu pada satu kunci yang tidak tersinkron ke mana pun.
Satu-satunya syarat adalah waktunya: alamat Anda diturunkan dari seluruh kumpulan kunci,
jadi kuncinya dipilih saat Anda membuat dompet.

<Callout type="warning" title="Yang tidak Anda dapatkan dari ini">
Kunci tambahan adalah jalan untuk masuk kembali, bukan gembok kedua. Karena satu kunci
mana pun bisa menandatangani, menambah kunci fisik melindungi Anda dari
<em>kehilangan</em> akses — bukan menghentikan orang yang sudah menguasai salah satu
kunci Anda. Begitulah wujud jujur dari 1-of-n.
</Callout>

## Itulah kenapa Vela ada

Dompet tanpa frasa pemulihan yang harus disembunyikan, tanpa kunci pemulihan yang harus
dipercaya, dan tanpa perusahaan yang harus Anda harapkan bertahan selamanya.

Kalau Anda ingin memeriksa klaimnya alih-alih menerimanya begitu saja:
[whitepaper](/id/docs/whitepaper) memuat arsitekturnya,
[Audit & masalah yang diketahui](/id/docs/security-audits) memuat setiap kontrak yang
kami andalkan beserta mana yang diaudit dan mana yang tidak, dan seluruh kodenya ada
[di GitHub](https://github.com/mondaylabsltd/vela-wallet).

Berikutnya: [memasang Vela](/id/docs/install).
