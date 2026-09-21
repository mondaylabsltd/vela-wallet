---
title: Jaringan & biaya
description: "24 jaringan bawaan Vela, cara menambahkan jaringan lain, persisnya bagaimana biaya sebuah transaksi dihitung dan siapa yang menerimanya, serta apa yang terjadi kalau gas sebuah relay habis."
source: 84328d162a3a
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Jaringan & biaya

## Jaringan bawaan

Vela punya **24 jaringan** bawaan, semuanya mainnet:

| Jaringan | Gas dibayar dengan | Jaringan | Gas dibayar dengan |
| --- | --- | --- | --- |
| Ethereum | ETH | Arc | USDC (koin native-nya) |
| BNB Chain | BNB | X Layer | OKB |
| Polygon | POL | Stable | USDT0 (koin native-nya) |
| Arbitrum | ETH | Soneium | ETH |
| Optimism | ETH | MegaETH | ETH |
| Base | ETH | Robinhood Chain | ETH |
| Avalanche | AVAX | Mantle | MNT |
| Gnosis | xDAI | Kaia | KAIA |
| Unichain | ETH | Celo | CELO |
| Tempo | pathUSD (tidak ada koin native) | Ink | ETH |
| Monad | MON | Plume | PLUME |
| World Chain | ETH | XRPL EVM | XRP |

Di sebagian besar jaringan itu, Anda juga bisa membayar biaya dengan stablecoin USD yang
diterima relay di jaringan tersebut (lihat di bawah).

Dompet Anda punya **alamat yang sama di setiap jaringan**, karena alamat itu dihitung
dari kunci Anda, bukan dari chain-nya.

## Menambahkan jaringan lain

Anda bisa menambahkan jaringan EVM apa pun di **Pengaturan → Jaringan**, asalkan
jaringan itu punya semua yang dibutuhkan dompet Vela: sebelas kontrak standar
(EntryPoint ERC-4337 v0.7, kontrak Safe v1.4.1, modul 4337 dan modul passkey milik Safe,
MultiSend, Multicall3, dan dua deployer deterministik) serta precompile **RIP-7212**
yang memverifikasi tanda tangan passkey di alamat `0x100`. Dompet memeriksa semuanya,
termasuk uji tanda tangan sungguhan terhadap precompile itu, sebelum mengizinkan Anda
menambahkan jaringannya.

Precompile ini syarat mutlak. Alamatnya adalah bagian dari cara setiap alamat Vela
dihitung, jadi tidak ada verifier cadangan dan tidak ada cara untuk men-deploy-nya
belakangan. Kalau sebuah chain punya precompile itu tetapi kekurangan sebagian kontrak,
[penyiapan chain](/id/chain-setup) menunjukkan apa yang kurang dan men-deploy yang bisa
di-deploy siapa saja. Ada satu celah dalam pemeriksaannya: dompet dengan lebih dari satu
kunci juga membutuhkan factory signer passkey milik Safe di jaringan itu, yang belum
diperiksa; tanpanya, hanya kunci pertama yang bisa menandatangani di sana.

## Bagaimana transaksi dibayar

Vela adalah dompet ERC-4337: Anda tidak menyiarkan transaksi sendiri. Aplikasi menyusun
sebuah **UserOperation**, Anda menandatanganinya dengan salah satu kunci Anda, lalu
sebuah **relay** mengirimkannya ke chain dan menalangi gasnya lebih dulu. (ERC-4337
menyebut peran ini bundler.) Relay mendapat penggantiannya **di dalam operasi Anda**:
pembayarannya berupa transfer dari dompet Anda ke relay yang berada dalam batch yang
sama dengan transaksi Anda, sehingga ikut dilindungi tanda tangan Anda. Tidak ada
paymaster, tidak ada yang mensponsori gas Anda, dan tidak ada yang bisa menolak
transaksi Anda karena kebijakan sponsor.

### Berapa biayanya

Layar konfirmasi menampilkan satu jumlah, dalam koin biaya dan dalam mata uang tampilan
Anda. Cara menghitungnya:

- **Gas yang dicadangkan dompet.** Dompet menyimulasikan transaksi lalu mencadangkan gas
  lebih banyak daripada perkiraan pemakaiannya: perkiraan verifikasi dan eksekusi
  masing-masing dinaikkan setengahnya, dengan batas minimum (misalnya, verifikasi
  minimal 300.000 gas setelah dompet di-deploy, dan 2.000.000 untuk transaksi yang
  men-deploy-nya).
- **Harga gas.** Yang lebih tinggi antara hasil baca dompet sendiri atas harga gas
  jaringan dan harga relay untuk kecepatan yang Anda pilih. Kecepatan bawaannya
  *cepat*, yang dihargai relay sekitar 1,8 × base fee ditambah dua kali priority fee.
- **Biaya = 3 × gas yang dicadangkan × harga gas**, dengan minimum sekitar $0,01. Di
  Tempo, pengalinya 2 dan biayanya dibayar dengan pathUSD.

