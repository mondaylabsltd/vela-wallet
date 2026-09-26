---
title: Clear signing
description: "Vela mendekode transaksi menjadi bahasa yang mudah dipahami sebelum Anda menyetujuinya — maksud, jumlah, alamat, dan risiko — alih-alih hex yang tak terbaca. Kalau tidak bisa mendekode sebuah panggilan, Vela memperingatkan Anda, bukan berpura-pura paham."
source: 29a66a1584f1
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Clear signing

Banyak dompet masih menampilkan data mentah untuk kontrak apa pun yang tidak mereka
kenali, dan "tanda tangan buta" (blind signing) — menyetujui panggilan yang sebenarnya
tidak bisa Anda baca — adalah salah satu cara dompet dikuras. Jawaban Vela adalah
**clear signing**: sebelum Anda menandatangani, transaksinya didekode menjadi sesuatu
yang bisa Anda pahami, sejauh mungkin.

## Apa yang Anda lihat

Alih-alih calldata mentah, Vela menampilkan:

- **Maksud** — apa yang dilakukan transaksi itu: *Kirim*, *Setujui*, *Tukar*, dan
  seterusnya.
- **Intinya** — jumlah dan alamat yang terlibat, dengan jumlah token ditampilkan dalam
  satuan sebenarnya dan penerima diubah menjadi nama bila namanya ada.
- **Detailnya** — nonce, tenggat, dan calldata mentah, tersedia saat Anda butuhkan
  alih-alih disodorkan ke muka Anda.
- **Indikasi risiko**, dengan kode warna supaya tindakan berbahaya langsung menonjol.

## Cara kerjanya (ERC-7730)

