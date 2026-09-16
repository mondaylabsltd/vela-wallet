---
title: Cara kerja passkey
description: "Model keamanan di balik Vela: apa itu passkey, di mana kunci Anda berada, dan kenapa tidak ada yang bisa dicuri lewat phishing."
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Cara kerja passkey

Seluruh model keamanan Vela bertumpu pada satu gagasan: kunci yang mengendalikan
dompet Anda adalah **passkey**, dibuat oleh perangkat Anda dan dipegang sistem operasi
— tidak ada aplikasi, termasuk Vela, yang bisa membacanya — dan hanya dipakai dengan
wajah atau sidik jari Anda.

## Passkey itu sebenarnya apa

Passkey adalah sepasang kunci publik/privat yang dibuat perangkat Anda. **Kunci
privatnya** dipegang penyedia passkey sistem operasi Anda — biasanya iCloud Keychain
di Apple, Google Password Manager di Android — disimpan terenkripsi ujung-ke-ujung,
jadi tidak ada aplikasi yang bisa membaca atau menyalinnya. Aplikasi tidak menerima
kuncinya; setelah Anda terverifikasi, mereka hanya boleh *meminta perangkat Anda
menandatangani sesuatu*.

Ini teknologi yang sama dengan yang melindungi Apple Pay dan buka kunci biometrik
Anda.

<Callout type="info" title="Poin pentingnya">
Sebuah aplikasi — termasuk Vela — bisa meminta tanda tangan, tetapi tidak pernah
melihat kunci privat Anda. Wajah atau sidik jari Anda memberi izin kepada perangkat
untuk menandatangani; kuncinya sendiri tetap di sistem operasi, terenkripsi
ujung-ke-ujung.
</Callout>

## Kenapa tidak ada yang bisa di-phishing

Phishing bekerja dengan membuat Anda menyerahkan sebuah rahasia. Pada frasa pemulihan,
rahasia itu adalah dua belas kata yang bisa Anda ketikkan ke halaman palsu. Pada
passkey, **tidak ada rahasia yang bisa diketik**. Situs penipu tidak bisa meminta Anda
"memasukkan passkey Anda", karena passkey bukan sesuatu yang bisa dimasukkan — ia
operasi perangkat keras yang dikunci oleh biometrik Anda.

Itu menghapus satu cara paling umum orang kehilangan dana yang mereka kelola sendiri.

## Rasanya menandatangani transaksi

1. Anda mengonfirmasi transaksi di Vela.
2. Perangkat Anda meminta Face ID / Touch ID.
3. Perangkat Anda menandatangani transaksi dengan passkey Anda.
4. Vela menyiarkan transaksi yang sudah ditandatangani ke jaringan.

Gerakan yang sama dengan membuka kunci ponsel — karena memang mekanisme passkey yang
sama, yang sudah dipakai perangkat Anda di tempat lain.

<Callout type="warning" title="Keamanan perangkat tetap penting">
Passkey sangat baik melindungi dari serangan jarak jauh dan phishing. Ia tidak
melindungi dari orang yang memegang perangkat Anda dalam keadaan tidak terkunci dan
bisa melewati pemeriksaan biometrik Anda. Pasang kode sandi perangkat dan jangan
menyerahkan ponsel yang tidak terkunci kepada orang yang tidak Anda percaya.
</Callout>

## Sisanya ada di mana

Kunci **publik** passkey Anda ditulis ke indeks kecil di rantai agar dompet Anda bisa
dipulihkan di perangkat baru. Itu topik halaman berikutnya:
[pemulihan & masuk](/id/docs/recovery).
