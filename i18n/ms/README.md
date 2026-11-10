# sync0

`sync0` ialah perkongsian fail P2P terpencar, penyegerakan masa nyata dan alat pengedaran berbilang sumber yang digunakan untuk menggantikan `rsync` - kelajuan terukur rangkaian kawasan luas rentas benua adalah sehingga **6.95×** daripada `rsync` (titik tunggal geometrik purata kelajuan **2.01×**. kelajuan purata geometri **2.01×**, P23P purata kelajuan ******, P23P. penyegerakan fail kecil yang besar merentasi lautan adalah sepantas **7.19×**.

Setiap mesin adalah kedua-dua pelayan dan pelanggan: tidak ada keperluan untuk pelayan pusat atau alamat rangkaian awam, penebuk lubang automatik adalah berdasarkan alamat kunci awam rangkaian yang diedarkan, dan ia menyokong penghantaran terus antara dua mesin, pengehosan kebenaran kumpulan berbilang perkongsian, pilihan berbilang subdirektori atas permintaan, penyegerakan tambahan masa nyata peringkat milisaat dan penyegerakan berbilang sumber P2P serentak.

## Pasang

### Kaedah 1: Pemasangan kargo

```bash
cargo install sync0
```

### Kaedah 2: Muat turun binari tersusun terlebih dahulu

Pergi ke [GitHub Releases](https://github.com/webc-site/sync0/releases) untuk memuat turun `.tar.xz` pakej termampat yang sepadan dengan sistem (menyokong Linux, macOS, Windows, Android, iOS, FreeBSD), nyahzip dan masukkan ke dalam `PATH`:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Bermula dengan cepat

### 1. Penghantaran langsung konfigurasi sifar antara dua mesin (`send` / `recv`)

Tidak perlu mengkonfigurasi kumpulan perkongsian, memindahkan terus fail atau direktori antara dua mesin pada kelajuan tinggi:

```bash
# Hujung penerima mendengar pada port dan menulis ke ./backup
sync0 recv 0.0.0.0:9000 ./backup

# Penghantaran terus daripada pengirim (pilihan --padam cermin untuk membersihkan lebihan fail, --kadar had laju 50MB)
sync0 send ./project 192.168.1.10:9000
```

### 2. Perkongsian siaran dan tarik/muat naik atas permintaan (`share` / `pull` / `push`)

```bash
# Pelayan: Terbitkan kumpulan perkongsian dengan satu baris arahan (keluarkan alamat nod iroh tanpa IP selepas permulaan)
sync0 share dataset:./data

# Pelanggan: Tarik keseluruhan kumpulan perkongsian set data ke ./local_data setempat
sync0 pull dataset:// ./local_data -p <node address>

# Pelanggan: Tarik berbilang pilihan atas permintaan (hanya menyegerakkan fail tunggal dan subdirektori yang ditentukan)
sync0 pull dataset://docs/readme.md,assets/icons,src/core ./local_data -p <nod address>

# Pelanggan: Muat naik fail atau direktori ke kumpulan perkongsian yang dibenarkan
sync0 push dataset://releases/v1 ./build_output -p <node address>
```

### 3. pembenihan langganan masa nyata dan penyegerakan dua hala berbilang mesin (`sub` / `group`)

Selepas mewujudkan sambungan yang panjang, jika mana-mana fail **menambah, mengubah suai, memadam, menukar kebenaran atau masa pengubahsuaian** berlaku pada hujung jauh, direktori tempatan akan dikemas kini secara serentak dalam milisaat dalam masa nyata (fail besar baharu/diubah suai hanya akan menarik blok perbezaan dan mengemas kininya di tempatnya, memadam fail akan membersihkan fail masa tempatan dan direktori kosong secara automatik, dan hanya menukar keizinan sifar secara automatik):

- **`sync0 sub`(Pencerminan masa nyata sehala + pembenihan automatik)**: Masa nyata tempatan mengikuti penambahan, pemadaman dan pengubahsuaian hujung perkongsian dan berfungsi secara automatik sebagai sumber benih P2P untuk menyediakan pecutan serentak untuk nod penarik yang lain.
- **`sync0 group`(Penyegerakan masa nyata dua hala berbilang mesin)**: Jika fail ditambah, dipadamkan atau diubah suai pada mana-mana mesin dalam kumpulan, semua nod dalam talian lain akan dikemas kini secara serentak dalam masa nyata dan berfungsi sebagai benih untuk satu sama lain.

```bash
# Langgan keseluruhan kumpulan perkongsian: penambahan jauh, pemadaman dan pengubahsuaian disegerakkan secara setempat dalam masa nyata dan disemai secara automatik
sync0 sub set data:// ./local_mirror -p <node address>

# Langganan berbilang pilihan atas permintaan: hanya segerakkan dan semaikan fail tunggal dan subdirektori yang ditentukan dalam masa nyata
sync0 sub set data://models/llama.onnx,configs ./local_mirror -p <alamat nod>

# Kumpulan rakan setara berbilang mesin penyegerakan masa nyata dua hala (juga menyokong laluan berbilang pilih penuh atau koma)
kumpulan sync0 team-docs:// ./docs -p <alamat nod>
```

### 4. Konfigurasi kebenaran kumpulan berbilang perkongsian pasukan dan ahli (`.nt` konfigurasi)

Direktori konfigurasi lalai ialah `~/.config/sync0` (boleh ditukar melalui `-c <directory>`), menggunakan format [NestedText](https://nestedtext.org) (`.nt`) minimalis:
- **Menyokong menyekat pembacaan rangkaian awam (hanya pengguna yang dibenarkan boleh membaca)**: Selagi kumpulan perkongsian dikonfigurasikan dengan senarai ahli `users:` (atau baris arahan menentukan `--allow-r` / `--allow-rw` / `--allow-w` / `--auth` / `--auth`), sistem **b id="5"> dihidupkan secara automatik (`users` adalah awam dan baca sahaja secara lalai (jika anda mahu menjadi umum dan baca sahaja dan terbuka untuk muat naik hanya kepada ahli tertentu, anda boleh menetapkan secara eksplisit `auth: false`).
- **Kebenaran penghantaran baca-tulis dan penghantaran subdirektori yang terperinci**: Ahli adalah baca-sahaja secara lalai (`r` tidak perlu ditulis secara eksplisit), dan menyokong `r` (baca-sahaja), `w` (hanya tulis pada kotak drop, tidak boleh dibaca), <b id="liwrite"> (baca) awalan dibenarkan untuk dimuat naik).

**Contoh konfigurasi ahli (`~/.config/sync0/users/`):**

```yaml
# ~/.config/sync0/users/alice.nt (baca sahaja lalai, had kelajuan 80MB)
id: 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c
rate: 80MB
```

```yaml
# ~/.config/sync0/users/bob.nt (kebenaran baca dan tulis, hanya dibenarkan untuk memuat naik ke subdirektori tertentu)
id: 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f
perm: rw
write_prefix:
  - uploads/bob
  - shared/common
```

```yaml
# ~/.config/sync0/users/charlie.nt (hanya menulis kebenaran penghantaran, membaca dilarang)
id: 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
perm: w
```

**Berkongsi contoh konfigurasi kumpulan (`~/.config/sync0/config.nt`):**

```yaml
users_dir: ./users

groups:
  # Pengguna tidak dikonfigurasikan: rangkaian awam lalai baca sahaja
  dataset:
    path: ./data
    rate: 50MB

  # Pengguna yang dikonfigurasikan: menyekat bacaan rangkaian awam secara automatik (hanya alice dan bob boleh membaca, charlie hanya boleh menghantar dan memuat naik)
  team-vault:
    path: ./vault
    users:
      - alice
      - bob
      - charlie
```

Mulakan perkhidmatan (muat fail konfigurasi atau gunakan terus parameter baris arahan untuk menyekat pembacaan rangkaian awam dan membenarkan ahli):

```bash
# Muatkan ~/.config/sync0/config.nt untuk bermula
sync0 share --region SHA

# Atau terbitkan kumpulan perkongsian peribadi melalui baris arahan tulen (nyatakan --allow-* untuk menyekat bacaan rangkaian awam secara automatik dan hanya pengguna yang dibenarkan boleh mengaksesnya)
sync0 share team-vault:./vault \
  --allow-r 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c \
  --allow-rw 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f:uploads/bob,shared/common \
  --allow-w 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
```

### 5. Alamat nod, kad perniagaan dan berbilang bahasa

```bash
# Lihat alamat nod bebas IP tempatan (kunci dijana secara automatik untuk larian pertama)
sync0 id

# Hasilkan kad perniagaan nod yang ditandatangani sendiri Ed25519 (nama panggilan, kod bandar, e-mel, tapak web, pengenalan)
sync0 profile -u alice --region SHA --email alice@example.com -b "Shanghai node"

# Terdapat 75 bahasa terbina dalam, yang mengikut bahasa sistem secara lalai dan boleh ditukar melalui pembolehubah persekitaran.
SYNC0_LANG=en sync0 --help
```

## Prestasi dan ciri teras

### Pengukuran sebenar WAN merentas benua (Perbandingan `rsync`)

Persekitaran ukuran sebenar: Amerika Syarikat `c1` → Eropah `n1` (RTT 65 ms), set data ialah kernel Linux `linux-7.3.3`:

#### 1) Mod a la carte lwn `rsync`

| Set data | saiz | `sync0` Titik Tunggal<br>(Mbps) | `rsync`<br>(Mbps) | Kelajuan (`sync0` / `rsync`) |
| :--- | ---: | ---: | ---: | ---: |
| `linux-7.3.3.tar.xz`(tidak boleh mampat) | 155 MB | **121.07** | 107.89 | **1.12×** |
| `linux-7.3.3.tar`(boleh mampat) | 1.6 GB | **442.07** | 506.62 | **0.87×** |
| `linux-7.3.3.tar`(kenaikan perubahan setempat) | 1.6 GB | **8,123.87** | 1,169.49 | **6.95×** |
| `linux-7.3.3/` (95,979 fail kecil) | 1.5 GB | **565.49** | 235.27 | **2.40×** |
| **min geometri** | — | **704.17** | 350.19 | **2.01×** |

#### 2) Mod dwi-sumber P2P lwn `rsync`

| Set data | saiz | `sync0` Sumber Dwi P2P<br>(Mbps) | `rsync`<br>(Mbps) | Kelajuan (`sync0` / `rsync`) |
| :--- | ---: | ---: | ---: | ---: |
| `linux-7.3.3.tar.xz`(tidak boleh mampat) | 155 MB | **466.85** | 107.89 | **4.33×** |
| `linux-7.3.3.tar`(boleh mampat) | 1.6 GB | **992.17** | 506.62 | **1.96×** |
| `linux-7.3.3.tar`(kenaikan perubahan setempat) | 1.6 GB | **5,666.84** | 1,169.49 | **4.85×** |
| `linux-7.3.3/` (95,979 fail kecil) | 1.5 GB | **703.30** | 235.27 | **2.99×** |
| **min geometri** | — | **1,165.63** | 350.19 | **3.33×** |

> Nota: Dalam mod dwi-sumber P2P, nod Eropah `s1` bekerjasama untuk pembenihan, dan RTT tulang belakang merentas benua ialah 65 ms.

### Ciri produk teras

- **Penebuk lubang automatik menangani kunci awam**: Tidak memerlukan pelayan pusat dan alamat rangkaian awam, penebuk lubang NAT automatik berdasarkan alamat kunci awam rangkaian yang diedarkan.
- **Penyegerakan masa nyata peringkat milisaat dan kesedaran metadata**: Berdasarkan pemantauan peristiwa fail asli (`FSEvents` / `inotify` / `ReadDirectoryChangesW`), tolakan masa nyata penambahan fail, pemadaman, kebenaran POSIX (`mode`) dan masa pengubahsuaian peringkat milisaat (`mtime`); penghantaran blok data sifar apabila hanya kebenaran atau masa diubah suai.
- **Muat turun dan muat naik serentak gaya BT dan penjadualan perkaitan serantau**: Nod muat turun tidak perlu menunggu sehingga jumlah penuh dimuat turun. Semua selang blok (`Op::Have`) yang telah melepasi semakan cincangan 128-bit segera dikunci dalam kumpulan memori/sementara untuk nod lain ditarik serentak. Bersama-sama dengan pertukaran nod PEX dinamik dan preemption separuh nod perlahan (Pencuri-Kerja), daya pemprosesan kluster dimaksimumkan.
- **Kekebalan nod anti-malicious dan keracunan kelompok**: senarai sumber berwibawa `root_hash` Penguncian konsisten sendiri + pecutan perkakasan mandatori `gxhash128` pengesahan penghadaman sebelum setiap blok diletakkan pada cakera dan dibiji secara luaran; jika blok yang diusik ditemui, pemutus litar peringkat milisaat akan memadamkan nod berniat jahat dan pulih secara automatik daripada sumber yang jujur.
- **Kenaikan in-situ FastCDC dan pengagregatan fail kecil**: Hanya blok pembezaan dipindahkan untuk pengubahsuaian separa fail besar dan diletakkan pada cakera; pembungkusan memori fail kecil besar-besaran dan pemampatan pembezaan awalan menghapuskan overhed perjalanan pergi dan balik fail demi fail.
- **Pemulihan diri selepas bekalan elektrik terputus dan penyambungan semula selepas dimulakan semula**: Format hibrid RDB + AOF seperti Redis (`.s0_subs.bin`) digunakan untuk mengekalkan status pembenihan dan senarai fail, perubahan harian dilampirkan dalam mikrosaat dan ditulis semula secara automatik dan teratur dengan kerap. Selepas pelayan dimulakan semula, perubahan luar talian secara automatik ditolak ke nod pembenihan.

## Seni bina teknikal dan dokumentasi

- **Protokol penyahgandingan dwi satah**:
  - **Kawalan dan permukaan perubahan masa nyata (`iroh` QUIC / TCP, bait pertama `PROTO_VERSION` versi jabat tangan) **: Bertanggungjawab untuk pengesahan kad niaga Ed25519, pengesahan kebenaran, penemuan benih Swarm dan sokongan untuk penstriman perubahan jisim peringkat `u64::MAX` batching dan <b id"<b id5">masa nyata dimampatkan"`mzstd` id="6"> / `FsOp::Remove` / `FsOp::Meta` Tiga primitif).
  - **Permukaan pengedaran data (UDP asli + `compio` I/O tak segerak) **: pengepala padat 20B, maklum balas berkala 18B TACK, kawalan kesesakan hibrid BBRv3/Copa, pemulihan kehilangan paket RACK-TLP, pintasan mampatan Zstd adaptif Shannon blok pertama dan <b id. end.="2"> pengesahan blok end.="2">
- **Ketahui lebih lanjut**:
  - [Laporan pengukuran prestasi dan skrip pengeluaran semula](https://github.com/webc-site/sync0/blob/main/doc/zh/benchmark.md)
  - [Reka bentuk seni bina sistem](https://github.com/webc-site/sync0/blob/main/doc/zh/design_v4.md)
  - [Protokol Kawat Binari dan Spesifikasi Kegigihan](https://github.com/webc-site/sync0/blob/main/doc/zh/proto.md)
  - [Pengesahan identiti, kawalan pihak berkuasa dan pengesahan integriti](https://github.com/webc-site/sync0/blob/main/doc/zh/crypto.md)