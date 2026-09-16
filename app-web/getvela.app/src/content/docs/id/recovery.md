---
title: Pemulihan & masuk
description: Bagaimana Vela memungkinkan Anda memulihkan dompet di perangkat baru tanpa frasa pemulihan — dan batas-batas jujur dari model itu.
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Pemulihan & masuk

Bagian tersulit dari dompet tanpa frasa pemulihan justru pemulihannya: kalau tidak ada
dua belas kata, bagaimana Anda masuk lagi di ponsel baru? Begini persisnya cara Vela
menanganinya.

## Cara kerjanya

Saat Anda membuat dompet, dua hal dikirim ke **indeks passkey** milik Vela:

- **Kunci publik** passkey Anda (tidak pernah kunci privatnya).
- **Nama** yang Anda pilih untuk dompet itu.

Kunci publiknya disimpan di blockchain Gnosis lewat sebuah kontrak pintar, jadi bisa
dibaca siapa pun dan tidak bergantung pada server Vela tetap hidup.

Sementara itu, kunci **privat** Anda adalah passkey yang disinkronkan oleh keychain
platform Anda — **iCloud Keychain** di perangkat Apple, **Google Password Manager** di
Android.

Untuk masuk di perangkat baru:

1. Masuk ke akun iCloud atau Google yang sama, dengan sinkronisasi keychain menyala.
2. Buka Vela dan pilih untuk masuk.
3. Verifikasi dengan passkey Anda. Platform menyediakan passkey yang tersinkron;
   indeks menyediakan akun yang cocok. Dompet Anda kembali.

Indeks itu cache, bukan titik kegagalan tunggal. Kalau suatu saat ia tidak terjangkau
dan akun Anda tidak ada di penyimpanan lokal, Vela bisa menyusun ulang kunci publik
Anda di perangkat dari dua tanda tangan passkey, lalu menurunkan kembali alamat dompet
Anda darinya — tanpa server mana pun.

<Callout type="info" title="Kenapa dipisah begini">
Kunci publik di indeks on-chain memungkinkan siapa pun (termasuk instalasi yang baru
sama sekali) menemukan akun Anda. Kunci privat, yang disinkronkan keychain platform
yang Anda percayai, adalah yang benar-benar memberi izin transaksi. Semua isi indeks
adalah data publik; tidak ada isinya yang bisa memindahkan dana Anda — hanya tanda
tangan dari passkey Anda yang bisa.
</Callout>

## Batas-batas jujurnya

Swakelola berarti tanggung jawabnya nyata. Ini yang perlu dipahami.

<Callout type="warning" title="Pemulihan Anda bergantung pada keychain platform">
Masuk lintas perangkat di Vela bersandar pada passkey Anda yang tersinkron lewat
iCloud Keychain atau Google Password Manager. Jaga akun itu tetap aman dan opsi
pemulihannya tetap mutakhir. Kalau Anda kehilangan akses ke <strong>keduanya</strong>
— perangkat Anda <strong>dan</strong> keychain akun platform Anda — Vela tidak bisa
membuat ulang kunci privat Anda; secara desain, kami memang tidak pernah memilikinya.
</Callout>

Saran praktis:

- **Biarkan sinkronisasi keychain menyala.** Itulah yang membawa passkey Anda antar
  perangkat.
- **Amankan akun Apple / Google Anda** dengan kata sandi kuat dan metode pemulihannya
  sendiri. Akun itu kini bagian dari keamanan dompet Anda.
- **Masuklah di lebih dari satu perangkat** bila memungkinkan, agar satu ponsel yang
  hilang jadi kerepotan, bukan krisis.

## Yang bisa dan tidak bisa dilakukan Vela

- **Bisa:** membantu Anda menemukan akun Anda lagi lewat indeks publik.
- **Tidak bisa:** memindahkan dana Anda, membekukan dompet Anda, atau memulihkan kunci
  privat. Vela tidak pernah memegangnya. Itulah inti swakelola — dan itulah pertukaran
  yang Anda ambil untuk mendapatkannya.
