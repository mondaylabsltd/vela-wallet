---
title: Jaringan & biaya
description: 12 jaringan yang didukung Vela, cara kerja biaya gas pada abstraksi akun, siapa yang menjalankan relay dan mengumpulkan biayanya, kapan Anda membiayai sendiri aktivasi akun gas, dan bagaimana Vela memilih endpoint RPC.
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Jaringan & biaya

## Jaringan yang didukung

Vela hadir dengan **12 jaringan EVM** bawaan:

| Jaringan | Token biaya asli |
| ----------- | ---------------- |
| Ethereum | ETH |
| BNB Chain | BNB |
| Polygon | POL |
| Arbitrum | ETH |
| Optimism | ETH |
| Base | ETH |
| Avalanche | AVAX |
| Gnosis | xDAI |
| Unichain | ETH |
| Tempo | USD |
| Monad | MON |
| World Chain | ETH |

Dompet Anda punya **alamat yang sama di semuanya**, jadi hanya ada satu alamat untuk
dibagikan ke mana pun.

Anda juga bisa **menambahkan jaringan sendiri** (Pengaturan → Jaringan). Karena Vela
adalah dompet berbasis akun pintar, sebuah jaringan harus menyediakan kontrak yang
diandalkan Vela — EntryPoint ERC-4337, kontrak-kontrak Safe, dan precompile tanda
tangan **P-256 (RIP-7212)** yang memverifikasi passkey Anda di rantai. Vela
memeriksanya otomatis sebelum mengizinkan Anda menambah jaringan.

<Callout type="info" title="Kenapa Gnosis sering muncul">
Selain menjadi salah satu dari 12 jaringan, Gnosis Chain menampung <strong>Indeks
Passkey</strong> milik Vela — kontrak yang menyimpan kunci publik dan nama akun Anda
untuk pemulihan lintas perangkat. Itu terpisah dari jaringan mana yang Anda pakai untuk
bertransaksi.
</Callout>

## Cara kerja biaya (abstraksi akun)

Vela memakai **abstraksi akun ERC-4337**, jadi transaksi tidak disiarkan langsung oleh
Anda — ia berupa **UserOperation** yang diserahkan ke sebuah **relay**, yang
mengirimkannya ke rantai dan mendapat penggantian gasnya. (Spesifikasi ERC-4337
menyebut peran itu *bundler*. Milik Vela disebut relay karena ia melakukan lebih dari
sekadar memaket: ia memberi kuotasi biaya di dalam kanal dan menjalankan protokol akun
gas di bawah ini, dan keduanya bukan bagian dari standar.) Beberapa hal mengikutinya:

- **Gas dibayar dari saldo dompet Anda sendiri** — secara bawaan dengan token asli
  jaringan (ETH, BNB, xDAI…), atau dengan stablecoin yang didukung di tempat relay
  menawarkannya; Anda memilih aset biayanya di layar konfirmasi. Tempo tidak punya koin
  asli, jadi gas di sana selalu diselesaikan dengan stablecoin USD. Tidak ada
  **paymaster** ERC-4337 yang mensponsori — atau menggerbangi — setiap transaksi. (Vela
  bisa saja mensponsori _aktivasi akun gas_ sekali jalan untuk pengguna baru; itu hal
  terpisah, dibahas di bawah.)
- **Relay yang memberi kuotasi harga gas** — ia satu-satunya sumber kebenaran, dan
  dompet menampilkan kuotasi itu lalu menandatangani persis apa yang ditampilkannya.
  Tidak ada pemilih kecepatan: setiap transaksi dikirim dengan prioritas tinggi.
- Total tagihannya adalah **biaya jaringan ditambah biaya layanan relay**, dengan
  tagihan minimum kecil untuk transaksi yang sangat murah. Kuotasi relay itulah
  harganya — tidak ada daftar tarif terpisah yang perlu dilihat. Sebagian mengalir ke
  validator rantai; sisanya membayar relay yang menalangi gas dan menjalankan
  infrastrukturnya.
- Layar konfirmasi menampilkan **perkiraan biaya** dalam aset biaya dan dalam mata uang
  tampilan Anda sebelum Anda menandatangani. Jumlah yang dikuotasi dan penerimanya
  adalah bagian dari yang Anda tandatangani, jadi relay dibayar persis sebesar yang
  ditampilkan — angka yang berubah akan membatalkan tanda tangan Anda.

## Siapa yang menjalankan relay — dan siapa yang mendapat biayanya

