# sync0

`sync0` minangka fungsi enggo bareng file P2P sing didesentralisasi, sinkronisasi wektu nyata lan alat distribusi multi-sumber sing digunakake kanggo ngganti `rsync` - kacepetan diukur saka jaringan area jembar transcontinental nganti **6.95×** saka `rsync` (titik siji-titik geometris rata-rata kacepetan **2.01×***, kacepetan rata-rata geometris siji-titik **2.01×****, P2P. sinkronisasi file cilik massive tengen segara minangka cepet minangka **7.19×**.

Saben mesin iku loro server lan klien: ana ora perlu kanggo server tengah utawa alamat jaringan umum, otomatis bolongan punching adhedhasar alamat tombol umum jaringan mbagekke, lan ndhukung transmisi langsung antarane loro mesin, multi-fungsi enggo bareng ijin hosting, sawetara pilihan saka subdirektori ing dikarepake, millisecond-tingkat nyata-wektu sinkronisasi tambahan lan P2P multi-sumber bebarengan a.

## Instal

### Cara 1: Instalasi kargo

```bash
cargo install sync0
```

### Cara 2: Ngundhuh binari sing wis dikompilasi

Bukak [Rilis GitHub](https://github.com/webc-site/sync0/releases) kanggo ngundhuh `.tar.xz` paket kompres sing cocog karo sistem (ndhukung Linux, macOS, Windows, Android, iOS, FreeBSD), unzip banjur lebokake ing `PATH`:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Miwiti kanthi cepet

### 1. Transmisi langsung tanpa konfigurasi antarane rong mesin (`send` / `recv`)

Ora perlu ngatur grup enggo bareng, langsung nransfer file utawa direktori ing antarane rong mesin kanthi kacepetan dhuwur:

```bash
# Pungkasan panampa ngrungokake ing port lan nulis menyang ./backup
sync0 recv 0.0.0.0:9000 ./backup

# Transmisi langsung saka pangirim (opsional --mbusak pangilon kanggo ngresiki keluwihan file, --rate watesan kacepetan 50MB)
sync0 send ./project 192.168.1.10:9000
```

### 2. Nuduhake kiriman lan tarik / unggahan sing dikarepake (`share` / `pull` / `push`)

```bash
# Server: Nerbitake grup enggo bareng karo siji baris printah (output alamat simpul iroh tanpa IP sawise wiwitan)
sync0 share dataset:./data

# Klien: Tarik kabeh klompok enggo bareng dataset menyang lokal ./local_data
sync0 narik dataset: // ./local_data -p <alamat simpul>

# Klien: Narik macem-macem pilihan sing dikarepake (mung nyinkronake file lan subdirektori sing ditemtokake)
sync0 pull dataset://docs/readme.md,assets/icons,src/core ./local_data -p <alamat simpul>

# Klien: Unggah file utawa direktori menyang grup enggo bareng sing sah
sync0 push dataset: // release / v1 ./build_output -p <alamat simpul>
```

### 3. Penyemaian langganan wektu nyata lan sinkronisasi rong arah multi-mesin (`sub` / `group`)

Sawise nggawe sambungan dawa, yen ana file ** nambah, ngowahi, mbusak, ngganti ijin utawa wektu modifikasi ** ana ing mburi remot, direktori lokal bakal synchronously dianyari ing milliseconds ing wektu nyata (anyar / dipunéwahi file gedhe mung bakal narik pamblokiran prabédan lan nganyari ing panggonan, mbusak file bakal kanthi otomatis ngresiki kaping lokal lan data kosong, lan mung ngganti ijin nol lan mung ngganti ijin nol.

- **`sync0 sub`(Mirroring wektu nyata siji arah + seeding otomatis)**: Wektu nyata lokal ngetutake tambahan, pambusakan lan modifikasi saka mburi enggo bareng, lan kanthi otomatis dadi sumber wiji P2P kanggo nyediakake akselerasi bebarengan kanggo kelenjar narik liyane.
- **`sync0 group`(Multi-machine peer-to-peer two-way real-time sinkronisasi)**: Yen file ditambahake, dibusak, utawa diowahi ing mesin apa wae ing grup, kabeh simpul online liyane bakal dianyari bebarengan ing wektu nyata lan dadi wiji kanggo saben liyane.

```bash
# Langganan kabeh grup enggo bareng: tambahan remot, pambusakan, lan modifikasi disinkronake sacara lokal ing wektu nyata lan kanthi otomatis winih
sync0 sub dataset: // ./local_mirror -p <alamat simpul>

# Langganan multi-pilih sing dikarepake: mung nyinkronake lan nyebarake file lan subdirektori sing ditemtokake ing wektu nyata
sync0 sub dataset://models/llama.onnx,configs ./local_mirror -p <alamat simpul>

# Multi-machine peer group two-way real-time sinkronisasi (uga ndhukung path multi-pilih lengkap utawa koma)
sync0 grup tim-docs: // ./docs -p <alamat simpul>
```

### 4. Konfigurasi ijin grup lan anggota tim multi-sharing (`.nt` konfigurasi)

Direktori konfigurasi standar yaiku `~/.config/sync0` (bisa diganti liwat `-c <directory>`), nggunakake format minimalis [NestedText](https://nestedtext.org) (`.nt`):
- **Ndhukung mbatesi maca jaringan umum (mung pangguna sing sah sing bisa maca)**: Anggere grup enggo bareng wis dikonfigurasi karo `users:` dhaptar anggota (utawa baris printah nemtokake `--allow-r` / `--allow-rw` / `--allow-w` / `--auth`), sistem kasebut ** ngaktifake kanthi otomatis ("**b id privat) jaringan enumerasi simpul anonim utawa maca, lan P2P Langganan kanggo seeding kelenjar uga bakal oleh warisan whitelist bebarengan, ngidini mung sah kedhaftar kanggo maca; grup enggo bareng sing ora dikonfigurasi karo `users` umum lan mung diwaca minangka standar (yen sampeyan pengin dadi umum lan mung diwaca lan mbukak kanggo upload mung kanggo anggota tartamtu, sampeyan bisa nyetel `auth: false` kanthi jelas).
- **Izin kiriman maca-tulis lan subdirektori sing apik**: Anggota mung diwaca kanthi standar (`r` ora perlu ditulis kanthi jelas), lan ndhukung `r` (mung diwaca), `w` (mung nulis menyang kothak gulung, ora bisa diwaca), <b id = "id-5" lan "diwaca" awalan diijini kanggo diunggah).

**Contoh konfigurasi anggota (`~/.config/sync0/users/`):**

```yaml
# ~/.config/sync0/users/alice.nt (mung diwaca standar, watesan kacepetan 80MB)
id: 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c
rate: 80MB
```

```yaml
# ~/.config/sync0/users/bob.nt (ijin maca lan nulis, mung diijini ngunggah menyang subdirektori sing ditemtokake)
id: 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f
perm: rw
write_prefix:
  - uploads/bob
  - shared/common
```

```yaml
# ~/.config/sync0/users/charlie.nt (mung nulis ijin pangiriman, maca dilarang)
id: 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
perm: w
```

**Contoh konfigurasi grup enggo bareng (`~/.config/sync0/config.nt`):**

```yaml
users_dir: ./users

groups:
  # Pangguna sing ora dikonfigurasi: jaringan umum mung diwaca
  dataset:
    path: ./data
    rate: 50MB

  # Pangguna sing dikonfigurasi: matesi maca jaringan umum kanthi otomatis (mung alice lan bob sing bisa maca, charlie mung bisa ngirim lan ngunggah)
  team-vault:
    path: ./vault
    users:
      - alice
      - bob
      - charlie
```

Miwiti layanan (muat file konfigurasi utawa langsung nggunakake paramèter baris perintah kanggo matesi maca jaringan umum lan menehi wewenang anggota):

```bash
# Muat ~/.config/sync0/config.nt kanggo miwiti
sync0 share --region SHA

# Utawa nerbitake grup enggo bareng pribadi liwat baris printah murni (nemtokake --allow-* kanggo matesi maca jaringan umum kanthi otomatis lan mung pangguna sah sing bisa ngakses)
sync0 share team-vault:./vault \
  --allow-r 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c \
  --allow-rw 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f:uploads/bob,shared/common \
  --allow-w 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
```

### 5. Alamat simpul, kertu bisnis lan multi-basa

```bash
# Deleng alamat simpul bebas IP lokal (kunci digawe kanthi otomatis kanggo mbukak pisanan)
sync0 id

# Gawe kertu bisnis simpul sing ditandatangani dhewe Ed25519 (jeneng celukan, kode kutha, email, situs web, introduksi)
sync0 profile -u alice --region SHA --email alice@example.com -b "Shanghai node"

# Ana 75 basa sing dibangun, sing ngetutake basa sistem kanthi standar lan bisa dialihake liwat variabel lingkungan.
SYNC0_LANG=en sync0 --help
```

## Kinerja lan fitur inti

### Pengukuran aktual WAN lintas benua (Perbandingan `rsync`)

Lingkungan pangukuran aktual: Amerika Serikat `c1` → Eropa `n1` (RTT 65 ms), set data yaiku kernel Linux `linux-7.3.3`:

#### 1) Mode A la carte vs `rsync`

| kumpulan data | ukuran | `sync0` Titik Tunggal<br>(Mbps) | `rsync`<br>(Mbps) | Speedup (`sync0` / `rsync`) |
| :--- | ---: | ---: | ---: | ---: |
| `linux-7.3.3.tar.xz`(ora bisa dikompres) | 155 MB | **121.07** | 107.89 | **1.12×** |
| `linux-7.3.3.tar`(bisa dikompres) | 1.6 GB | **442.07** | 506.62 | **0.87×** |
| `linux-7.3.3.tar`(tambahan owah-owahan lokal) | 1.6 GB | **8,123.87** | 1,169.49 | **6.95×** |
| `linux-7.3.3/` (95.979 file cilik) | 1.5 GB | **565.49** | 235.27 | **2.40×** |
| ** tegese geometris ** | — | **704.17** | 350.19 | **2.01×** |

#### 2) Mode dual-sumber P2P vs `rsync`

| kumpulan data | ukuran | `sync0` Sumber Ganda P2P<br>(Mbps) | `rsync`<br>(Mbps) | Speedup (`sync0` / `rsync`) |
| :--- | ---: | ---: | ---: | ---: |
| `linux-7.3.3.tar.xz`(ora bisa dikompres) | 155 MB | **466.85** | 107.89 | **4.33×** |
| `linux-7.3.3.tar`(bisa dikompres) | 1.6 GB | **992.17** | 506.62 | **1.96×** |
| `linux-7.3.3.tar`(tambahan owah-owahan lokal) | 1.6 GB | **5,666.84** | 1,169.49 | **4.85×** |
| `linux-7.3.3/` (95.979 file cilik) | 1.5 GB | **703.30** | 235.27 | **2.99×** |
| ** tegese geometris ** | — | **1,165.63** | 350.19 | **3.33×** |

> Cathetan: Ing mode dual-source P2P, node Eropa `s1` kolaborasi kanggo seeding, lan RTT backbone cross-continental yaiku 65 ms.

### Fitur produk inti

- **Tombol umum ngarahake bolongan otomatis **: Ora perlu server pusat lan alamat jaringan umum, otomatis bolongan NAT adhedhasar alamat kunci umum jaringan sing disebarake.
- **Sinkronisasi wektu nyata lan kesadaran metadata tingkat milidetik**: Adhedhasar pemantauan acara file asli (`FSEvents` / `inotify` / `ReadDirectoryChangesW`), push nyata-wektu tambahan file, pambusakan, ijin POSIX (`mode`) lan wektu modifikasi tingkat milidetik (`mtime`); transmisi pamblokiran data nul nalika mung ijin utawa wektu diowahi.
- ** BT-gaya download simultaneous lan upload lan jadwal karemenan regional **: Node ngundhuh ora perlu ngenteni jumlah lengkap kanggo diundhuh. Kabeh interval pemblokiran (<b id = "1") sing wis liwati mriksa hash 128-bit langsung dikunci ing memori / blumbang sauntara kanggo kelenjar liyane kanggo narik bebarengan. Bebarengan karo pertukaran simpul PEX dinamis lan preemption setengah simpul alon (Work-Stealing), throughput kluster dimaksimalkan.
- **Kekebalan simpul anti-malayo lan keracunan kluster**: dhaptar sumber resmi `root_hash` Ngunci mandiri + akselerasi hardware wajib `gxhash128` verifikasi pencernaan sadurunge saben blok diselehake ing disk lan winih eksternal; yen pemblokiran tampered ditemokake, mbobol sirkuit tingkat milidetik bakal blackout simpul angkoro lan kanthi otomatis waras saka sumber jujur.
- **FastCDC in-situ increment lan agregasi file cilik**: Mung pamblokiran diferensial sing ditransfer kanggo modifikasi parsial file gedhe lan diselehake ing disk; packaging memori file cilik massive lan komprèsi diferensial ater-ater ngilangke file-by-file round-trip overhead.
- ** Marasake awak dhewe sawise mati daya lan resumption sawise miwiti maneh **: Redis-kaya RDB + AOF format hibrida (`.s0_subs.bin`) digunakake kanggo terus status seeding lan dhaftar file, owah-owahan saben dina ditambahake ing microseconds, lan otomatis ditulis maneh lan diatur ajeg. Sawise server diwiwiti maneh, owah-owahan offline kanthi otomatis di-push menyang simpul seeding.

## Arsitektur teknis lan dokumentasi

- **Protokol decoupling bidang ganda**:
  - **Kontrol lan permukaan pangowahan wektu nyata (`iroh` QUIC / TCP, byte pisanan `PROTO_VERSION` versi jabat tangan) **: Tanggung jawab kanggo verifikasi kertu bisnis Ed25519, verifikasi ijin, panemuan wiji Swarm, lan dhukungan kanggo batching streaming pangowahan massa tingkat lan `mzstd` dikompres `u64::MAX` level lan `mzstd` nyata id="6"> / `FsOp::Remove` / `FsOp::Meta` Telung primitif).
  - ** Lumahing distribusi data (UDP asli + `compio` I/O asinkron) **: header kompak 20B, umpan balik periodik 18B TACK, kontrol kemacetan hibrida BBRv3/Copa, Recovery mundhut paket RACK-TLP, blok pertama Shannon entropi adaptif Zstd komprèsi bypass, lan <b id-end.
- **Sinau luwih lengkap**:
  - [Laporan pangukuran kinerja lan skrip reproduksi](https://github.com/webc-site/sync0/blob/main/doc/zh/benchmark.md)
  - [Desain arsitektur sistem](https://github.com/webc-site/sync0/blob/main/doc/zh/design_v4.md)
  - [Protokol Kawat Binary lan Spesifikasi Ketekunan](https://github.com/webc-site/sync0/blob/main/doc/zh/proto.md)
  - [Otentikasi identitas, kontrol wewenang lan verifikasi integritas](https://github.com/webc-site/sync0/blob/main/doc/zh/crypto.md)