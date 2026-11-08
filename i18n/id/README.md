# sync0

`sync0` adalah alat berbagi file peer-to-peer (P2P) dan sinkronisasi multi-mesin yang bersaing dengan Resilio Sync.

Setiap mesin adalah server dan klien: node mana pun dapat menerbitkan beberapa grup berbagi file. Setelah node lain bergabung, mereka dapat secara otomatis memperoleh node benih yang tersedia dari server rekan dan menariknya secara bersamaan dari berbagai sumber. Setelah sinkronisasi selesai, benih akan secara otomatis dibuat untuk lebih banyak node guna mempercepat pengunduhan.

## Sorotan Inti

- **Setiap orang adalah server dan klien**: Sebanding dengan Resilio Sync, tidak ada ketergantungan server pusat. Setiap node dapat membuat beberapa grup berbagi file, dan juga dapat bergabung dengan grup berbagi orang lain; node secara otomatis bertukar dan mendapatkan node benih yang tersedia untuk mencapai penarikan bersamaan dan sinkronisasi waktu nyata dari beberapa mesin.
- **Alamat simpul bebas IP**: Secara default, kunci simpul dan `base64url` alamat `iroh` yang dikodekan secara otomatis dibuat di `~/.config/sync0` tanpa perlu mengingat atau mengekspos IP publik.
- **Sinkronisasi multi-pilih sesuai permintaan**: Mendukung sinkronisasi seluruh grup berbagi, dan juga mendukung sinkronisasi sesuai permintaan dengan memilih beberapa file atau subdirektori secara bersamaan.
- **Izin hierarki anggota grup dan kontrol direktori unggah**: Mendukung izin baca-saja (`ro`), unggahan baca-tulis (`rw`), dan manajemen (`admin`), dan dapat secara individual menentukan awalan jalur yang boleh diunggah oleh setiap anggota (secara default, dibolehkan mengunggah ke direktori akar berbagi).
- **Sinkronisasi kecepatan penuh dan bertahap di seluruh jaringan**: Transmisi file besar secara bersamaan, agregasi file kecil berukuran besar di memori, dan hanya blok data yang disinkronkan ketika file dimodifikasi sebagian.

## Perbandingan kecepatan dengan rsync

### Pengukuran aktual sumber tunggal jaringan lintas samudera (Tiongkok → Amerika Serikat, penundaan 260 ms)

| Skenario transmisi | Ukuran data | model | `sync0` | `rsync -az` | Tingkatkan pengganda |
| :--- | :---: | :---: | :---: | :---: | :---: |
| 500 file kecil | 1.91 MB | Sinkronisasi penuh | 18.84 Mbps | 5.32 Mbps | 3.5x |
| 500 file kecil | 1.91 MB | Sinkronisasi tambahan | 27.15 Mbps | 3.83 Mbps | 7.1x |
| Paket kode dan arsip (`.tar`) | 10.0 MB | Sinkronisasi penuh | 139.70 Mbps | 24.97 Mbps | 5.6x |
| Paket kode dan arsip (`.tar`) | 10.0 MB | Sinkronisasi tambahan | 83.38 Mbps | 26.02 Mbps | 3.2x |
| Berkas video (`.mp4`) | 15.0 MB | Sinkronisasi penuh | 54.06 Mbps | 27.04 Mbps | 2.0x |
| Berkas video (`.mp4`) | 15.0 MB | Sinkronisasi tambahan | 216.83 Mbps | 21.51 Mbps | 10.1x |

### Tes tarik multi-node P2P jaringan lintas samudera (Tiongkok → Amerika Serikat dual node, penundaan 260 ms)

Perbandingan terukur dari simpul benih multi-sumber bergaris tarikan bersamaan:

| Skenario transmisi | Ukuran data | Penyemaian simpul tunggal | Penyemaian bersamaan P2P dua simpul | Tingkatkan pengganda |
| :--- | :---: | :---: | :---: | :---: |
| 500 file kecil | 1.97 MB | 5.28 Mbps | 7.87 Mbps | 1.5x |
| Paket kode dan arsip (`.tar`) | 30.0 MB | 56.35 Mbps | 83.17 Mbps | 1.5x |
| Berkas video (`.mp4`) | 30.0 MB | 27.00 Mbps | 39.46 Mbps | 1.5x |

