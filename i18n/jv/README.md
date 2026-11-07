# sync0

`sync0` minangka alat baris perintah kanggo nransfer file ing jaringan, nyinkronake direktori kanthi bertahap, lan nyebarake data kanthi bebarengan menyang macem-macem mesin.

## niat desain asli

Ing kamar lintas-mesin, negara, utawa lingkungan sing tundha jaringan lan mundhut paket, piranti kayata `rsync` lan `scp` adhedhasar konfirmasi urutan sambungan siji, sing mbatesi panggunaan bandwidth; nalika sawetara mesin narik data saka server padha ing wektu sing padha, kertu jaringan server sumber bisa gampang dadi bottleneck.

Tujuan saka `sync0` yaiku kanggo ngrampungake rong masalah:

1. Ayo file lan direktori sinkronisasi antarane loro mesin mbukak bandwidth fisik lengkap, lan mung nransfer bagean diganti nalika file sebagian diowahi.
2. Multiple mesin bisa nransfer pamblokiran data kanggo saben liyane nalika nyinkronake direktori padha, ngurangi beban ing server sumber lan shortening wektu sinkronisasi sakabèhé.

## Perbandingan kacepetan karo rsync

Lingkungan tes: Node Eropa $\leftrightarrow$ Node Amerika Utara, wektu tundha round-trip 86 ms, ngukur wektu lengkap kalebu pemindaian direktori, perbandingan deduplikasi, transmisi jaringan lan verifikasi penempatan disk:

| Skenario transmisi | Ukuran data | `sync0` | `rsync` | pirang-pirang |
| :--- | :---: | :---: | :---: | :---: |
| 10.000 file cilik | 77.6 MB | 0.90 s (688 Mbps) | 4.86 s (128 Mbps) | 5.4x |
| Kode lan paket arsip (`.tar`) | 512 MB | 1.94 s (2111 Mbps) | 9.36 s (438 Mbps) | 4.8x |
| File video (`.mp4`) | 512 MB | 7.36 s (556 Mbps) | 12.11 s (338 Mbps) | 1.6x |
| Sinkronisasi tambahan (ngowahi isi 1%) | 512 MB | 0.12 s | 1.85 s | 15.4x |

## Cara dandan kinerja

- **Transmisi bebarengan UDP**: Ngirim pamblokiran data liwat UDP, lan mung ngirim maneh fragmen sing ilang kanggo nyegah mundhut paket siji sing ngalangi transfer file sakteruse.
- **Segmentasi lan deduplikasi konten**: Pisah file dadi blok data miturut isi lan etung nilai mriksa. Isi sing padha wis ana ing mburi panrima digunakake maneh langsung sacara lokal tanpa duplikat jaringan.
- **Ngatur kompresi kanthi otomatis miturut kacepetan jaringan**: Aktifake kompresi kanggo konten sing bisa dikompres kayata teks lan kode; otomatis skip komprèsi kanggo video, gambar, paket compressed utawa ing LAN kacepetan dhuwur kanggo nyuda konsumsi CPU.
- ** Multi-node cedhak narik **: Nalika sawetara mesin narik direktori padha, pamblokiran data bakal ditarik bebarengan saka simpul diundhuh ing kutha padha utawa karo latensi ngisor.

## Instal

### Cara 1: Instal liwat Cargo

```bash
cargo install sync0
```

### Cara 2: Ngundhuh binari sing wis dikompilasi

