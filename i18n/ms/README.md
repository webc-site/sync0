# sync0

`sync0` ialah alat baris arahan untuk memindahkan fail merentasi rangkaian, menyegerakkan direktori secara berperingkat dan mengedarkan data secara serentak kepada berbilang mesin.

## Niat reka bentuk asal

Dalam bilik merentas mesin, negara atau persekitaran dengan kelewatan rangkaian dan kehilangan paket, alatan seperti `rsync` dan `scp` adalah berdasarkan pengesahan jujukan sambungan tunggal, yang mengehadkan penggunaan lebar jalur; apabila berbilang mesin menarik data dari pelayan yang sama pada masa yang sama, kad rangkaian pelayan sumber dengan mudah boleh menjadi halangan.

Matlamat `sync0` adalah untuk menyelesaikan dua masalah:

1. Biarkan fail dan penyegerakan direktori antara kedua-dua mesin menjalankan lebar jalur fizikal penuh, dan hanya pindahkan bahagian yang diubah apabila fail diubah suai sebahagiannya.
2. Berbilang mesin boleh memindahkan blok data antara satu sama lain apabila menyegerakkan direktori yang sama, mengurangkan beban pada pelayan sumber dan memendekkan masa penyegerakan keseluruhan.

## Perbandingan kelajuan dengan rsync

Persekitaran ujian: Nod Eropah $\leftrightarrow$ Nod Amerika Utara, kelewatan pergi balik 86 ms, mengukur masa yang memakan masa yang lengkap termasuk pengimbasan direktori, perbandingan penyahduplikasi, penghantaran rangkaian dan pengesahan peletakan cakera:

| Senario penghantaran | Saiz data | `sync0` | `rsync` | berbilang |
| :--- | :---: | :---: | :---: | :---: |
| 10,000 fail kecil | 77.6 MB | 0.90 s (688 Mbps) | 4.86 s (128 Mbps) | 5.4x |
| Pakej kod dan arkib (`.tar`) | 512 MB | 1.94 s (2111 Mbps) | 9.36 s (438 Mbps) | 4.8x |
| Fail video (`.mp4`) | 512 MB | 7.36 s (556 Mbps) | 12.11 s (338 Mbps) | 1.6x |
| Penyegerakan tambahan (ubah suai kandungan 1%) | 512 MB | 0.12 s | 1.85 s | 15.4x |

## Kaedah peningkatan prestasi

- **Penghantaran serentak UDP**: Hantar blok data melalui UDP, dan hanya hantar semula serpihan yang hilang untuk mengelakkan satu kehilangan paket menyekat pemindahan fail berikutnya.
- **Pembahagian kandungan dan penyahduplikasian**: Pisahkan fail kepada blok data mengikut kandungan dan hitung nilai semakan. Kandungan yang sama yang sudah ada di bahagian penerima digunakan semula secara langsung secara tempatan tanpa menduplikasi rangkaian.
- **Laraskan pemampatan secara automatik mengikut kelajuan rangkaian**: Hidupkan pemampatan untuk kandungan boleh mampat seperti teks dan kod; melangkau pemampatan secara automatik untuk video, gambar, pakej termampat atau di bawah LAN berkelajuan tinggi untuk mengurangkan penggunaan CPU.
- **Penarikan berdekatan berbilang nod**: Apabila berbilang mesin menarik direktori yang sama, blok data akan ditarik serentak daripada nod yang dimuat turun di bandar yang sama atau dengan kependaman yang lebih rendah.

## Pasang

### Kaedah 1: Pasang melalui Kargo

```bash
cargo install sync0
```

### Kaedah 2: Muat turun binari tersusun terlebih dahulu

