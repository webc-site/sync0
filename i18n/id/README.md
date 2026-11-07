# sync0

`sync0` adalah alat baris perintah untuk mentransfer file melintasi jaringan, menyinkronkan direktori secara bertahap, dan mendistribusikan data secara bersamaan ke beberapa mesin.

## Niat desain asli

Di ruang lintas mesin, negara, atau lingkungan dengan penundaan jaringan dan kehilangan paket, alat seperti `rsync` dan `scp` didasarkan pada konfirmasi urutan koneksi tunggal, yang membatasi penggunaan bandwidth; ketika beberapa mesin menarik data dari server yang sama pada saat yang sama, kartu jaringan server sumber dapat dengan mudah menjadi hambatan.

Tujuan dari `sync0` adalah untuk memecahkan dua masalah:

1. Biarkan sinkronisasi file dan direktori antara kedua mesin menjalankan bandwidth fisik penuh, dan hanya mentransfer bagian yang diubah ketika file dimodifikasi sebagian.
2. Beberapa mesin dapat mentransfer blok data satu sama lain saat menyinkronkan direktori yang sama, mengurangi beban pada server sumber dan memperpendek waktu sinkronisasi secara keseluruhan.

## Perbandingan kecepatan dengan rsync

Lingkungan pengujian: Node Eropa $\leftrightarrow$ Node Amerika Utara, penundaan bolak-balik 86 ms, mengukur keseluruhan waktu yang memakan waktu termasuk pemindaian direktori, perbandingan deduplikasi, transmisi jaringan, dan verifikasi penempatan disk:

| Skenario transmisi | Ukuran data | `sync0` | `rsync` | banyak |
| :--- | :---: | :---: | :---: | :---: |
| 10.000 file kecil | 77.6 MB | 0.90 s (688 Mbps) | 4.86 s (128 Mbps) | 5.4x |
| Paket kode dan arsip (`.tar`) | 512 MB | 1.94 s (2111 Mbps) | 9.36 s (438 Mbps) | 4.8x |
| Berkas video (`.mp4`) | 512 MB | 7.36 s (556 Mbps) | 12.11 s (338 Mbps) | 1.6x |
| Sinkronisasi tambahan (memodifikasi 1% konten) | 512 MB | 0.12 s | 1.85 s | 15.4x |

## Metode peningkatan kinerja

- **Transmisi serentak UDP**: Mengirim blok data melalui UDP, dan hanya mengirimkan ulang fragmen yang hilang untuk menghindari kehilangan paket tunggal yang memblokir transfer file berikutnya.
- **Segmentasi dan deduplikasi konten**: Membagi file menjadi blok data sesuai dengan konten dan menghitung nilai pemeriksaan. Konten yang sama yang sudah ada di pihak penerima digunakan kembali secara langsung secara lokal tanpa menduplikasi jaringan.
- **Secara otomatis menyesuaikan kompresi berdasarkan kecepatan jaringan**: Aktifkan kompresi untuk konten yang dapat dikompres seperti teks dan kode; secara otomatis melewati kompresi untuk video, gambar, paket terkompresi, atau pada LAN berkecepatan tinggi untuk mengurangi konsumsi CPU.
- **Penarikan multi-node terdekat**: Saat beberapa mesin menarik direktori yang sama, blok data akan ditarik secara bersamaan dari node yang diunduh di kota yang sama atau dengan latensi lebih rendah.

## Memasang

### Metode 1: Instal melalui Cargo

```bash
cargo install sync0
```

### Metode 2: Unduh biner yang telah dikompilasi sebelumnya

