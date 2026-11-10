# sync0

`sync0` adalah alat berbagi file P2P terdesentralisasi, sinkronisasi real-time, dan distribusi multi-sumber yang digunakan untuk menggantikan `rsync` - kecepatan terukur jaringan area luas lintas benua mencapai **6,95×** dari `rsync` (kecepatan rata-rata geometrik titik tunggal **2,01×**, kecepatan rata-rata geometrik sumber ganda P2P **3,33×**), dan sinkronisasi file kecil berukuran besar melintasi lautan secepat **7.19×**.

Setiap mesin adalah server dan klien: tidak memerlukan server pusat atau alamat jaringan publik, pelubangan otomatis didasarkan pada alamat kunci publik jaringan terdistribusi, dan mendukung transmisi langsung antara dua mesin, hosting izin grup multi-berbagi, beberapa pilihan subdirektori sesuai permintaan, sinkronisasi inkremental waktu nyata tingkat milidetik, dan akselerasi bersamaan multi-sumber P2P.

## Memasang

### Metode 1: Pemasangan kargo

```bash
cargo install sync0
```

### Metode 2: Unduh biner yang telah dikompilasi sebelumnya

Buka [Rilis GitHub](https://github.com/webc-site/sync0/releases) untuk mengunduh `.tar.xz` paket terkompresi yang sesuai dengan sistem (mendukung Linux, macOS, Windows, Android, iOS, FreeBSD), unzip dan masukkan ke dalam `PATH`:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Mulailah dengan cepat

### 1. Transmisi langsung tanpa konfigurasi antara dua mesin (`send` / `recv`)

Tidak perlu mengkonfigurasi grup berbagi, langsung mentransfer file atau direktori antara dua mesin dengan kecepatan tinggi:

```bash
# Pihak penerima mendengarkan port dan menulis ke ./backup
sync0 recv 0.0.0.0:9000 ./backup

# Transmisi langsung dari pengirim (opsional --delete mirror untuk membersihkan file berlebih, --rate batas kecepatan 50MB)
sync0 send ./project 192.168.1.10:9000
```

### 2. Berbagi postingan dan tarik/unggah sesuai permintaan (`share` / `pull` / `push`)

```bash
# Server: Publikasikan grup berbagi dengan satu baris perintah (keluarkan alamat node iroh bebas IP setelah startup)
sync0 share dataset:./data

# Klien: Tarik seluruh grup berbagi kumpulan data ke ./local_data
sync0 tarik kumpulan data:// ./local_data -p <alamat simpul>

# Klien: Tarik beberapa pilihan sesuai permintaan (hanya menyinkronkan satu file dan subdirektori tertentu)
sync0 tarik kumpulan data://docs/readme.md,assets/icons,src/core ./local_data -p <alamat simpul>

# Klien: Unggah file atau direktori ke grup berbagi resmi
sync0 push dataset://releases/v1 ./build_output -p <alamat node>
```

### 3. Penyemaian langganan real-time dan sinkronisasi dua arah multi-mesin (`sub` / `group`)

Setelah membuat koneksi yang panjang, jika ada file **menambah, memodifikasi, menghapus, mengubah izin atau waktu modifikasi** terjadi di ujung jarak jauh, direktori lokal akan diperbarui secara sinkron dalam milidetik secara real-time (file besar baru/dimodifikasi hanya akan menarik blok perbedaan dan memperbaruinya di tempatnya, menghapus file akan secara otomatis membersihkan file lokal dan direktori kosong, dan hanya mengubah izin atau stempel waktu nol blok data secara langsung):

- **`sync0 sub`(Pencerminan real-time satu arah + penyemaian otomatis)**: Real-time lokal mengikuti penambahan, penghapusan, dan modifikasi ujung berbagi, dan secara otomatis berfungsi sebagai sumber benih P2P untuk memberikan akselerasi bersamaan untuk node penarik lainnya.
- **`sync0 group`(Sinkronisasi real-time dua arah multi-mesin peer-to-peer)**: Jika file ditambahkan, dihapus, atau diubah pada mesin apa pun dalam grup, semua node online lainnya akan diperbarui secara serentak secara real-time dan berfungsi sebagai benih untuk satu sama lain.

```bash
# Berlangganan ke seluruh grup berbagi: penambahan, penghapusan, dan modifikasi jarak jauh disinkronkan secara lokal secara real-time dan diunggulkan secara otomatis
sync0 sub kumpulan data:// ./local_mirror -p <alamat simpul>

# Langganan multi-pilih sesuai permintaan: hanya menyinkronkan dan menyemai file dan subdirektori tertentu secara real time
sync0 sub kumpulan data://models/llama.onnx,configs ./local_mirror -p <alamat simpul>

# Sinkronisasi real-time dua arah grup rekan multi-mesin (juga mendukung jalur multi-pilihan penuh atau koma)
sinkronisasi0 grup tim-docs:// ./docs -p <alamat simpul>
```

### 4. Konfigurasi izin anggota dan grup multi-berbagi tim (konfigurasi `.nt`)

Direktori konfigurasi default adalah `~/.config/sync0` (dapat dialihkan melalui `-c <directory>`), menggunakan format minimalis [NestedText](https://nestedtext.org) (`.nt`):
- **Mendukung pembatasan pembacaan jaringan publik (hanya pengguna resmi yang dapat membaca)**: Selama grup berbagi dikonfigurasi dengan daftar anggota `users:` (atau baris perintah menentukan `--allow-r` / `--allow-rw` / `--allow-w` / `--auth`), sistem **secara otomatis mengaktifkan autentikasi pribadi (`auth: true`)** dan melarang penghitungan atau pembacaan simpul anonim jaringan publik, dan Berlangganan P2P ke node penyemaian juga akan mewarisi daftar putih secara bersamaan, sehingga hanya pengguna yang berwenang yang dapat membaca; grup berbagi yang tidak dikonfigurasi dengan `users` bersifat publik dan hanya-baca secara default (jika Anda ingin menjadi publik dan hanya-baca dan terbuka untuk unggahan hanya kepada anggota tertentu, Anda dapat menyetel `auth: false` secara eksplisit).
- **Izin baca-tulis dan pengiriman subdirektori yang terperinci**: Anggota bersifat hanya baca secara default (`r` tidak perlu ditulis secara eksplisit), dan mendukung `r` (hanya baca), `w` (hanya menulis ke drop box, tidak dapat dibaca), `rw` (baca-tulis) dan `write_prefix` (membatasi awalan subdirektori yang diizinkan untuk diunggah).

**Contoh konfigurasi anggota (`~/.config/sync0/users/`):**

```yaml
# ~/.config/sync0/users/alice.nt (hanya baca default, batas kecepatan 80MB)
id: 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c
rate: 80MB
```

```yaml
# ~/.config/sync0/users/bob.nt (izin baca dan tulis, hanya diperbolehkan mengunggah ke subdirektori tertentu)
id: 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f
perm: rw
write_prefix:
  - uploads/bob
  - shared/common
```

```yaml
# ~/.config/sync0/users/charlie.nt (hanya izin pengiriman tulis, dilarang membaca)
id: 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
perm: w
```

**Contoh konfigurasi grup berbagi (`~/.config/sync0/config.nt`):**

```yaml
users_dir: ./users

groups:
  # Pengguna tidak dikonfigurasi: jaringan publik default hanya dapat dibaca
  dataset:
    path: ./data
    rate: 50MB

  # Pengguna yang dikonfigurasi: secara otomatis membatasi pembacaan jaringan publik (hanya alice dan bob yang dapat membaca, charlie hanya dapat mengirim dan mengunggah)
  team-vault:
    path: ./vault
    users:
      - alice
      - bob
      - charlie
```

Mulai layanan (muat file konfigurasi atau langsung gunakan parameter baris perintah untuk membatasi pembacaan jaringan publik dan memberi otorisasi kepada anggota):

```bash
# Muat ~/.config/sync0/config.nt untuk memulai
sync0 share --region SHA

# Atau publikasikan grup berbagi pribadi melalui baris perintah murni (tentukan --allow-* untuk secara otomatis membatasi pembacaan jaringan publik dan hanya pengguna resmi yang dapat mengaksesnya)
sync0 share team-vault:./vault \
  --allow-r 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c \
  --allow-rw 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f:uploads/bob,shared/common \
  --allow-w 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
```

### 5. Alamat node, kartu nama dan multi-bahasa

```bash
# Lihat alamat node lokal bebas IP (kunci dibuat secara otomatis saat pertama kali dijalankan)
sync0 id

# Hasilkan kartu nama simpul yang ditandatangani sendiri Ed25519 (nama panggilan, kode kota, email, situs web, perkenalan)
sync0 profile -u alice --region SHA --email alice@example.com -b "Shanghai node"

# Terdapat 75 bahasa bawaan, yang mengikuti bahasa sistem secara default dan dapat dialihkan melalui variabel lingkungan.
SYNC0_LANG=en sync0 --help
```

## Performa dan fitur inti

### Pengukuran aktual WAN lintas benua (Perbandingan `rsync`)

Lingkungan pengukuran aktual: Amerika Serikat `c1` → Eropa `n1` (RTT 65 ms), kumpulan datanya adalah kernel Linux `linux-7.3.3`:

#### 1) Mode a la carte vs `rsync`

| Kumpulan data | ukuran | `sync0` Titik Tunggal<br>(Mbps) | `rsync`<br>(Mbps) | Percepatan (`sync0` / `rsync`) |
| :--- | ---: | ---: | ---: | ---: |
| `linux-7.3.3.tar.xz`(tidak dapat dikompresi) | 155 MB | **121.07** | 107.89 | **1,12×** |
| `linux-7.3.3.tar`(dapat dikompresi) | 1.6 GB | **442.07** | 506.62 | **0,87×** |
| `linux-7.3.3.tar`(peningkatan perubahan lokal) | 1.6 GB | **8,123.87** | 1,169.49 | **6,95×** |
| `linux-7.3.3/` (95.979 file kecil) | 1.5 GB | **565.49** | 235.27 | **2,40×** |
| **rata-rata geometri** | — | **704.17** | 350.19 | **2.01×** |

#### 2) Mode sumber ganda P2P vs `rsync`

| Kumpulan data | ukuran | `sync0` Sumber Ganda P2P<br>(Mbps) | `rsync`<br>(Mbps) | Percepatan (`sync0` / `rsync`) |
| :--- | ---: | ---: | ---: | ---: |
| `linux-7.3.3.tar.xz`(tidak dapat dikompresi) | 155 MB | **466.85** | 107.89 | **4,33×** |
| `linux-7.3.3.tar`(dapat dikompresi) | 1.6 GB | **992.17** | 506.62 | **1,96×** |
| `linux-7.3.3.tar`(peningkatan perubahan lokal) | 1.6 GB | **5,666.84** | 1,169.49 | **4,85×** |
| `linux-7.3.3/` (95.979 file kecil) | 1.5 GB | **703.30** | 235.27 | **2,99×** |
| **rata-rata geometri** | — | **1,165.63** | 350.19 | **3,33×** |

> Catatan: Dalam mode sumber ganda P2P, node Eropa `s1` berkolaborasi untuk melakukan seeding, dan RTT backbone lintas benua adalah 65 ms.

### Fitur produk inti

- **Kunci publik mengatasi pelubangan otomatis**: Tidak memerlukan server pusat dan alamat jaringan publik, pelubangan NAT otomatis berdasarkan alamat kunci publik jaringan terdistribusi.
- **Sinkronisasi real-time tingkat milidetik dan kesadaran metadata**: Berdasarkan pemantauan kejadian file asli (`FSEvents` / `inotify` / `ReadDirectoryChangesW`), dorongan real-time untuk penambahan, penghapusan, izin POSIX (`mode`) dan waktu modifikasi tingkat milidetik (`mtime`); nol transmisi blok data ketika hanya izin atau waktu yang diubah.
- **Pengunduhan dan pengunggahan simultan gaya BT serta penjadwalan afinitas regional**: Node pengunduhan tidak perlu menunggu hingga jumlah penuh diunduh. Semua interval blok (`Op::Have`) yang telah melewati pemeriksaan hash 128-bit segera dikunci dalam memori/kumpulan sementara agar node lain dapat menariknya secara bersamaan. Bersama dengan pertukaran node PEX yang dinamis dan setengah preemption node yang lambat (Work-Stealing), throughput cluster menjadi maksimal.
- **Perubahan node anti-berbahaya dan imunitas peracunan klaster**: daftar sumber resmi `root_hash` Penguncian yang konsisten + akselerasi perangkat keras wajib `gxhash128` verifikasi intisari sebelum setiap blok ditempatkan pada disk dan diunggulkan secara eksternal; jika blok yang dirusak ditemukan, pemutus sirkuit tingkat milidetik akan mematikan node berbahaya dan secara otomatis memulihkan dari sumber yang jujur.
- **Peningkatan in-situ FastCDC dan agregasi file kecil**: Hanya blok diferensial yang ditransfer untuk modifikasi sebagian file besar dan ditempatkan pada disk; pengemasan memori file kecil yang besar dan kompresi diferensial awalan menghilangkan overhead bolak-balik file demi file.
- **Penyembuhan mandiri setelah listrik padam dan dimulainya kembali setelah restart**: Format hybrid RDB + AOF seperti Redis (`.s0_subs.bin`) digunakan untuk mempertahankan status penyemaian dan daftar file, perubahan harian ditambahkan dalam mikrodetik, dan secara otomatis ditulis ulang dan diatur secara teratur. Setelah server dimulai ulang, perubahan offline secara otomatis dikirim ke node penyemaian.

## Arsitektur dan dokumentasi teknis

- **Protokol pemisahan bidang ganda**:
  - **Kontrol dan permukaan perubahan waktu nyata (`iroh` QUIC / TCP, jabat tangan versi `PROTO_VERSION` byte pertama) **: Bertanggung jawab atas verifikasi kartu nama Ed25519, verifikasi izin, penemuan benih Swarm, dan dukungan untuk batching streaming perubahan massal level `mzstd` dan kompresi `SubPush` dorongan waktu nyata (`FsOp::Upsert` / `FsOp::Remove` / `FsOp::Meta` Tiga primitif).
  - **Permukaan distribusi data (UDP asli + `compio` I/O asinkron) **: header ringkas 20B, umpan balik periodik TACK 18B, kontrol kemacetan hibrid BBRv3/Copa, pemulihan kehilangan paket RACK-TLP, bypass kompresi Zstd adaptif entropi Shannon blok pertama, dan `gxhash128` verifikasi blok ujung ke ujung.
- **Pelajari lebih lanjut**:
  - [Laporan pengukuran kinerja dan skrip reproduksi](https://github.com/webc-site/sync0/blob/main/doc/zh/benchmark.md)
  - [Desain arsitektur sistem](https://github.com/webc-site/sync0/blob/main/doc/zh/design_v4.md)
  - [Protokol Kawat Biner dan Spesifikasi Persistensi](https://github.com/webc-site/sync0/blob/main/doc/zh/proto.md)
  - [Otentikasi identitas, kontrol otoritas dan verifikasi integritas](https://github.com/webc-site/sync0/blob/main/doc/zh/crypto.md)