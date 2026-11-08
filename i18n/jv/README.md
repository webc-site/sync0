# sync0

`sync0` minangka alat enggo bareng file peer-to-peer (P2P) lan alat sinkronisasi multi-mesin sing saingan karo Resilio Sync.

Saben mesin minangka server lan klien: simpul apa wae bisa nerbitake pirang-pirang grup enggo bareng file. Sawise simpul liyane gabung, bisa kanthi otomatis entuk simpul wiji sing kasedhiya saka server peer lan narik bebarengan saka macem-macem sumber. Sawise sinkronisasi rampung, wiji bakal digawe kanthi otomatis kanggo luwih akeh node kanggo nyepetake download.

## Inti Highlights

- **Saben uwong iku server lan klien**: Dibandhingake karo Resilio Sync, ora ana ketergantungan server pusat. Sembarang simpul bisa nggawe macem-macem grup enggo bareng file, lan uga bisa gabung karo grup enggo bareng wong liya; simpul kanthi otomatis ijol-ijolan lan entuk simpul wiji sing kasedhiya kanggo entuk narik bebarengan lan sinkronisasi wektu nyata saka pirang-pirang mesin.
- **Alamat simpul bebas IP**: Kanthi gawan, tombol simpul lan alamat `base64url` sing dikodekan `iroh` digawe kanthi otomatis ing `~/.config/sync0` tanpa perlu ngelingi utawa mbukak IP umum.
- **Multi-pilih sinkronisasi on demand**: Ndhukung sinkronisasi kabeh klompok enggo bareng, lan uga ndhukung sinkronisasi ing dikarepake kanthi milih sawetara file utawa subdirektori ing wektu sing padha.
- **Izin hierarkis anggota grup lan kontrol direktori upload**: Ndhukung ijin mung diwaca (`ro`), upload diwaca-tulis (`rw`), lan ijin manajemen (`admin`), lan bisa nemtokake awalan path sing diijini saben anggota diunggah (kanthi standar, upload direktori menyang root).
- **Sinkronisasi kacepetan lengkap lan inkremental ing jaringan**: Transmisi file gedhe bebarengan, kumpulan file cilik sing gedhe banget ing memori, lan mung pamblokiran data sing diselarasake nalika file diowahi sebagian.

## Perbandingan kacepetan karo rsync

### Pengukuran aktual sumber tunggal jaringan transoceanic (China → Amerika Serikat, tunda 260 ms)

| Skenario transmisi | Ukuran data | model | `sync0` | `rsync -az` | Tambah multiplier |
| :--- | :---: | :---: | :---: | :---: | :---: |
| 500 file cilik | 1.91 MB | Sinkronisasi lengkap | 18.84 Mbps | 5.32 Mbps | 3.5x |
| 500 file cilik | 1.91 MB | Sinkronisasi tambahan | 27.15 Mbps | 3.83 Mbps | 7.1x |
| Kode lan paket arsip (`.tar`) | 10.0 MB | Sinkronisasi lengkap | 139.70 Mbps | 24.97 Mbps | 5.6x |
| Kode lan paket arsip (`.tar`) | 10.0 MB | Sinkronisasi tambahan | 83.38 Mbps | 26.02 Mbps | 3.2x |
| File video (`.mp4`) | 15.0 MB | Sinkronisasi lengkap | 54.06 Mbps | 27.04 Mbps | 2.0x |
| File video (`.mp4`) | 15.0 MB | Sinkronisasi tambahan | 216.83 Mbps | 21.51 Mbps | 10.1x |

### Jaringan transoceanic P2P multi-node pull test (China → Amerika Serikat dual node, tunda 260 ms)

Perbandingan sing diukur saka simpul wiji multi-sumber belang tarik bebarengan:

| Skenario transmisi | Ukuran data | Penyemaian simpul tunggal | Penyemaian bebarengan P2P rong simpul | Tambah multiplier |
| :--- | :---: | :---: | :---: | :---: |
| 500 file cilik | 1.97 MB | 5.28 Mbps | 7.87 Mbps | 1.5x |
| Kode lan paket arsip (`.tar`) | 30.0 MB | 56.35 Mbps | 83.17 Mbps | 1.5x |
| File video (`.mp4`) | 30.0 MB | 27.00 Mbps | 39.46 Mbps | 1.5x |

