---
title: Clear signing
description: Vela menerjemahkan transaksi ke bahasa yang jelas sebelum Anda menyetujuinya — maksud, jumlah, alamat, dan risiko — alih-alih hex yang tak terbaca. Kalau tidak bisa menerjemahkan, ia memperingatkan Anda, bukan berpura-pura.
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Clear signing

Sebagian besar dompet meminta Anda menyetujui dinding heksadesimal lalu berharap yang
terbaik. "Tanda tangan buta" — menyetujui panggilan yang sebenarnya tidak bisa Anda
baca — ada di balik sebagian besar dompet yang terkuras. Jawaban Vela adalah **clear
signing**: sebelum Anda menandatangani, transaksinya diterjemahkan menjadi sesuatu
yang bisa Anda pahami.

## Yang Anda lihat

Alih-alih calldata mentah, Vela menampilkan:

- **Maksud** — apa yang dilakukan transaksi itu: *Kirim*, *Setujui*, *Tukar*, dan
  seterusnya.
- **Intinya** — jumlah dan alamat yang terlibat, dengan jumlah token dalam satuan
  sebenarnya dan penerima yang diterjemahkan menjadi nama bila ada.
- **Detailnya** — nonce, tenggat, dan calldata mentah, tersedia saat diminta alih-alih
  disodorkan ke muka Anda.
- **Indikasi risiko**, dengan kode warna, supaya yang menakutkan terlihat menakutkan.

## Cara kerjanya (ERC-7730)

Vela menerjemahkan **panggilan kontrak** maupun **data bertipe EIP-712** memakai
deskriptor
[ERC-7730](https://github.com/LedgerHQ/clear-signing-erc7730-registry) — definisi
kecil yang bisa dibagikan tentang arti fungsi-fungsi sebuah kontrak.

- Kalau ada **deskriptor khusus kontrak**, transaksinya ditandai **terverifikasi** dan
  diberi label dengan nama kontraknya.
- Kalau tidak ada, Vela turun ke **deskriptor standar** untuk bentuk-bentuk umum —
  token ERC-20, NFT ERC-721, vault ERC-4626, dan permit ERC-2612 — sehingga sebagian
  besar aktivitas sehari-hari tetap terbaca.

Jumlah token diformat memakai **desimal on-chain sebenarnya** milik token itu. Vela
tidak pernah begitu saja mengasumsikan 18; kalau desimalnya tidak bisa dipastikan, ia
tetap menampilkan nilainya tetapi **menandainya belum terverifikasi** alih-alih
menebak.

## Tingkat risiko

Setiap transaksi yang berhasil diterjemahkan mendapat tingkat risiko agar pola
berbahaya menonjol:

- **Hati-hati** untuk persetujuan dan permit — Anda sedang memberi daya belanja.
- **Bahaya** untuk yang benar-benar berisiko, seperti **persetujuan token tanpa
  batas**.
- Risiko lebih rendah untuk tindakan rutin seperti staking atau menyetor.

<Callout type="warning" title="Persetujuan tanpa batas diblokir">
Sebuah "approve" yang memberi jatah tanpa batas adalah salah satu cara paling umum
dana terkuras belakangan. Vela tidak sekadar menandainya: ia menulis ulang
permintaannya menjadi jumlah terbatas pilihan Anda, dan pemeriksaan terakhir sebelum
pengiriman menolak mengirim persetujuan apa pun yang masih tanpa batas. Penjaga itu
membaca calldata mentah secara langsung, jadi ia bekerja bahkan saat tidak ada
deskriptor untuk kontrak tersebut.
</Callout>

## Saat Vela tidak bisa menerjemahkan sebuah panggilan

Kejujuran lebih penting daripada layar yang rapi. Kalau tidak ada deskriptor ERC-7730
tetapi fungsinya muncul di basis data selector publik, Vela menerjemahkan panggilan itu
secara umum dan melabelinya **upaya terbaik** — terbaca, tetapi belum terverifikasi —
di bawah spanduk peringatan. Kalau itu pun gagal, atau Vela hanya bisa menerjemahkan
sebagian transaksi, ia **tidak** berpura-pura memahaminya.

<Callout type="danger" title="Peringatan tanda tangan buta yang tegas">
Kalau sebuah panggilan tidak bisa diterjemahkan, Vela menampilkan peringatan tanda
tangan buta yang jelas alih-alih ringkasan ramah yang palsu. Kalau hanya sebagian
kolom yang bisa dipecahkan, ia memberi tahu Anda bahwa tampilannya belum lengkap dan
menahan tingkat risikonya tetap tinggi. Anda selalu tahu seberapa banyak dari yang
Anda tandatangani yang benar-benar bisa dibaca Vela.
</Callout>

## Kenapa ini penting

Swakelola berarti tidak ada yang bisa membatalkan transaksi buruk untuk Anda.
Pertahanannya bukan meja bantuan — melainkan memahami apa yang Anda setujui
**sebelum** Anda menyetujuinya. Clear signing mengubah "percayalah pada gumpalan tak
terbaca ini" menjadi "inilah persisnya apa yang dilakukannya". Lihat
[whitepaper](/id/docs/whitepaper) untuk tahu di mana posisinya dalam model keamanan
Vela secara keseluruhan.
