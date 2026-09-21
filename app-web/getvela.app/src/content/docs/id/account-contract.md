---
title: Kontrak akun
description: "Dompet Vela Anda adalah Safe v1.4.1 yang tidak dimodifikasi. Tidak ada satu pun kontrak di jalur menuju uang Anda yang ditulis Vela — inilah persisnya kontrak-kontrak itu, apa yang Anda dapatkan darinya, dan apa harganya."
source: 588a6ba6e672
---

# Kontrak akun

Dompet Anda bukan struktur data pribadi milik sebuah aplikasi. Dompet Anda adalah akun
pintar **Safe v1.4.1** — kontrak yang dipakai banyak treasury on-chain berskala besar —
yang di-deploy persis seperti yang dirilis Safe, tanpa modifikasi.

## Tidak ada kontrak kami di jalurnya

Setiap kontrak yang bisa menyentuh uang Anda ditulis oleh Safe atau oleh para penyusun
ERC-4337:

| Kontrak | Peran di dompet Anda | Ditulis oleh |
| --- | --- | --- |
| [Safe v1.4.1](https://github.com/safe-fndn/safe-smart-account/tree/v1.4.1) (SafeL2, lewat proxy) | Akun itu sendiri; pemilik, ambang batas, eksekusi | Safe |
| [Safe 4337 Module v0.3.0](https://github.com/safe-global/safe-modules/tree/4337/v0.3.0/modules/4337) | Memungkinkan EntryPoint menjalankan Safe; juga menjadi fallback handler-nya | Safe |
| [SafeWebAuthnSharedSigner v0.2.1](https://github.com/safe-global/safe-modules/tree/passkey/v0.2.1/modules/passkey) | Memverifikasi tanda tangan P-256 kunci pertama | Safe |
| [SafeWebAuthnSignerFactory v0.2.1](https://github.com/safe-global/safe-modules/tree/passkey/v0.2.1/modules/passkey) dan signer yang dibuatnya | Satu kontrak signer kecil untuk tiap kunci tambahan | Safe |
| [ERC-4337 EntryPoint v0.7](https://github.com/eth-infinitism/account-abstraction/releases/tag/v0.7.0) | Menjalankan operasi yang Anda tandatangani | Penyusun ERC-4337 |

Kontrak milik Vela sendiri tidak ada di daftar ini: **registri kunci publik** yang
mencatat kunci setiap dompet agar perangkat baru bisa menemukannya
([pemulihan](/id/docs/recovery)), registri domain pendampingnya, dan indeks lama yang
digantikan keduanya. Kontrak-kontrak itu tidak menyimpan dana dan tidak punya peran di
Safe Anda.

Repositori dompet sama sekali tidak berisi Solidity — Anda bisa memeriksanya dengan satu
perintah:

```bash
git clone https://github.com/mondaylabsltd/vela-wallet
find vela-wallet -name '*.sol'   # tidak mencetak apa pun
```

Vela tidak memegang peran istimewa apa pun di akun Anda: tidak ada kunci admin, tidak
ada jalur upgrade, dan tidak ada modul yang bisa ditambahkannya. Hanya kunci Anda yang
bisa mengubah Safe Anda.

## Kenapa kata "tidak dimodifikasi" yang penting

Banyak dompet dibangun di atas "sebuah Safe", fork dari Safe, atau akun yang terinspirasi
Safe. Perbedaannya penting dalam tiga hal.

**Auditnya berlaku untuk apa yang benar-benar Anda pakai.** Audit Safe mencakup rilis
ini, atau rilis sebelumnya yang hanya berbeda lewat perubahan kecil yang terdokumentasi
([halaman audit](/id/docs/security-audits) memuat detailnya). Audit sebuah fork mencakup
kode sebelum di-fork; modifikasinya justru bagian yang tidak diaudit siapa pun.

**Ekosistem memperlakukan akun Anda sebagai Safe, karena memang Safe.** Block explorer
bisa mendekodenya, dan alat-alat Safe bisa membacanya serta menyusun transaksi untuknya.
Namun untuk *menandatangani* transaksi itu, sebuah program harus bisa meminta kunci Anda
membuat tanda tangan untuk `getvela.app`, domain tempat passkey Anda terikat — jadi
aplikasi web resmi Safe, yang disajikan dari domain lain, tidak bisa menandatangani untuk
Anda. [Panduan hosting sendiri](/id/docs/self-hosting#if-getvela-app-disappears)
mencantumkan apa saja yang bisa.

**Permukaan serangannya juga diawasi banyak pihak lain.** Kontrak akun buatan sendiri
kebanyakan hanya diawasi pembuatnya. Kontrak inti Safe diawasi semua orang yang
menyimpan uang di Safe; modul 4337 dan modul passkey punya pengamat yang lebih sedikit,
tetapi nyata.

## Harganya

Memakai standar tidak gratis:

- **Gas.** Tanda tangan Anda diverifikasi on-chain dan transaksinya berjalan lewat
  EntryPoint. Pengiriman sederhana dari dompet Vela yang sudah di-deploy memakai sekitar
  140.000–170.000 gas on-chain dalam pengukuran kami di Gnosis (September 2026); transfer
  ETH biasa dari akun biasa memakai 21.000. Di luar gas, relay menagih biayanya — lihat
  [jaringan & biaya](/id/docs/networks-and-fees).
- **Akunnya harus di-deploy.** Alamat Anda dihitung dengan `CREATE2` sebelum apa pun ada
  on-chain, jadi Anda bisa langsung menerima dana di sana; transaksi keluar pertama Anda
  di tiap jaringan membayar deploy kontraknya.
- **Tidak semua chain memenuhi syarat.** Tanda tangan passkey diverifikasi dengan
  precompile **RIP-7212**, dan alamat precompile itu adalah bagian dari data penyiapan
  setiap dompet, jadi jaringan tanpa precompile itu sama sekali tidak bisa menjalankan
  Vela.
- **Risiko Safe kini juga risiko Anda.** Memercayai kontrak yang dipakai luas tetaplah
  memercayai sebuah kontrak. Vela tidak menambahkan kontrak kedua buatannya sendiri di
  jalur menuju uang Anda yang juga harus Anda percayai.

## Apa yang diaudit dan apa yang tidak

Kontrak Safe, modul 4337 dan modul passkey-nya, serta EntryPoint v0.7 punya laporan audit
pihak ketiga yang dipublikasikan. **Kode Vela sendiri — aplikasi, layanan backend, dan
kontrak registri — belum pernah diaudit pihak ketiga, dan tidak ada audit yang
dijadwalkan**; audit adalah sasaran untuk saat proyek ini mampu membiayainya, bukan
komitmen dengan tanggal. Setiap kontrak, laporan auditnya, dan masalah yang kami pantau
ada di [audit & masalah yang diketahui](/id/docs/security-audits).

## Lihat sendiri

Akun Anda ada on-chain. Buka alamat Anda di block explorer setelah akun itu di-deploy:
alamat itu adalah proxy Safe yang implementasinya adalah deployment kanonis SafeL2 v1.4.1
milik Safe, di setiap jaringan.

Berikutnya: [audit & masalah yang diketahui](/id/docs/security-audits).