Buka [Rilis GitHub](https://github.com/webc-site/sync0/releases) untuk mengunduh `.tar.xz` paket terkompresi untuk sistem terkait (mendukung Linux x86_64/ARM64, macOS Apple Silicon/Intel, Windows x64/ARM64, Android ARM64/x86_64, FreeBSD, NetBSD). Setelah membuka ritsleting, tempatkan `sync0` ke dalam direktori `PATH`:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Panduan Pengguna

### 1. Mentransfer file atau direktori secara langsung antara dua mesin

Mulai pemantauan pada mesin penerima dan tentukan jalur penerimaan dan penyimpanan:

```bash
sync0 recv 0.0.0.0:9000 ./backup
```

Kirim satu file atau seluruh direktori dari mesin pengirim:

```bash
sync0 send ./project 192.168.1.10:9000
```

Parameter yang umum digunakan:

```bash
# Hapus file berlebihan yang ada di pihak penerima tetapi tidak ada di pihak pengirim (sinkronisasi cermin)
sync0 send ./project 192.168.1.10:9000 --delete

# Batasi kecepatan pengiriman (satuan: byte/detik, misalnya 50 MB/s)
sync0 send ./project 192.168.1.10:9000 --rate 50000000

# Mode senyap (tidak ada bilah kemajuan yang ditampilkan)
sync0 send ./project 192.168.1.10:9000 -q
```

### 2. Menerbitkan direktori bersama dan menarik dari banyak node

Publikasikan direktori bersama (bernama `dataset`) di server asal:

```bash
sync0 share dataset ./data --region SHA
```

Tarik direktori bersama di mesin lain (Anda dapat mengisi beberapa alamat node secara bersamaan, dipisahkan dengan koma untuk menarik secara bersamaan):

```bash
sync0 pull 10.0.0.1:9000,10.0.0.2:9000 dataset ./local_data
```

Tarik hanya subdirektori tertentu di direktori bersama:

```bash
sync0 pull 10.0.0.1:9000 dataset ./local_data --subpath 2026/logs
```

Dorong perubahan lokal kembali ke direktori bersama jarak jauh:

```bash
sync0 push 10.0.0.1:9000 dataset ./local_data
```

### 3. Berlangganan pembaruan otomatis katalog

Berlangganan ke direktori bersama dari node sumber. Ketika file ditambahkan atau diubah di direktori sumber, maka secara otomatis akan disinkronkan ke komputer lokal. Pada saat yang sama, mesin lokal akan secara otomatis berfungsi sebagai node untuk ditarik oleh mesin lain:

```bash
sync0 sub 10.0.0.1:9000 dataset ./local_mirror
```

### 4. Sinkronisasi dua arah dari grup multi-mesin

Beberapa mesin bergabung dengan nama grup yang sama, dan perubahan pada direktori node mana pun akan secara otomatis disinkronkan ke node lain dalam grup:

```bash
sync0 group team-docs ./docs 10.0.0.1:9000,10.0.0.2:9000
```

### 5. Identitas simpul dan kartu nama pribadi

Setiap node mendukung pengaturan nama pengguna, kota, email, situs web, dan tanda tangan yang dipersonalisasi (maksimum 256 karakter), dan meneruskan otentikasi tanda tangan kunci pribadi lokal:

```bash
# Lihat ID kunci publik lokal (tentukan direktori konfigurasi untuk menyimpan kunci)
sync0 id -c ~/.config/sync0

# Hasilkan dan tampilkan kartu nama node dengan tanda tangan yang dipersonalisasi
sync0 profile -c ~/.config/sync0 \
  -u alice \
  --region SHA \
  --email alice@example.com \
  --url https://example.com \
  -b "Shanghai backup node"
```

`share`, `pull`, `sub`, `group` juga dapat diluncurkan dengan parameter profil berikut:

```bash
sync0 share dataset ./data -c ~/.config/sync0 -u alice --region SHA -b "Shanghai backup node"
```

### 6. Peralihan multi-bahasa

`sync0` Mendukung 75 bahasa. Secara default, bantuan dan petunjuk baris perintah dalam bahasa yang sesuai secara otomatis ditampilkan sesuai dengan lingkungan bahasa sistem operasi; jika bahasa sistem tidak ada dalam daftar yang didukung, bahasa Inggris digunakan secara default.

Untuk menentukan bahasa secara manual, atur variabel lingkungan `SYNC0_LANG`:

```bash
SYNC0_LANG=en sync0 --help
SYNC0_LANG=ja sync0 --help
SYNC0_LANG=de sync0 --help
```