### Pengukuran aktual jalur khusus berkecepatan tinggi lintas benua (Eropa → Amerika Utara, penundaan 86 ms)

| Skenario transmisi | Ukuran data | `sync0` | `rsync` | Tingkatkan pengganda |
| :--- | :---: | :---: | :---: | :---: |
| 10.000 file kecil | 77.6 MB | 688 Mbps | 128 Mbps | 5.4x |
| Paket kode dan arsip (`.tar`) | 512 MB | 2111 Mbps | 438 Mbps | 4.8x |
| Berkas video (`.mp4`) | 512 MB | 556 Mbps | 338 Mbps | 1.6x |
| Sinkronisasi tambahan (memodifikasi 1% konten) | 512 MB | 34133 Mbps | 2214 Mbps | 15.4x |

## Memasang

### Metode 1: Instal melalui Cargo

```bash
cargo install sync0
```

### Metode 2: Unduh biner yang telah dikompilasi sebelumnya

Buka [Rilis GitHub](https://github.com/webc-site/sync0/releases) untuk mengunduh paket terkompresi `.tar.xz` untuk sistem terkait (mendukung Linux, macOS, Windows, Android, FreeBSD, NetBSD), unzip dan masukkan ke dalam direktori `PATH`:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Mulailah dengan cepat

`sync0` menggunakan `~/.config/sync0` sebagai direktori konfigurasi secara default (jika Anda perlu beralih ke direktori lain, Anda dapat menambahkan `-c <directory>`).

### 1. Lihat alamat node lokal dan pengaturan kartu nama

```bash
# Lihat alamat node lokal (kunci dibuat secara otomatis di ~/.config/sync0 untuk pertama kalinya)
sync0 id

# Hasilkan kartu nama simpul pribadi (mendukung nama pengguna, kota, email, situs web, dan tanda tangan dalam 256 kata)
sync0 profile -u alice --region SHA --email alice@example.com --url https://example.com -b "Shanghai node"
```

### 2. Publikasikan beberapa grup berbagi file dan atur izin anggota grup (`.nt` Konfigurasi)

`sync0` dikonfigurasi dalam format [NestedText](https://nestedtext.org) (`.nt`). Setiap pengguna memiliki file konfigurasi `.nt` terpisah. Anda dapat menentukan direktori profil pengguna melalui `users_dir` di `config.nt` (yang secara otomatis menyertakan semua file `.nt` di direktori ini). Pengguna dan awalan jalur yang diizinkan untuk diunggah menggunakan daftar tidak berurutan (`-`). **Jika awalan jalur unggahan tidak diisi, unggahan ke direktori akar berbagi diperbolehkan secara default**.

1. Buat file konfigurasi `.nt` terpisah untuk setiap anggota di direktori `~/.config/sync0/users/`:

`~/.config/sync0/users/alice.nt` (anggota hanya-baca, tarif terbatas `80MB`, `rate` mendukung `GB`, `MB`, `KB` unit):

```yaml
id: 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c
perm: ro
rate: 80MB
```

`~/.config/sync0/users/bob.nt` (anggota unggahan yang dibatasi: hanya diperbolehkan mengunggah ke direktori `uploads/bob` dan `shared/common`):

```yaml
id: 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f
perm: rw
write_prefix:
  - uploads/bob
  - shared/common
```

`~/.config/sync0/users/charlie.nt` (Unggahan anggota default: Jika `write_prefix` dihilangkan, pengunggahan ke direktori akar share diperbolehkan secara default):

```yaml
id: 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
perm: rw
```

2. Konfigurasikan direktori pengguna dan beberapa grup berbagi file di `~/.config/sync0/config.nt`:

```yaml
users_dir: ./users

groups:
  dataset:
    path: ./data
    auth: false
    perm: ro
    rate: 50MB

  models:
    path: ./models
    auth: false
    perm: ro
    rate: 1GB

  team-vault:
    path: ./vault
    auth: true
    perm: ro
    rate: 512KB
    users:
      - alice
      - bob
      - charlie
```

Mulai berbagi layanan:

```bash
# Startup langsung (secara otomatis memuat semua konfigurasi pengguna .nt di direktori ~/.config/sync0/config.nt dan users_dir)
sync0 share --region SHA

# Atau dengan cepat publikasikan beberapa grup berbagi dan otorisasi anggota grup secara langsung melalui baris perintah
sync0 share dataset:./data team-vault:./vault \
  --auth \
  --allow-ro 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c \
  --allow-rw 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f:uploads/bob,shared/common \
  --allow-rw 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
```

### 3. Tarik semua konten atau pilih beberapa file dan direktori (`sync0 pull`)

Hubungkan ke node rekan mana pun untuk secara otomatis mendapatkan semua node benih yang tersedia di grup berbagi, dan tarik dari beberapa node benih secara bersamaan:

```bash
# Tarik seluruh grup berbagi kumpulan data ke ./local_data
sync0 tarik kumpulan data:// ./local_data -p <alamat simpul>

# Sinkronisasi multi-pilih: secara bersamaan menarik satu file docs/readme.md dan subdirektori aset/ikon, src/core
sync0 tarik kumpulan data://docs/readme.md,assets/icons,src/core ./local_data -p <alamat simpul>
```

### 4. Unggah file atau direktori ke grup berbagi (`sync0 push`)

Anggota dengan izin unggah (`rw` atau `admin`) dapat mengunggah file atau direktori lokal ke grup berbagi jarak jauh:

```bash
# Anggota dengan izin unggah direktori akar: Unggah langsung ke direktori akar bersama
sync0 push team-vault:// ./new_release -p <alamat simpul>

# Hanya anggota yang berwenang untuk awalan tertentu (seperti uploads/bob): unggah ke awalan jalur yang diizinkan
sync0 push team-vault://uploads/bob/report.pdf ./report.pdf -p <alamat simpul>
```

### 5. Berlangganan sinkronisasi dan penyemaian otomatis (`sync0 sub`)

Setelah berlangganan grup berbagi dari node tertentu, server lokal menjadi klien penerima dan server penyemaian baru; ketika file node sumber diperbarui, file tersebut secara otomatis disinkronkan dan menyediakan unduhan terpotong untuk node lain:

```bash
# Berlangganan ke seluruh grup berbagi dan secara otomatis menjadi benih
sync0 sub kumpulan data:// ./local_mirror -p <alamat simpul>

# Hanya banyak pilihan untuk berlangganan file dan subdirektori tertentu dan menyemainya
sync0 sub kumpulan data://models/llama.onnx,configs ./local_mirror -p <alamat simpul>
```

### 6. Sinkronisasi rekan grup multi-mesin (`sync0 group`)

Beberapa mesin bergabung dalam grup berbagi yang sama, satu sama lain secara otomatis menemukan node benih yang tersedia dan mempertahankan sinkronisasi dua arah (mendukung sinkronisasi penuh atau beberapa pilihan file dan direktori):

```bash
# Bergabunglah sepenuhnya dengan grup dan sinkronisasi dua arah
sinkronisasi0 grup tim-docs:// ./docs -p <alamat simpul>

# Hanya direktori wal dan file meta.json di grup sinkronisasi multi-pilih
grup sync0 team-docs://wal,meta.json ./docs -p <alamat simpul>
```

### 7. Transmisi langsung point-to-point antara dua mesin (`send` / `recv`)

Tidak perlu melakukan pra-konfigurasi grup, dan Anda dapat mentransfer sementara file atau direktori secara langsung antara dua mesin:

```bash
# Pemantauan penerima
sync0 recv 0.0.0.0:9000 ./backup

# Pihak pengirim secara langsung mentransfer file atau direktori (opsional --delete mirror untuk membersihkan kelebihan file, --rate speed limit)
sync0 send ./project 192.168.1.10:9000
```

### 8. Peralihan multi-bahasa

`sync0` mendukung 75 bahasa, dan secara otomatis beralih sesuai dengan lingkungan bahasa sistem secara default; itu juga dapat ditentukan secara manual melalui `SYNC0_LANG`:

```bash
SYNC0_LANG=en sync0 --help
SYNC0_LANG=ja sync0 --help
SYNC0_LANG=de sync0 --help
```