### Pangukuran aktual saka garis khusus kacepetan dhuwur transcontinental (Eropa → Amerika Utara, tundha 86 ms)

| Skenario transmisi | Ukuran data | `sync0` | `rsync` | Tambah multiplier |
| :--- | :---: | :---: | :---: | :---: |
| 10.000 file cilik | 77.6 MB | 688 Mbps | 128 Mbps | 5.4x |
| Kode lan paket arsip (`.tar`) | 512 MB | 2111 Mbps | 438 Mbps | 4.8x |
| File video (`.mp4`) | 512 MB | 556 Mbps | 338 Mbps | 1.6x |
| Sinkronisasi tambahan (ngowahi isi 1%) | 512 MB | 34133 Mbps | 2214 Mbps | 15.4x |

## Instal

### Cara 1: Instal liwat Cargo

```bash
cargo install sync0
```

### Cara 2: Ngundhuh binari sing wis dikompilasi

Bukak [Rilis GitHub](https://github.com/webc-site/sync0/releases) kanggo ngundhuh `.tar.xz` paket kompres kanggo sistem sing cocog (ndhukung Linux, macOS, Windows, Android, FreeBSD, NetBSD), unzip banjur lebokake ing direktori `PATH`:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Miwiti kanthi cepet

`sync0` nggunakake `~/.config/sync0` minangka direktori konfigurasi kanthi gawan (yen sampeyan kudu ngalih menyang direktori liyane, sampeyan bisa nambah `-c <directory>`).

### 1. Deleng alamat simpul lokal lan setelan kertu bisnis

```bash
# Deleng alamat simpul lokal (kunci digawe kanthi otomatis ing ~/.config/sync0 kanggo pisanan)
sync0 id

# Gawe kertu bisnis simpul pribadi (ndhukung jeneng pangguna, kutha, email, situs web lan teken ing 256 tembung)
sync0 profile -u alice --region SHA --email alice@example.com --url https://example.com -b "Shanghai node"
```

### 2. Nerbitake macem-macem grup enggo bareng file lan nyetel ijin anggota grup (`.nt` Konfigurasi)

`sync0` dikonfigurasi ing format [NestedText](https://nestedtext.org) (`.nt`). Saben pangguna duwe file konfigurasi `.nt` sing kapisah. Sampeyan bisa nemtokake direktori profil pangguna liwat `users_dir` ing `config.nt` (sing kanthi otomatis kalebu kabeh file `.nt` ing direktori iki). Pangguna lan awalan path sing diijini diunggah nggunakake dhaptar sing ora diurutake (`-`). **Yen awalan path upload ora diisi, upload menyang direktori root share diijini kanthi standar**.

1. Gawe file konfigurasi `.nt` sing kapisah kanggo saben anggota ing direktori `~/.config/sync0/users/`:

`~/.config/sync0/users/alice.nt` (anggota mung diwaca, tarif diwatesi `80MB`, `rate` ndhukung unit `GB`, `MB`, `KB`):

```yaml
id: 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c
perm: ro
rate: 80MB
```

`~/.config/sync0/users/bob.nt` (anggota unggahan sing diwatesi: mung diijini ngunggah menyang direktori `uploads/bob` lan `shared/common`):

```yaml
id: 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f
perm: rw
write_prefix:
  - uploads/bob
  - shared/common
```

`~/.config/sync0/users/charlie.nt` (Unggahan anggota standar: Nalika `write_prefix` diilangi, upload menyang direktori root saham diijini kanthi standar):

```yaml
id: 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
perm: rw
```

2. Konfigurasi direktori pangguna lan macem-macem grup enggo bareng file ing `~/.config/sync0/config.nt`:

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

Miwiti layanan enggo bareng:

```bash
# Miwiti langsung (otomatis mbukak kabeh konfigurasi pangguna .nt ing direktori ~/.config/sync0/config.nt lan users_dir)
sync0 share --region SHA

# Utawa cepet nerbitake macem-macem klompok enggo bareng lan wewenang anggota grup langsung liwat baris printah
sync0 share dataset:./data team-vault:./vault \
  --auth \
  --allow-ro 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c \
  --allow-rw 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f:uploads/bob,shared/common \
  --allow-rw 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
```

### 3. Tarik kabeh isi utawa pilih macem-macem file lan direktori (`sync0 pull`)

Sambungake menyang simpul peer kanggo entuk kabeh simpul wiji sing kasedhiya ing grup enggo bareng kanthi otomatis, lan tarik saka macem-macem simpul wiji bebarengan:

```bash
# Tarik kabeh klompok enggo bareng dataset menyang lokal ./local_data
sync0 narik dataset: // ./local_data -p <alamat simpul>

# Sinkronisasi multi-pilih: bebarengan narik file siji docs/readme.md lan subdirektori aset/ikon, src/inti
sync0 pull dataset://docs/readme.md,assets/icons,src/core ./local_data -p <alamat simpul>
```

### 4. Unggah file utawa direktori menyang grup enggo bareng (`sync0 push`)

Anggota sing duwe ijin upload (`rw` utawa `admin`) bisa ngunggah file utawa direktori lokal menyang grup enggo bareng remot:

```bash
# Anggota kanthi ijin upload direktori root: Unggah langsung menyang direktori root sing dienggo bareng
sync0 push team-vault: // ./new_release -p <alamat simpul>

# Mung anggota sing sah kanggo awalan tartamtu (kayata unggahan/bob): upload menyang awalan path sing diijini
sync0 push team-vault://uploads/bob/report.pdf ./report.pdf -p <alamat simpul>
```

### 5. Langganan sinkronisasi otomatis lan seeding (`sync0 sub`)

Sawise langganan grup enggo bareng saka simpul tartamtu, server lokal loro klien panampa lan server seeding anyar; nalika file simpul sumber dianyari, disinkronake kanthi otomatis lan nyedhiyakake undhuhan potongan kanggo simpul liyane:

```bash
# Langganan kabeh klompok enggo bareng lan otomatis winih
sync0 sub dataset: // ./local_mirror -p <alamat simpul>

# Mung multi-pilih kanggo langganan file lan subdirektori tartamtu lan wiji mau
sync0 sub dataset://models/llama.onnx,configs ./local_mirror -p <alamat simpul>
```

### 6. Sinkronisasi peer grup multi-mesin (`sync0 group`)

Akeh mesin gabung ing grup enggo bareng sing padha, saben liyane kanthi otomatis nemokake simpul wiji sing kasedhiya lan njaga sinkronisasi rong arah (ndhukung sinkronisasi lengkap utawa macem-macem pilihan file lan direktori):

```bash
# Gabung kanthi lengkap ing grup lan sinkronisasi rong arah
sync0 grup tim-docs: // ./docs -p <alamat simpul>

# Mung direktori wal lan file meta.json ing grup sinkronisasi multi-pilih
sync0 group team-docs://wal,meta.json ./docs -p <alamat simpul>
```

### 7. Transmisi langsung point-to-point antarane rong mesin (`send` / `recv`)

Ora perlu pra-konfigurasi grup, lan sampeyan bisa mindhah file utawa direktori kanggo sementara ing antarane rong mesin:

```bash
# Ngawasi panrima
sync0 recv 0.0.0.0:9000 ./backup

# Pungkasan ngirim langsung nransfer file utawa direktori (opsional --delete mirror kanggo ngresiki keluwihan file, --rate watesan kacepetan)
sync0 send ./project 192.168.1.10:9000
```

### 8. Ngalih multi-basa

`sync0` ndhukung 75 basa, lan kanthi otomatis ngalih miturut lingkungan basa sistem minangka standar; uga bisa ditemtokake kanthi manual liwat `SYNC0_LANG`:

```bash
SYNC0_LANG=en sync0 --help
SYNC0_LANG=ja sync0 --help
SYNC0_LANG=de sync0 --help
```