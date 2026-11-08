# sync0

`sync0` ialah perkongsian fail peer-to-peer (P2P) dan alat penyegerakan berbilang mesin yang bersaing dengan Resilio Sync.

Setiap mesin ialah pelayan dan pelanggan: mana-mana nod boleh menerbitkan berbilang kumpulan perkongsian fail. Selepas nod lain bergabung, mereka boleh mendapatkan nod benih yang tersedia secara automatik daripada pelayan rakan sebaya dan menariknya secara serentak daripada pelbagai sumber. Selepas penyegerakan selesai, seeds akan dibuat secara automatik untuk lebih banyak nod untuk mempercepatkan muat turun.

## Sorotan Teras

- **Semua orang adalah pelayan dan pelanggan**: Setanding dengan Penyegerakan Resilio, tiada pergantungan pelayan pusat. Mana-mana nod boleh mencipta berbilang kumpulan perkongsian fail, dan juga boleh menyertai kumpulan perkongsian orang lain; nod bertukar secara automatik dan mendapatkan nod benih yang tersedia untuk mencapai penarikan serentak dan penyegerakan masa nyata berbilang mesin.
- **Alamat nod tanpa IP**: Secara lalai, kekunci nod dan alamat `base64url` yang dikodkan `iroh` dijana secara automatik dalam `~/.config/sync0` tanpa perlu mengingati atau mendedahkan IP awam.
- **Penyegerakan berbilang pilih atas permintaan**: Menyokong penyegerakan keseluruhan kumpulan perkongsian, dan juga menyokong penyegerakan atas permintaan dengan memilih berbilang fail atau subdirektori pada masa yang sama.
- **Kebenaran hierarki ahli kumpulan dan kawalan direktori muat naik**: Menyokong kebenaran baca sahaja (`ro`), muat naik baca-tulis (`rw`) dan pengurusan (`admin`), dan boleh menentukan secara individu awalan laluan yang dibenarkan untuk dimuat naik oleh setiap ahli (secara lalai, muat naik ke akar umbi).
- **Penyegerakan berkelajuan penuh dan tambahan merentas rangkaian**: Penghantaran serentak fail besar, pengagregatan fail kecil yang besar dalam ingatan dan hanya blok data disegerakkan apabila fail diubah suai sebahagiannya.

## Perbandingan kelajuan dengan rsync

### Pengukuran sebenar sumber tunggal rangkaian transoceanic (China → Amerika Syarikat, lengah 260 ms)

| Senario penghantaran | Saiz data | model | `sync0` | `rsync -az` | Tingkatkan pengganda |
| :--- | :---: | :---: | :---: | :---: | :---: |
| 500 fail kecil | 1.91 MB | Penyegerakan penuh | 18.84 Mbps | 5.32 Mbps | 3.5x |
| 500 fail kecil | 1.91 MB | Penyegerakan tambahan | 27.15 Mbps | 3.83 Mbps | 7.1x |
| Pakej kod dan arkib (`.tar`) | 10.0 MB | Penyegerakan penuh | 139.70 Mbps | 24.97 Mbps | 5.6x |
| Pakej kod dan arkib (`.tar`) | 10.0 MB | Penyegerakan tambahan | 83.38 Mbps | 26.02 Mbps | 3.2x |
| Fail video (`.mp4`) | 15.0 MB | Penyegerakan penuh | 54.06 Mbps | 27.04 Mbps | 2.0x |
| Fail video (`.mp4`) | 15.0 MB | Penyegerakan tambahan | 216.83 Mbps | 21.51 Mbps | 10.1x |

### Ujian tarik berbilang nod rangkaian transoceanic P2P (China → nod dwi Amerika Syarikat, kelewatan 260 ms)

Perbandingan terukur tarikan serentak berjalur nod benih berbilang sumber:

| Senario penghantaran | Saiz data | pembenihan nod tunggal | Pembenihan serentak P2P dua nod | Tingkatkan pengganda |
| :--- | :---: | :---: | :---: | :---: |
| 500 fail kecil | 1.97 MB | 5.28 Mbps | 7.87 Mbps | 1.5x |
| Pakej kod dan arkib (`.tar`) | 30.0 MB | 56.35 Mbps | 83.17 Mbps | 1.5x |
| Fail video (`.mp4`) | 30.0 MB | 27.00 Mbps | 39.46 Mbps | 1.5x |