Bukak [Rilis GitHub](https://github.com/webc-site/sync0/releases) kanggo ngundhuh paket kompres `.tar.xz` kanggo sistem sing cocog (ndhukung Linux x86_64/ARM64, macOS Apple Silicon/Intel, Windows x64/ARM64, Android ARM64/x86_64, FreeBSD, NetBSD). Sawise mbukak zip, selehake `sync0` menyang direktori `PATH`:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Pandhuan pangguna

### 1. Langsung nransfer file utawa direktori ing antarane rong mesin

Miwiti ngawasi ing mesin panampa lan nemtokake dalan nampa lan nyimpen:

```bash
sync0 recv 0.0.0.0:9000 ./backup
```

Kirimi file siji utawa kabeh direktori saka mesin pangirim:

```bash
sync0 send ./project 192.168.1.10:9000
```

Parameter sing umum digunakake:

```bash
# Mbusak file keluwih sing ana ing mburi panampa nanging ora ing mburi ngirim (sinkronisasi pangilon)
sync0 send ./project 192.168.1.10:9000 --delete

# Batasi tingkat kirim (unit: byte/detik, contone 50 MB/s)
sync0 send ./project 192.168.1.10:9000 --rate 50000000

# Mode bisu (ora ditampilake garis kemajuan)
sync0 send ./project 192.168.1.10:9000 -q
```

### 2. Nerbitake direktori sing dienggo bareng lan narik saka macem-macem simpul

Nerbitake direktori sing dienggo bareng (jenenge `dataset`) ing server asal:

```bash
sync0 share dataset ./data --region SHA
```

Narik direktori sing dienggo bareng ing mesin liyane (sampeyan bisa ngisi pirang-pirang alamat simpul bebarengan, dipisahake karo koma kanggo narik bebarengan):

```bash
sync0 pull 10.0.0.1:9000,10.0.0.2:9000 dataset ./local_data
```

Tarik mung subdirektori tartamtu ing direktori sing dienggo bareng:

```bash
sync0 pull 10.0.0.1:9000 dataset ./local_data --subpath 2026/logs
```

Push owah-owahan lokal bali menyang direktori sing dienggo bareng remot:

```bash
sync0 push 10.0.0.1:9000 dataset ./local_data
```

### 3. Langganan kanggo nganyari otomatis katalog

Langganan direktori sing dienggo bareng saka simpul sumber. Nalika file ditambahake utawa diowahi ing direktori sumber, bakal kanthi otomatis disinkronake menyang komputer lokal. Ing wektu sing padha, mesin lokal bakal kanthi otomatis dadi simpul kanggo mesin liyane kanggo narik:

```bash
sync0 sub 10.0.0.1:9000 dataset ./local_mirror
```

### 4. Sinkronisasi rong arah saka klompok multi-mesin

Akeh mesin gabung karo jeneng grup sing padha, lan owah-owahan ing sembarang direktori simpul bakal disinkronake kanthi otomatis menyang simpul liyane ing grup kasebut:

```bash
sync0 group team-docs ./docs 10.0.0.1:9000,10.0.0.2:9000
```

### 5. Identitas simpul lan kertu bisnis pribadi

Saben simpul ndhukung nyetel jeneng pangguna, kutha, email, situs web lan teken pribadi (maksimal 256 karakter), lan ngliwati otentikasi tandha kunci pribadi lokal:

```bash
# Deleng ID kunci umum lokal (nemtokake direktori konfigurasi kanggo njaga kunci kasebut)
sync0 id -c ~/.config/sync0

# Gawe lan nampilake kertu bisnis simpul kanthi teken pribadi
sync0 profile -c ~/.config/sync0 \
  -u alice \
  --region SHA \
  --email alice@example.com \
  --url https://example.com \
  -b "Shanghai backup node"
```

`share`, `pull`, `sub`, `group` uga bisa diluncurake nganggo paramèter profil iki:

```bash
sync0 share dataset ./data -c ~/.config/sync0 -u alice --region SHA -b "Shanghai backup node"
```

### 6. Ngalih multi-basa

`sync0` Ndhukung 75 basa. Kanthi gawan, bantuan baris perintah lan pituduh ing basa sing cocog ditampilake kanthi otomatis miturut lingkungan basa sistem operasi; yen basa sistem ora ana ing dhaptar sing didhukung, Inggris digunakake minangka standar.

Kanggo nemtokake basa kanthi manual, setel variabel lingkungan `SYNC0_LANG`:

```bash
SYNC0_LANG=en sync0 --help
SYNC0_LANG=ja sync0 --help
SYNC0_LANG=de sync0 --help
```