Vela mendekode **panggilan kontrak** maupun **typed data EIP-712** memakai deskriptor
[ERC-7730](https://github.com/LedgerHQ/clear-signing-erc7730-registry) — definisi kecil
yang bisa dibagikan tentang arti fungsi-fungsi sebuah kontrak.

Vela mencari deskriptor dengan urutan ini:

1. **Bawaan aplikasi** — deskriptor untuk kontrak yang banyak dipakai: router Uniswap,
   PancakeSwap, dan SushiSwap, WETH, pool Aave v3, 1inch, Lido dan wstETH, serta
   Seaport.
2. **Diambil dari server data chain milik Vela**, yang menerbitkan ulang registri
   ERC-7730 publik.
3. **Bentuk standar** — token ERC-20, NFT ERC-721 dan ERC-1155, vault ERC-4626, dan
   permit ERC-2612 — jadi sebagian besar tindakan sehari-hari tetap bisa didekode.

**Terverifikasi** hanya untuk sumber pertama. Sebuah transaksi diberi label
terverifikasi hanya kalau deskripsinya berasal dari deskriptor bawaan aplikasi yang
sedang Anda jalankan — atau dari server data chain dan isinya identik dengan salinan
bawaan itu, yang membuktikan tidak ada yang diubah di perjalanan. Apa pun selain itu
yang dikirim server tetap didekode dan tetap ditampilkan, dengan satu baris yang
menyatakan bahwa isinya datang dari layanan deskriptor dan tidak ada yang
mengautentikasinya. Layanan itu tidak ditandatangani, jadi tingkat kepercayaannya hanya
setinggi siapa pun yang menjalankannya — itulah salah satu alasan Anda bisa
[menjalankan server sendiri](/id/docs/self-hosting#chain-data).

Jumlah token diformat dengan **desimal on-chain token yang sebenarnya**. Kalau Vela
tidak bisa memastikan desimal sebuah token, jumlahnya ditampilkan seolah token itu punya
18 desimal dan **ditandai belum terverifikasi**, supaya angka yang salah tidak pernah
terlihat seperti angka yang sudah diperiksa.

## Tingkat risiko

Setiap transaksi yang didekode mendapat tingkat risiko supaya pola berbahaya langsung
menonjol:

- **Hati-hati** untuk persetujuan dan permit — Anda memberi izin untuk membelanjakan.
- **Bahaya** untuk yang benar-benar berisiko, seperti **persetujuan token tanpa batas**.
- Risiko lebih rendah untuk tindakan rutin seperti staking atau menyetor.

<Callout type="warning" title="Persetujuan “tanpa batas” ditampilkan merah, dengan pilihan untuk membatasinya">
Persetujuan on-chain dengan jumlah tanpa batas adalah salah satu cara paling umum dana
dikuras di kemudian hari. Saat dApp meminta persetujuan seperti itu (<code>approve</code>,
<code>increaseAllowance</code>, atau <code>approve</code> milik Permit2) di tingkat
"tanpa batas" — 2^200 atau lebih (2^152 untuk Permit2), angka yang dipakai dApp untuk
"tanpa batas" — Vela menampilkannya dengan warna merah dan menawarkan batas: jumlah
tertentu, sebesar saldo Anda (kalau saldonya bisa dibaca), atau — kecuali pada
<code>increaseAllowance</code> — pencabutan. Kalau Anda tidak memilih salah satunya,
persetujuan itu dikirim persis seperti yang dibuat dApp: Permit2 dirancang dengan
mengandalkan persetujuan yang tetap berlaku, dan batch akun pintar membelanjakan allowance
itu dalam transaksi yang sama, jadi batas yang lebih rendah dari pembelanjaan itu membuat
seluruh batch gagal. Di dalam batch, setiap persetujuan tanpa batas bisa diberi batasnya
sendiri dengan cara yang sama (jumlah tertentu atau pencabutan).
Pemeriksaan terakhir sebelum pengiriman membaca calldata mentah, jadi persetujuan tanpa
batas yang tidak pernah ditampilkan di layar persetujuan tidak bisa keluar.
<strong>Persetujuan besar yang terbatas</strong> (bahkan jauh di atas saldo Anda)
ditampilkan dengan peringatan hati-hati. <strong>Permit yang ditandatangani</strong>
(tanda tangan EIP-2612 dan Permit2) tidak bisa dibatasi — dApp mengirim salinannya
sendiri — jadi permit hanya bisa ditandatangani sesuai permintaan atau ditolak: yang tanpa
batas ditampilkan merah, yang terbatas dengan peringatan hati-hati. Permintaan untuk
memberikan <code>setApprovalForAll</code> NFT atas seluruh koleksi belum bisa disetujui di
aplikasi.
</Callout>

## Kalau Vela tidak bisa mendekode sebuah panggilan

Kalau tidak ada deskriptor ERC-7730 tetapi fungsinya tercantum di basis data selector
publik, Vela mendekode panggilan itu secara umum dan memberinya label **upaya terbaik**
(best effort) — sudah didekode, tetapi belum terverifikasi — di bawah spanduk peringatan.
Kalau cara itu pun gagal, atau Vela hanya bisa mendekode sebagian transaksi, Vela
**tidak** berpura-pura memahaminya.

<Callout type="danger" title="Peringatan tanda tangan buta yang tegas">
Kalau sebuah panggilan tidak bisa didekode, Vela menampilkan peringatan tanda tangan
buta yang jelas, bukan ringkasan yang pura-pura ramah. Kalau hanya sebagian kolom yang
bisa diurai, Vela memberi tahu bahwa tampilannya tidak lengkap dan tetap menjaga tingkat
risikonya tinggi. Anda selalu tahu seberapa banyak dari yang Anda tandatangani yang
benar-benar bisa dibaca Vela.
</Callout>

## Kenapa ini penting

Di dompet non-kustodial, tidak ada yang bisa membatalkan transaksi buruk untuk Anda.
Pertahanannya bukan layanan pelanggan — melainkan memahami apa yang Anda setujui
**sebelum** Anda menyetujuinya. Clear signing adalah cara Vela berusaha menunjukkan hal
itu, dan ada batasnya: clear signing hanya bisa sejujur aplikasi yang menampilkannya,
itulah sebabnya [pemeriksaan independen](/id/docs/clear-signing-self-host) penting.
Lihat [whitepaper](/id/docs/whitepaper) untuk posisinya dalam model keamanan Vela.
