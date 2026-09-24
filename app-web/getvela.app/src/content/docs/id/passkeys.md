---
title: Cara kerja passkey
description: "Apa itu passkey, di mana kunci privatnya disimpan untuk tiap jenis kunci, kenapa tidak ada rahasia yang bisa dicuri lewat phishing, dan apa yang tidak dilindungi passkey."
source: b23999b2ed69
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Cara kerja passkey

Kunci yang mengendalikan dompet Vela adalah **passkey**: kredensial WebAuthn pada kurva
P-256. Perangkat atau kunci keamanan Anda membuat tiap passkey, menyimpan kunci
privatnya, dan hanya memakainya setelah Anda mengonfirmasi dengan Face ID, sidik jari,
PIN perangkat, atau sentuhan dan PIN di kunci keamanan. Vela tidak pernah menerima
kunci privatnya; kalau pengelola kata sandi menyinkronkannya, pengelola itulah yang
menyimpannya, terenkripsi, atas nama Anda.

## Apa itu passkey

Passkey adalah pasangan kunci publik/privat yang dibuat untuk satu situs web — untuk
Vela, `getvela.app`. Aplikasi tidak pernah menerima kunci privatnya; aplikasi hanya bisa
meminta autentikator menandatangani sesuatu, dan autentikator meminta persetujuan Anda
lebih dulu.

Di mana kunci privatnya disimpan bergantung pada jenis kuncinya:

| Jenis kunci | Tempat kunci privat disimpan | Tersinkron ke perangkat lain? |
| --- | --- | --- |
| **Perangkat ini** — Face ID, Touch ID, sidik jari, Windows Hello | Pengelola kata sandi platform Anda (Rantai Kunci iCloud, Pengelola Sandi Google) atau pengelola kata sandi seperti 1Password | Biasanya, dengan enkripsi end-to-end, kalau sinkronisasi aktif. Kunci Windows Hello tetap di PC itu |
| **Ponsel lain**, dihubungkan dengan memindai kode QR | Pengelola kata sandi di ponsel itu | Sama seperti di atas |
| **Kunci keamanan fisik** (YubiKey dan kunci FIDO2 lainnya, lewat USB atau NFC) | Di dalam kunci keamanan itu | Tidak pernah |

Dompet Vela bisa memakai hingga tujuh kunci dengan kombinasi apa pun, yang dipilih saat
Anda membuatnya; [kunci penanda tangan & kunci keamanan](/id/docs/signers) membahas
pilihan itu.

## Tidak ada rahasia yang bisa dicuri lewat phishing

Phishing bekerja dengan membuat Anda menyerahkan sebuah rahasia. Frasa pemulihan adalah
dua belas kata yang bisa saja Anda ketik di suatu tempat karena dibujuk. Passkey **tidak
punya rahasia yang bisa diketik**: tidak ada yang bisa dibocorkan, tidak ada yang bisa
ditempel, dan situs palsu tidak bisa memintanya. Selain itu, karena passkey dibuat untuk
satu situs web, browser Anda hanya menawarkan passkey `getvela.app` ke halaman di
getvela.app dan subdomainnya.

Itu menghilangkan satu jenis kerugian yang umum terjadi di dompet non-kustodial:
frasa pemulihan yang dicuri.

## Apa yang tidak dilindungi passkey

<Callout type="warning" title="Passkey menandatangani apa pun yang Anda setujui">
Permintaan konfirmasi dari ponsel atau browser Anda memberi tahu <em>kunci mana</em>
yang dipakai, bukan <em>apa</em> yang ditandatangani. Passkey akan menandatangani
transaksi berbahaya semudah transaksi yang baik kalau Anda menyetujuinya. Itulah
sebabnya Vela mendekode setiap transaksi sebelum Anda menandatangani
(<a href="/id/docs/clear-signing">clear signing</a>), dan kenapa halaman yang
menampilkannya itu penting (<a href="/id/docs/bybit-attack">serangan Bybit</a>).
</Callout>

Passkey juga tidak melindungi dari orang yang memegang ponsel Anda dalam keadaan tidak
terkunci dan bisa lolos pemeriksaannya, atau yang menguasai akun tempat passkey Anda
tersinkron. Pasang kode sandi perangkat, amankan akun Apple atau Google Anda, dan
pertimbangkan kunci keamanan fisik yang tidak tersinkron ke mana pun.

## Seperti apa rasanya menandatangani

1. Anda mengonfirmasi transaksi di Vela, setelah membaca apa yang dilakukannya.
2. Perangkat atau kunci keamanan Anda meminta Face ID, sidik jari, PIN Anda, atau
   sentuhan plus PIN.
3. Perangkat itu menandatangani, dan hanya tanda tangannya yang kembali ke aplikasi.
4. Aplikasi menyerahkan operasi yang sudah ditandatangani ke relay, yang mengirimkannya;
   kontrak dompet Anda memeriksa tanda tangan passkey on-chain sebelum melakukan apa pun.

## Ke mana kunci publiknya pergi

Bagian **publik** dari kunci-kunci Anda dicatat di registri publik di Gnosis Chain,
supaya perangkat baru bisa menemukan dompet Anda. Itulah pokok bahasan
[pemulihan & masuk](/id/docs/recovery).

Berikutnya: [kunci penanda tangan & kunci keamanan](/id/docs/signers).