Karena cadangannya jauh di atas pemakaian sebenarnya dan harganya diberi ruang lebih,
**biayanya sering sepuluh kali lipat atau lebih dari biaya on-chain transaksi yang
sebenarnya**, dan lebih besar lagi untuk transaksi pertama di suatu jaringan. Relay
membayar biaya sebenarnya dan menyimpan sisanya; tidak ada yang dikembalikan. Di
jaringan yang murah, jumlahnya hanya beberapa sen; di mainnet Ethereum, jumlahnya bisa
lumayan besar. Jumlah pastinya ada di layar konfirmasi sebelum Anda menandatangani.

<Callout type="info" title="Yang Anda lihat adalah yang Anda bayar">
Jumlah biaya dan alamat tujuannya adalah bagian dari operasi yang Anda tandatangani.
Relay yang mengubah salah satunya akan membuat tanda tangan Anda tidak sah, jadi Anda
membayar persis sebesar jumlah yang ditampilkan — tidak lebih, bahkan kalau harga gas
naik sebelum transaksi masuk ke blok. Kuotasi harga gas dari relay yang lebih dari tiga
kali hasil baca dompet sendiri akan ditolak.
</Callout>

### Anda bisa membayar dengan apa

- **Koin native** jaringan itu, selalu.
- **Stablecoin USD** dari daftar relay untuk jaringan itu, bila relay bisa menghargai
  koin native-nya. Stablecoin yang sama sekali tidak Anda miliki disembunyikan.
- Di **Tempo**, yang tidak punya koin native, hanya **pathUSD**.

Anda memilih koin biaya, beserta kecepatannya (*Lambat*, *Standar*, atau *Cepat*), di
layar konfirmasi dan di Pengaturan.

### Transaksi pertama Anda di suatu jaringan

Anda bisa menerima di jaringan mana pun sebelum dompet Anda ada di sana. Saat pertama
kali Anda mengirim dari suatu jaringan, transaksi itu sekaligus men-deploy kontrak dompet
Anda (dan satu kontrak signer kecil untuk tiap kunci tambahan). Gas untuk deploy itu
termasuk dalam biaya transaksi tersebut, jadi pengiriman pertama di tiap jaringan lebih
mahal daripada pengiriman berikutnya.

Saat Anda mengirim koin native dalam jumlah **maksimum**, Vela menyisihkan cukup untuk
biayanya.

## Siapa yang menjalankan relay — dan siapa yang menerima biayanya

Secara bawaan, setiap jaringan memakai **relay milik Vela**, dan biayanya masuk ke Vela.
Anda bisa mengarahkan dompet ke relay lain di **Pengaturan → Lanjutan → Endpoint
Layanan**; satu alamat melayani semua jaringan bawaan, sedangkan jaringan kustom tetap
memakai alamat relay yang tercatat saat jaringan itu ditambahkan. Relay-nya harus
[vela-relay](https://github.com/mondaylabsltd/vela-relay) — milik Vela atau yang Anda
jalankan sendiri — karena dompet meminta kuotasi biaya dengan metode khusus Vela yang
tidak diimplementasikan bundler umum seperti Pimlico atau Alchemy. Siapa pun yang
menjalankan relay yang Anda pakai, dialah yang menerima biayanya;
[panduan hosting sendiri](/id/docs/self-hosting#relay) menjelaskan cara menjalankannya.

Relay menerima operasi yang sudah ditandatangani. Relay tidak bisa mengubah penerima,
jumlah, biaya, atau apa pun lainnya. Relay bisa menunda atau menolaknya, dan menentukan
kapan transaksi itu masuk ke chain — jadi untuk sebuah swap, secara teori relay bisa
bertransaksi mendahului Anda (front-running) dalam batas slippage Anda.

### Kalau gas sebuah relay habis

Relay membayar gas dari **treasury**-nya sendiri di tiap jaringan. Kalau treasury itu
kosong, layar kirim memberi tahu Anda sebelum Anda menandatangani:

- Di jaringan yang dilayani relay Vela, operator relay (Vela) perlu mengisinya kembali;
  Anda bisa melaporkannya. Kalau tidak bisa menunggu, Anda **boleh** mengirim sedikit
  koin native ke treasury itu sendiri. Kontribusi itu **tidak dapat dikembalikan** dan
  **tidak** membayar transaksi Anda sendiri.
- Di jaringan kustom, mengisi dana relay menjadi urusan siapa pun yang menjalankannya —
  bisa jadi Anda sendiri.

Tidak ada akun gas per dompet dan tidak ada deposit aktivasi: versi awal Vela pernah
punya mekanisme itu, dan sekarang sudah tidak ada.

## Cara Vela membaca tiap jaringan

Vela membaca saldo dan menyimulasikan transaksi lewat **sekumpulan endpoint RPC** per
jaringan — endpoint bawaan, cadangan publik, serta kunci penyedia atau endpoint yang Anda
tambahkan — dan pindah ke endpoint berikutnya kalau satu endpoint lambat atau mati. Anda
bisa mengatur endpoint sendiri per jaringan di **Pengaturan → Jaringan**. (Aplikasi
Android saat ini memakai satu endpoint per jaringan, tanpa pengalihan otomatis, dan
aplikasi iPhone belum mengizinkan Anda mengubahnya.)

Berikutnya: [cara kerja passkey](/id/docs/passkeys).
