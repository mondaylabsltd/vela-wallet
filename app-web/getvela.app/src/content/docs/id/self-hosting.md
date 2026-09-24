---
title: Panduan hosting sendiri
description: "Semua yang dijalankan Vela untuk Anda, fungsi masing-masing, dan cara menggantinya dengan milik Anda sendiri — relay, indeks kunci publik, data chain, kurs, dan aplikasinya — ditambah satu hal yang tidak bisa Anda ganti dan cara bertahan tanpa getvela.app."
source: 3617d6d07f71
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Panduan hosting sendiri

Uang Anda ada di kontrak Safe on-chain, dikendalikan oleh kunci Anda. Tidak ada yang
dijalankan Vela yang bisa memindahkannya. Yang dijalankan Vela adalah mesin yang membuat
dompet nyaman dipakai: relay yang mengirim transaksi Anda, indeks yang membantu perangkat
baru menemukan dompet Anda, direktori data chain, sumber kurs, dan aplikasinya sendiri.

Halaman ini mencantumkan setiap bagian itu, apa yang rusak tanpanya, dan cara menjalankan
milik Anda sendiri. Halaman ini juga membahas satu bagian yang tidak bisa Anda ganti —
domain tempat passkey Anda terikat — dan apa yang harus dilakukan kalau getvela.app
hilang.

<Callout type="info" title="Untuk siapa halaman ini">
Anda sebaiknya terbiasa dengan terminal, Docker atau Cloudflare Workers, dan mengisi dana
ke sebuah alamat di suatu chain. Tidak ada yang di sini yang Anda butuhkan untuk memakai
Vela sehari-hari.
</Callout>

## Petanya