### Pengukuran sebenar talian berdedikasi berkelajuan tinggi rentas benua (Eropah → Amerika Utara, kelewatan 86 ms)

| Senario penghantaran | Saiz data | `sync0` | `rsync` | Tingkatkan pengganda |
| :--- | :---: | :---: | :---: | :---: |
| 10,000 fail kecil | 77.6 MB | 688 Mbps | 128 Mbps | 5.4x |
| Pakej kod dan arkib (`.tar`) | 512 MB | 2111 Mbps | 438 Mbps | 4.8x |
| Fail video (`.mp4`) | 512 MB | 556 Mbps | 338 Mbps | 1.6x |
| Penyegerakan tambahan (ubah suai kandungan 1%) | 512 MB | 34133 Mbps | 2214 Mbps | 15.4x |

## Pasang

### Kaedah 1: Pasang melalui Kargo

```bash
cargo install sync0
```

### Kaedah 2: Muat turun binari tersusun terlebih dahulu

Pergi ke [GitHub Releases](https://github.com/webc-site/sync0/releases) untuk memuat turun `.tar.xz` pakej termampat untuk sistem yang sepadan (menyokong Linux, macOS, Windows, Android, FreeBSD, NetBSD), nyahzip dan masukkan ke dalam direktori `PATH`:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Bermula dengan cepat

`sync0` menggunakan `~/.config/sync0` sebagai direktori konfigurasi secara lalai (jika anda perlu menukar ke direktori lain, anda boleh menambah `-c <directory>`).

### 1. Lihat alamat nod tempatan dan tetapan kad perniagaan

```bash
# Lihat alamat nod setempat (kunci dijana secara automatik dalam ~/.config/sync0 buat kali pertama)
sync0 id

# Hasilkan kad perniagaan nod peribadi (menyokong nama pengguna, bandar, e-mel, tapak web dan tandatangan dalam 256 perkataan)
sync0 profile -u alice --region SHA --email alice@example.com --url https://example.com -b "Shanghai node"
```

### 2. Terbitkan berbilang kumpulan perkongsian fail dan tetapkan kebenaran ahli kumpulan (`.nt` Konfigurasi)

`sync0` dikonfigurasikan dalam format [NestedText](https://nestedtext.org) (`.nt`). Setiap pengguna mempunyai fail konfigurasi `.nt` yang berasingan. Anda boleh menentukan direktori profil pengguna melalui `users_dir` dalam `config.nt` (yang secara automatik menyertakan semua `.nt` fail dalam direktori ini). Kedua-dua pengguna dan awalan laluan yang dibenarkan untuk dimuat naik menggunakan senarai tidak tertib (`-`). **Apabila awalan laluan muat naik tidak diisi, muat naik ke direktori akar bahagian dibenarkan secara lalai**.

1. Cipta fail konfigurasi `.nt` berasingan untuk setiap ahli dalam direktori `~/.config/sync0/users/`:

`~/.config/sync0/users/alice.nt` (ahli baca sahaja, kadar terhad `80MB`, `rate` menyokong unit `GB`, `MB`, `KB`):

```yaml
id: 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c
perm: ro
rate: 80MB
```

`~/.config/sync0/users/bob.nt` (ahli muat naik terhad: hanya dibenarkan memuat naik ke direktori `uploads/bob` dan `shared/common`):

```yaml
id: 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f
perm: rw
write_prefix:
  - uploads/bob
  - shared/common
```

`~/.config/sync0/users/charlie.nt` (Muat naik lalai ahli: Apabila `write_prefix` ditinggalkan, muat naik ke direktori akar bahagian dibenarkan secara lalai):

```yaml
id: 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
perm: rw
```

2. Konfigurasikan direktori pengguna dan berbilang kumpulan perkongsian fail dalam `~/.config/sync0/config.nt`:

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

Mulakan perkhidmatan perkongsian:

```bash
# Permulaan langsung (secara automatik memuatkan semua konfigurasi pengguna .nt dalam direktori ~/.config/sync0/config.nt dan users_dir)
sync0 share --region SHA

# Atau terbitkan berbilang kumpulan perkongsian dengan cepat dan beri kebenaran kepada ahli kumpulan secara langsung melalui baris arahan
sync0 share dataset:./data team-vault:./vault \
  --auth \
  --allow-ro 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c \
  --allow-rw 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f:uploads/bob,shared/common \
  --allow-rw 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
```

### 3. Tarik semua kandungan atau pilih berbilang fail dan direktori (`sync0 pull`)

Sambung ke mana-mana nod rakan sebaya untuk mendapatkan semua nod benih yang tersedia secara automatik dalam kumpulan perkongsian, dan tarik daripada berbilang nod benih secara serentak:

```bash
# Tarik keseluruhan kumpulan perkongsian set data ke local ./local_data
sync0 pull dataset:// ./local_data -p <node address>

# Penyegerakan berbilang pilih: serentak tarik fail tunggal docs/readme.md dan aset/ikon subdirektori, src/teras
sync0 pull dataset://docs/readme.md,assets/icons,src/core ./local_data -p <nod address>
```

### 4. Muat naik fail atau direktori ke kumpulan perkongsian (`sync0 push`)

Ahli yang mempunyai kebenaran muat naik (`rw` atau `admin`) boleh memuat naik fail atau direktori setempat ke kumpulan perkongsian jauh:

```bash
# Ahli yang mempunyai kebenaran memuat naik direktori akar: Muat naik terus ke direktori akar yang dikongsi
sync0 push team-vault:// ./new_release -p <alamat nod>

# Hanya ahli yang diberi kuasa untuk awalan tertentu (seperti muat naik/bob): muat naik ke awalan laluan yang dibenarkan
sync0 push team-vault://uploads/bob/report.pdf ./report.pdf -p <alamat nod>
```

### 5. Langgan penyegerakan automatik dan pembenihan (`sync0 sub`)

Selepas melanggan kumpulan perkongsian nod tertentu, pelayan tempatan ialah pelanggan penerima dan pelayan pembenihan baharu; apabila fail nod sumber dikemas kini, ia disegerakkan secara automatik dan menyediakan muat turun potongan untuk nod lain:

```bash
# Langgan keseluruhan kumpulan perkongsian dan buat benih secara automatik
sync0 sub set data:// ./local_mirror -p <node address>

# Hanya berbilang pilih untuk melanggan fail dan subdirektori tertentu dan menyemainya
sync0 sub set data://models/llama.onnx,configs ./local_mirror -p <alamat nod>
```

### 6. Penyegerakan rakan sebaya kumpulan berbilang mesin (`sync0 group`)

Berbilang mesin menyertai kumpulan perkongsian yang sama, satu sama lain secara automatik menemui nod benih yang tersedia dan mengekalkan penyegerakan dua hala (menyokong penyegerakan penuh atau berbilang pilihan fail dan direktori):

```bash
# Sertai sepenuhnya kumpulan dan penyegerakan dua hala
kumpulan sync0 team-docs:// ./docs -p <alamat nod>

# Hanya direktori wal dan fail meta.json dalam kumpulan penyegerakan berbilang pilih
sync0 group team-docs://wal,meta.json ./docs -p <node address>
```

### 7. Penghantaran terus titik ke titik antara dua mesin (`send` / `recv`)

Tidak perlu prakonfigurasi kumpulan, dan anda boleh memindahkan fail atau direktori buat sementara waktu antara dua mesin:

```bash
# Pemantauan penerima
sync0 recv 0.0.0.0:9000 ./backup

# Penghujung penghantaran terus memindahkan fail atau direktori (pilihan --padam cermin untuk membersihkan lebihan fail, --kadar had laju)
sync0 send ./project 192.168.1.10:9000
```

### 8. Penukaran berbilang bahasa

`sync0` menyokong 75 bahasa dan bertukar secara automatik mengikut persekitaran bahasa sistem secara lalai; ia juga boleh ditentukan secara manual melalui `SYNC0_LANG`:

```bash
SYNC0_LANG=en sync0 --help
SYNC0_LANG=ja sync0 --help
SYNC0_LANG=de sync0 --help
```