Pergi ke [GitHub Releases](https://github.com/webc-site/sync0/releases) untuk memuat turun `.tar.xz` pakej termampat untuk sistem yang sepadan (menyokong Linux x86_64/ARM64, macOS Apple Silicon/Intel, Windows x64/ARM64, Android ARM64/x86_64, FreeBSD, NetBSD). Selepas menyahzip, letakkan `sync0` ke dalam direktori `PATH`:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Panduan Pengguna

### 1. Pindahkan terus fail atau direktori antara dua mesin

Mulakan pemantauan pada mesin penerima dan nyatakan laluan penerimaan dan simpanan:

```bash
sync0 recv 0.0.0.0:9000 ./backup
```

Hantar satu fail atau keseluruhan direktori daripada mesin penghantar:

```bash
sync0 send ./project 192.168.1.10:9000
```

Parameter yang biasa digunakan:

```bash
# Padamkan fail berlebihan yang wujud pada bahagian penerima tetapi bukan pada hujung penghantaran (cermin penyegerakan)
sync0 send ./project 192.168.1.10:9000 --delete

# Hadkan kadar penghantaran (unit: bait/saat, contohnya 50 MB/s)
sync0 send ./project 192.168.1.10:9000 --rate 50000000

# Mod senyap (tiada bar kemajuan ditunjukkan)
sync0 send ./project 192.168.1.10:9000 -q
```

### 2. Menerbitkan direktori kongsi dan menarik daripada berbilang nod

Terbitkan direktori kongsi (bernama `dataset`) pada pelayan asal:

```bash
sync0 share dataset ./data --region SHA
```

Tarik direktori kongsi pada mesin lain (anda boleh mengisi berbilang alamat nod pada masa yang sama, dipisahkan dengan koma untuk ditarik serentak):

```bash
sync0 pull 10.0.0.1:9000,10.0.0.2:9000 dataset ./local_data
```

Tarik hanya subdirektori tertentu dalam direktori kongsi:

```bash
sync0 pull 10.0.0.1:9000 dataset ./local_data --subpath 2026/logs
```

Tolak perubahan setempat kembali ke direktori kongsi jauh:

```bash
sync0 push 10.0.0.1:9000 dataset ./local_data
```

### 3. Langgan kemas kini automatik katalog

Langgan direktori kongsi nod sumber. Apabila fail ditambah atau diubah suai dalam direktori sumber, ia akan disegerakkan secara automatik ke komputer tempatan. Pada masa yang sama, mesin tempatan secara automatik akan berfungsi sebagai nod untuk ditarik oleh mesin lain:

```bash
sync0 sub 10.0.0.1:9000 dataset ./local_mirror
```

### 4. Penyegerakan dua hala kumpulan berbilang mesin

Berbilang mesin menyertai nama kumpulan yang sama, dan perubahan dalam mana-mana direktori nod akan disegerakkan secara automatik ke nod lain dalam kumpulan:

```bash
sync0 group team-docs ./docs 10.0.0.1:9000,10.0.0.2:9000
```

### 5. Identiti nod dan kad perniagaan peribadi

Setiap nod menyokong penetapan nama pengguna, bandar, e-mel, tapak web dan tandatangan diperibadikan (maksimum 256 aksara), dan lulus pengesahan tandatangan kunci persendirian tempatan:

```bash
# Lihat ID kunci awam tempatan (nyatakan direktori konfigurasi untuk mengekalkan kunci)
sync0 id -c ~/.config/sync0

# Jana dan paparkan kad perniagaan nod dengan tandatangan yang diperibadikan
sync0 profile -c ~/.config/sync0 \
  -u alice \
  --region SHA \
  --email alice@example.com \
  --url https://example.com \
  -b "Shanghai backup node"
```

`share`, `pull`, `sub`, `group` juga boleh dilancarkan dengan parameter profil ini:

```bash
sync0 share dataset ./data -c ~/.config/sync0 -u alice --region SHA -b "Shanghai backup node"
```

### 6. Penukaran berbilang bahasa

`sync0` Menyokong 75 bahasa. Secara lalai, bantuan baris arahan dan gesaan dalam bahasa yang sepadan dipaparkan secara automatik mengikut persekitaran bahasa sistem pengendalian; jika bahasa sistem tiada dalam senarai yang disokong, bahasa Inggeris digunakan secara lalai.

Untuk menentukan bahasa secara manual, tetapkan pembolehubah persekitaran `SYNC0_LANG`:

```bash
SYNC0_LANG=en sync0 --help
SYNC0_LANG=ja sync0 --help
SYNC0_LANG=de sync0 --help
```