| Bagian | Fungsinya | Bawaan Vela | Bisa Anda ganti? | Tanpanya |
| --- | --- | --- | --- | --- |
| **Relay** | Menerima operasi yang Anda tandatangani, membayar gas, mengirimkannya, dan menagih biaya yang Anda tandatangani | `vela-relay-cf.getvela.app` | Ya — jalankan [vela-relay](#relay) dan arahkan dompet ke sana | Anda tidak bisa mengirim |
| **Indeks kunci publik** | Mendaftarkan kunci dompet baru on-chain; menjawab "kunci ini bagian dari dompet mana?" | `p256-index-v2.getvela.app` | Ya — jalankan [p256-index](#index) | Dompet baru tidak bisa dibuat; saat masuk, aplikasi beralih membaca chain langsung |
| **Kontrak registri** | Catatan publik permanen tentang kunci setiap dompet | `0x94fD…1EA9` di Gnosis | Tidak perlu — tidak ada yang memilikinya; dompet membacanya langsung | — |
| **Data chain** | Detail jaringan, daftar token, logo, deskriptor clear signing | `ethereum-data.getvela.app` | Ya — jalankan [ethereum-data](#chain-data) | Tidak ada daftar token atau logo; lebih sedikit transaksi yang bisa didekode; penambahan jaringan gagal |
| **Kurs** | Nilai fiat dalam mata uang tampilan Anda | `vela-currency.getvela.app` | Ya — jalankan [vela-currency](#exchange-rates) atau sumber apa pun yang kompatibel dengan Frankfurter | Aplikasi beralih ke kurs Chainlink on-chain sebisa mungkin (desktop menampilkan USD) |
| **Node RPC** | Membaca saldo, menyimulasikan transaksi | Endpoint publik per jaringan | Ya — per jaringan, di Pengaturan → Jaringan | Vela beralih antar-endpoint secara otomatis |
| **Aplikasinya** | Dompet itu sendiri | wallet.getvela.app, build rilis | Ya — [kompilasi sendiri](#web-app) | — |
| **getvela.app** | Domain tempat passkey Anda terikat | — | **Tidak** — lihat [di bawah](#if-getvela-app-disappears) | — |

Ada juga beberapa layanan pihak ketiga yang dihubungi dan bukan milik Vela: basis data
function selector publik (sourcify, openchain, 4byte) yang dipakai sebagai jalan terakhir
saat mendekode transaksi, direktori autentikator yang menamai model kunci keamanan Anda,
serta server tunnel Apple dan Google saat Anda menandatangani dengan ponsel lewat pindai
kode QR.

## Satu hal yang tidak bisa Anda ganti: domain passkey

<span id="if-getvela-app-disappears"></span>

Sebuah passkey milik situs web tempat passkey itu dibuat. Kunci Vela dibuat untuk
`getvela.app`. Browser hanya menawarkannya ke halaman di getvela.app atau subdomainnya
(atau ke origin yang dinyatakan getvela.app sebagai terkait), dan passkey bawaan ponsel
hanya berfungsi di aplikasi yang dijamin oleh getvela.app. Di luar browser, aturannya
lebih longgar: Chrome mengizinkan ekstensi yang punya izin untuk getvela.app memakainya,
dan program di komputer Anda bisa langsung meminta tanda tangan getvela.app dari kunci
keamanan atau ponsel — begitulah aplikasi yang dikompilasi sendiri bekerja, dan itulah
sebabnya perangkat lunak yang Anda jalankan itu penting. Ada dua akibatnya.

**Salinan dompet web di domain Anda sendiri adalah dompet yang berbeda.** Kalau disajikan
dari `wallet.example.com`, kode yang sama membuat passkey untuk `wallet.example.com` —
kunci baru, dan karena itu alamat baru. Salinan itu tidak bisa menandatangani untuk
dompet yang dibuat di wallet.getvela.app. Salinan itu tetap berguna: untuk dompet yang
Anda buat di sana, atau untuk menjalankan seluruh sistemnya sendiri dari nol.

**Untuk dompet yang sudah ada, cara-cara ini tetap berfungsi kalau getvela.app mati atau
hilang:**

| Jalan masuk | Kunci yang bisa dipakai | Cara mendapatkannya |
| --- | --- | --- |
| **Ekstensi browser Vela** (browser berbasis Chromium: Chrome, Edge, Brave) | Kunci apa pun yang bisa dijangkau browser: passkey perangkat ini, kunci keamanan USB (NFC bila komputernya mendukung), ponsel lewat QR | File zip rilis dari [GitHub](https://github.com/mondaylabsltd/vela-wallet/releases), atau [kompilasi sendiri](#web-app) |
| **Aplikasi desktop atau ponsel yang Anda kompilasi sendiri** | Ponsel lewat QR, dan kunci keamanan USB | [Kompilasi sendiri](#web-app) |
| **Aplikasi dari toko aplikasi dan aplikasi desktop yang dinotarisasi** | Ponsel lewat QR dan kunci keamanan selalu bisa; passkey "Perangkat ini" hanya selama sistem operasi masih bisa memverifikasi aplikasinya terhadap getvela.app | Rilis GitHub (toko aplikasi menyusul) |

Ekstensi bisa memakai kunci `getvela.app` karena Chrome mengizinkan ekstensi yang punya
izin untuk sebuah situs memakai passkey situs itu. Browser memeriksa izin itu secara
lokal; kami sudah mengukur bahwa ini berfungsi, meski belum dengan domainnya benar-benar
offline. Aplikasi yang dikompilasi sendiri bisa memakai ponsel atau kunci keamanan karena
Vela berkomunikasi langsung dengan keduanya; passkey bawaan ponsel ("Perangkat ini")
mengharuskan aplikasinya ditandatangani Vela, dan aplikasi Anda tidak.

[Halaman tanda tangan](/id/docs/clear-signing-self-host) bukan jalan masuk tersendiri:
halaman itu menandatangani permintaan yang dikirim program lain, dan belum ada aplikasi
Vela yang mengirimkannya.

<Callout type="warning" title="Siapa pun yang menguasai domain bisa meminta tanda tangan">
Halaman apa pun yang disajikan dari getvela.app atau salah satu subdomainnya — atau oleh
siapa pun yang menguasai domain itu di masa depan — bisa meminta tanda tangan dari kunci
Anda, dan permintaan konfirmasi sistem menampilkan "getvela.app", bukan transaksinya.
Begitulah passkey bekerja di mana pun. Karena itulah situs web Vela melarang halamannya
sendiri memakai passkey. Ini juga alasan ekstensi dan aplikasi yang dikompilasi sendiri
penting: keduanya membawa kodenya sendiri, meskipun secara bawaan keduanya masih mengambil
deskriptor dan memakai layanan di bawah getvela.app.
</Callout>

## Arahkan dompet ke layanan Anda

Setiap aplikasi punya empat kolom di **Pengaturan → Lanjutan → Endpoint Layanan** (di
desktop, **Pengaturan → Endpoint Layanan**): **Indeks data chain**, **Indeks passkey**,
**Vela Relay**, dan **Kurs fiat**. Setiap kolom menampilkan bawaan Vela sampai Anda
mengubahnya; **Setel Ulang ke Bawaan** mengembalikan keempatnya. Untuk relay, indeks,
dan data chain, dompet memanggil `/api/health` dan menampilkan lencana status, yang hanya
hijau kalau endpoint itu menyebut nama layanan yang benar dan melaporkan
`status: "ok"`. Apa pun warnanya, yang Anda ketik tetap disimpan — tunggu sampai hijau.

| Layanan | `service` di `/api/health` |
| --- | --- |
| Relay | `vela-relay` |
| Indeks kunci publik | `webauthn-p256-publickey-registry` |
| Data chain | `ethereum-data` |
| Kurs | tidak diperiksa berdasarkan nama — harus mengembalikan daftar kurs berbasis USD |

Keempat aplikasi menghormati keempat kolom itu, dan endpoint yang diubah berlaku pada
panggilan berikutnya, bukan pada peluncuran berikutnya: setiap jalur — membuat dompet,
masuk, mencari nama untuk sebuah alamat — membaca endpoint tepat pada saat memakainya.
Di iOS, indeks passkey juga bisa diubah di layar masuk kalau indeks bawaan tidak bisa
dijangkau.

(Sampai September 2026 ada empat pengecualian untuk hal itu; yang terburuk adalah
halaman iOS yang menampilkan nilai contoh dan tidak menyimpan apa pun. Semuanya sudah
diperbaiki.)

## Jalankan relay Anda sendiri

<span id="relay"></span>

Relay-nya adalah [vela-relay](https://github.com/mondaylabsltd/vela-relay) (Rust, MIT).
Satu deployment melayani semua chain: dompet memanggil `https://your-relay/<chainId>`.
Relay-nya harus vela-relay — dompet meminta kuotasi biaya dengan metode khusus Vela yang
tidak diimplementasikan bundler ERC-4337 umum.

**Yang Anda butuhkan**

- Docker plus Redis dan server [Iggy](https://iggy.apache.org) yang sudah Anda jalankan,
  atau akun Cloudflare dengan paket **Workers Paid** serta Node.js dan toolchain Rust
  (dengan target `wasm32-unknown-unknown`) di komputer Anda.
- Sebuah `OPERATOR_SECRET` (hex, minimal 32 byte). Dari secret ini diturunkan satu alamat
  treasury dan sekumpulan alamat relayer, sama di setiap chain. Jaga kerahasiaannya:
  secret ini mengendalikan dana relay.
- Gas di setiap chain yang ingin Anda layani: kirim koin chain itu (pathUSD di Tempo) ke
  alamat treasury Anda. Treasury akan mengisi ulang para relayer.

**Docker**

```sh
git clone https://github.com/mondaylabsltd/vela-relay
cd vela-relay
cp .env.example .env
# di .env: VELA_RELAY_IGGY_URL, VELA_RELAY_REDIS_URL, OPERATOR_SECRET,
# VELA_RELAY_CHAIN_DIRECTORY_URL kalau Anda menjalankan data chain sendiri,
# dan VELA_RELAY_IMAGE diisi dengan image rilis yang Anda percayai (lihat docs/docker.md)
docker compose pull relay
docker compose up -d --no-build
curl --fail http://127.0.0.1:4567/readyz
```

Dua-duanya bisa: image yang sudah dipublikasikan paling cepat, dan
`docker compose up --build` membangun hal yang sama dari kode sumber yang bisa Anda
baca. Tanpa Docker, `cargo run --release --bin vela-relay` menjalankannya secara
langsung.

**Cloudflare Workers**

```sh
cd vela-relay/vela-relay-cf
npx wrangler queues create vela-relay-ops
npx wrangler queues create vela-relay-dlq
npx wrangler secret put OPERATOR_SECRET
# data chain sendiri: tambahkan "VELA_RELAY_CHAIN_DIRECTORY_URL" di bawah "vars" di wrangler.jsonc
npx wrangler deploy
```

**Periksa**

```sh
curl https://your-relay/api/health        # {"service":"vela-relay","status":"ok",…}
curl https://your-relay/v1/treasury/100   # alamat treasury Anda di Gnosis, dan apakah perlu diisi gas
```

Lalu isikan `https://your-relay` di kolom **Vela Relay**.

**Perlu diketahui**

- Biaya yang dibayar dompet masuk ke treasury Anda. Dompet menghitungnya dengan cara yang
  sama, relay mana pun yang Anda pakai (lihat [jaringan & biaya](/id/docs/networks-and-fees)).
- Jaringan kustom yang Anda tambahkan sebelum mengganti relay tetap memakai alamat relay
  yang tercatat saat jaringan itu ditambahkan.
- Relay membaca detail tiap chain dan stablecoin yang diterimanya dari sebuah direktori
  chain: `ethereum-data.getvela.app`, kecuali Anda mengisi
  `VELA_RELAY_CHAIN_DIRECTORY_URL` dengan [milik Anda sendiri](#chain-data). Pengaturan ini ada sejak vela-relay v0.9.6; build yang lebih lama selalu membaca salinan milik Vela.

## Jalankan indeks kunci publik Anda sendiri

<span id="index"></span>

Indeksnya adalah [p256-index](https://github.com/mondaylabsltd/p256-index) (Rust, MIT). Saat
sebuah dompet dibuat, indeks ini memeriksa bukti setiap kunci, lalu menulis kelompok
kunci itu ke **kontrak registri** di Gnosis dan membayar gasnya. Tetap pakai registri
yang sudah ada di `0x94fD1A891EB6c5F340622Baf2F3A0cb70A941EA9`: registri itu tidak punya
pemilik, alamat mana pun yang punya dana bisa menulis ke sana, dan setiap aplikasi Vela
membacanya langsung. Registri buatan Anda sendiri tidak akan terlihat oleh
aplikasi-aplikasi itu.

**Yang Anda butuhkan**

- Docker dengan Redis dan Iggy (versi server), atau akun Cloudflare (versi Worker, yang
  README-nya sendiri mencatat bahwa penulisan on-chain-nya belum diuji secara end-to-end).
- Kunci privat Gnosis yang berisi xDAI. Mendaftarkan satu dompet menghabiskan sekitar
  1,1 juta gas untuk satu kunci dan sekitar 3,6 juta untuk tujuh kunci.
- Pengaturan berikut:

```dotenv
P256_INDEX_IGGY_URL=iggy+tcp://user:password@iggy.example:5100
P256_INDEX_REDIS_URL=redis://redis.example:6379/0
P256_INDEX_CONTRACT_ADDRESS=0x94fD1A891EB6c5F340622Baf2F3A0cb70A941EA9
P256_INDEX_DOMAIN_REGISTRY=0x5266DfF591B9F9EecfEdb8E7EfEf6c687854edaf
PRIVATE_KEY=0x…
```

`P256_INDEX_DOMAIN_REGISTRY` adalah yang paling sering terlewat: tanpanya, server
membagikan challenge yang ditolak kontrak, dan setiap pendaftaran gagal. Variabel ini
ada di `.env.example`, dan nilainya harus sama dengan `DOMAIN_REGISTRY` milik kontrak
yang sudah di-deploy — mulai VERSION 12 registri, domain challenge dipatok saat deploy,
jadi nilainya tidak berubah ketika kontraknya di-deploy ulang.

**Jalankan dan periksa**

```sh
git clone https://github.com/mondaylabsltd/p256-index
cd p256-index
cargo run --release -p p256-index-server
curl https://your-index/api/health   # "service":"webauthn-p256-publickey-registry","status":"ok"
```

Server ini mendengarkan di HTTP biasa (port 11256 secara bawaan); pasang proxy TLS di
depannya, karena dompet hanya menerima endpoint `https://`. `docker build -f
p256-index-server/Dockerfile .` juga berhasil dari clone yang bersih; kalau memakai
Compose, salin dulu `.env.example` menjadi `p256-index-server/.env`.

**Kalau tidak ada indeks sama sekali yang menjawab**, dompet yang sudah ada tetap
berfungsi: saat masuk, aplikasi membaca kontrak registri di Gnosis (lalu Ethereum) lewat
node RPC Anda. Dompet dengan satu kunci bahkan bisa dibangun ulang dari dua tanda tangan
tanpa melibatkan registri. Membuat dompet baru memang membutuhkan indeks, karena harus
ada yang membayar pendaftarannya.

## Jalankan data chain Anda sendiri

<span id="chain-data"></span>

Data chain adalah [ethereum-data](https://github.com/atshelchin/ethereum-data) (MIT):
JSON statis dan gambar untuk sekitar 2.600 jaringan beserta token-tokennya, ditambah
deskriptor ERC-7730 yang dipakai Vela untuk menjelaskan transaksi.

```sh
docker run -d --name ethereum-data -p 3000:3000 --restart unless-stopped \
  ghcr.io/atshelchin/ethereum-data:latest
curl http://localhost:3000/api/health   # "service":"ethereum-data","status":"ok"
```

README-nya juga menjelaskan cara build dari kode sumber dan deploy ke Cloudflare.
Sajikan lewat HTTPS, lalu isikan alamatnya di kolom **Indeks data chain**.

Relay juga membaca file-file ini, termasuk satu kolom khusus Vela (daftar `stables`
menentukan stablecoin mana yang bisa membayar biaya). Arahkan relay ke salinan Anda
dengan `VELA_RELAY_CHAIN_DIRECTORY_URL=https://your-chain-data`; relay menyimpan entri
tiap jaringan di cache selama satu jam.

## Jalankan layanan kurs Anda sendiri

<span id="exchange-rates"></span>

[vela-currency](https://github.com/mondaylabsltd/vela-currency) (MIT) menerbitkan ulang
kurs harian Bank Sentral Eropa. Layanan ini tidak butuh kunci API apa pun.

```sh
docker run -d -p 8080:8080 -v rates-data:/data ghcr.io/mondaylabsltd/vela-currency:latest
curl "http://localhost:8080/v2/rates?base=USD"
```

Isikan `https://your-host/v2/rates?base=USD` di kolom **Kurs fiat**. Layanan apa pun yang
kompatibel dengan Frankfurter juga bisa. Pertahankan `?base=USD`: setiap konversi
mengandalkannya.

## Kompilasi aplikasinya sendiri

<span id="web-app"></span>

Semua aplikasinya ada di [satu repositori](https://github.com/mondaylabsltd/vela-wallet)
(MIT). README-nya mencantumkan langkah build tiap aplikasi; versi singkatnya:

| Aplikasi | Build | Bisa menandatangani untuk dompet getvela.app Anda yang sudah ada? |
| --- | --- | --- |
| Ekstensi browser | `cd app-web/vela-wallet && pnpm install && pnpm build:extension`, lalu muat `extension/dist` lewat **Muat yang belum dibuka** di `chrome://extensions` | Ya, dengan kunci apa pun |
| Dompet web | `cd app-web/vela-wallet && pnpm install && pnpm build`; di-deploy sebagai Cloudflare Worker | Tidak — di domain Anda, ini dompet yang berbeda (lihat di atas) |
| Desktop | `cd app-desktop/vela-wallet && cargo run` (skrip pengemasan ada di README-nya) | Ya, dengan ponsel lewat QR atau kunci keamanan USB |
| Android | Buat binding core-nya dulu, lalu `./gradlew :app:installDebug` | Ya, dengan ponsel lewat QR atau kunci keamanan USB |
| iOS | `./rust/scripts/build-ios-xcframework.sh`, lalu build di Xcode dengan tim developer Anda sendiri | Ya, dengan ponsel lewat QR atau YubiKey USB-C / Lightning (firmware 5.8 atau yang lebih baru) |

Passkey "Perangkat ini" di aplikasi yang dikompilasi sendiri tidak akan berfungsi untuk
dompet getvela.app: Apple dan Google hanya mengizinkan aplikasi yang ditandatangani Vela
memakai passkey `getvela.app`.

## Menambahkan jaringan yang tidak dibawa Vela

Vela berjalan di chain EVM apa pun yang punya precompile P-256 dan kontrak-kontrak standar
yang diperiksanya. [Penyiapan chain](/id/chain-setup) memberi tahu apa yang kurang di
sebuah chain dan men-deploy yang bisa di-deploy siapa saja;
[jaringan & biaya](/id/docs/networks-and-fees) menjelaskan persyaratannya. Pemeriksaannya
sudah mencakup dua kontrak yang dibutuhkan dompet dengan lebih dari satu kunci, dan
menandainya sebagai kontrak semacam itu — chain tanpa keduanya tetap menjalankan dompet
satu kunci.

## Yang masih mengarah ke Vela setelah semua ini

Kalau Anda mengganti semua yang di atas, yang berikut ini masih tersisa:

- **Direktori autentikator** yang menamai model kunci keamanan — hanya kosmetik;
  aplikasinya akan menampilkan nama umum sebagai gantinya.
- **File asosiasi getvela.app**, yang dibutuhkan aplikasi dari toko aplikasi untuk passkey
  "Perangkat ini". Ponsel atau kunci keamanan tidak membutuhkannya.

Dan yang berikut ini bukan milik Vela: basis data selector publik, tunnel milik Apple dan Google untuk menandatangani lewat
ponsel, dan penyedia RPC mana pun yang Anda pilih.

Berikutnya: [halaman tanda tangan yang bisa Anda jalankan sendiri](/id/docs/clear-signing-self-host).