Setiap jaringan mengarah ke sebuah relay. Secara bawaan itu **relay milik Vela
sendiri**, dan Anda bisa mengganti endpoint-nya di _Pengaturan → Lanjutan → Endpoint
Layanan_. Satu endpoint berlaku untuk semua jaringan bawaan; jaringan buatan sendiri
menyimpan URL relay yang Anda berikan saat menambahkannya.

Catatan jujur soal kompatibilitas: aplikasinya mengambil kuotasi biaya lewat metode RPC
khusus Vela (`vela_getInBandGasQuote`), dan alur pengirimannya gagal tanpa itu. Jadi
endpoint yang Anda tuju harus menjalankan
[vela-relay](https://github.com/mondaylabsltd/vela-relay) — instans milik Vela atau
milik Anda sendiri. Bundler ERC-4337 umum seperti **Pimlico** atau **Alchemy** tidak
mengimplementasikan metode itu, jadi pada rilis sekarang ia tidak akan jalan dari ujung
ke ujung.

Siapa pun yang mengoperasikan relay untuk sebuah jaringan **mengumpulkan biaya jaringan
itu** — markup relay pada setiap transaksi dan deposit aktivasi akun gas. Jalankan
vela-relay Anda sendiri dan biaya itu membiayai infrastruktur Anda alih-alih milik
Vela; Vela tidak mengambil potongan dari lalu lintas yang Anda arahkan ke tempat lain.

<Callout type="warning" title="Akun gas adalah bagian dari protokol vela-relay">
Langkah <strong>aktivasi akun gas</strong> mengisi sebuah akun relay khusus untuk
dompet Anda di setiap jaringan. Kalau Anda mengarahkan endpoint-nya ke vela-relay yang
Anda hosting sendiri, depositnya mengisi akun relay Anda sendiri, bukan milik Vela.
</Callout>

### Mengaktifkan akun gas (Vela Relay)

Di relay Vela, transaksi pertama Anda pada setiap jaringan akan **mengaktifkan sebuah
akun gas khusus**. Aplikasinya lebih dulu meminta kas relay untuk membiayainya bagi
Anda — ini terjadi diam-diam di dalam alur pengiriman, dan dompet yang disponsori tidak
pernah melihat layar pengisian. Hanya kalau sponsornya ditolak, aplikasinya menampilkan
permintaan pengisian: Anda mengirim sedikit token asli ke alamat akun gas yang
ditampilkannya, dan ia memberi tahu kenapa sponsornya tidak tersedia.

**Anda membayar sendiri biaya aktivasinya** kapan pun sponsor gratis tidak ditawarkan,
yaitu ketika:

- **Kas Vela untuk jaringan itu kosong atau menipis** — dana gratis di rantai itu
  sedang habis.
- **Anda sudah memakai kuota gratisnya** — sponsor dibatasi per dompet, jadi setelah
  beberapa kali pertama, Anda yang membiayai.
- **Relay Vela sama sekali tidak membiayai jaringan itu** — misalnya **jaringan buatan
  sendiri atau jaringan uji yang Anda tambahkan**, yang untuknya Vela tidak memegang
  kas. (Arahkan yang seperti itu ke relay Anda sendiri kalau Anda ingin melewati
  aktivasi sama sekali.)

Deposit aktivasinya **tidak dapat dikembalikan** — ia saldo awal milik relay dan mengisi
dirinya sendiri dari penggantian gas seiring waktu, meski tetap bisa menipis dan perlu
**diaktifkan ulang** nanti. Alamat relay juga bisa berubah saat ada peningkatan layanan,
dan itu menuntut aktivasi baru.

Biayanya diambil dari saldo Anda pada **aset biaya** yang Anda pilih — token asli secara
bawaan. Kalau sebuah pengiriman terhalang karena gas, artinya saldo Anda pada aset biaya
itu tidak menutupi biayanya; di tempat relay menawarkan gas berbasis stablecoin,
mengganti aset biaya di layar konfirmasi bisa membukanya.

Saat Anda mengirim jumlah **maksimum** dari token asli, Vela otomatis menyisihkan cukup
untuk gas agar transaksinya tidak gagal.

## Bagaimana Vela berbicara dengan setiap jaringan

Vela membaca saldo dan mengirim transaksi lewat **sekumpulan endpoint RPC**, bukan satu
penyedia tunggal. Ia mengumpulkan endpoint dari beberapa sumber, menilainya berdasarkan
latensi dan keandalan, dan **beralih otomatis** saat salah satunya lambat atau mati —
sementara mencadangkan endpoint yang buruk — sehingga satu node yang rewel tidak pernah
menjatuhkan seluruh aplikasi.

Berikutnya: [cara kerja passkey](/id/docs/passkeys).
