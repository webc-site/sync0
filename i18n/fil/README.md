# sync0

Ang `sync0` ay isang peer-to-peer (P2P) file sharing at multi-machine synchronization tool na nakikipagkumpitensya sa Resilio Sync.

Ang bawat makina ay parehong server at isang kliyente: anumang node ay maaaring mag-publish ng maramihang mga grupo sa pagbabahagi ng file. Pagkatapos sumali sa ibang mga node, awtomatiko silang makakakuha ng mga available na seed node mula sa peer server at sabay-sabay na hilahin ang mga ito mula sa maraming source. Pagkatapos makumpleto ang pag-synchronize, ang mga buto ay awtomatikong gagawin para sa higit pang mga node upang mapabilis ang mga pag-download.

## Mga Pangunahing Highlight

- **Lahat ay parehong server at kliyente**: Maihahambing sa Resilio Sync, walang dependency sa gitnang server. Anumang node ay maaaring lumikha ng maramihang mga grupo sa pagbabahagi ng file, at maaari ding sumali sa mga grupo ng pagbabahagi ng ibang tao; ang mga node ay awtomatikong nagpapalitan at kumuha ng mga available na seed node upang makamit ang sabay-sabay na paghila at real-time na pag-synchronize ng maraming makina.
- **IP-free node address**: Bilang default, ang node key at `base64url` na naka-encode na `iroh` address ay awtomatikong nabuo sa `~/.config/sync0` nang hindi kinakailangang tandaan o ilantad ang pampublikong IP.
- **Multi-select na pag-synchronise on demand**: Sinusuportahan ang pag-synchronize ng buong grupo ng pagbabahagi, at sinusuportahan din ang pag-synchronize on demand sa pamamagitan ng pagpili ng maraming file o subdirectory nang sabay-sabay.
- **Mga hierarchical na pahintulot ng miyembro ng grupo at kontrol ng direktoryo sa pag-upload**: Sinusuportahan ang read-only (`ro`), read-write upload (`rw`), at pamamahala (`admin`) na mga pahintulot, at maaaring indibidwal na tukuyin ang path prefix na pinapayagang i-upload ng bawat miyembro (bilang default, ang pag-upload sa root ng share ay pinapayagan).
- **Full-speed at incremental na pag-synchronize sa mga network**: Kasabay na pagpapadala ng malalaking file, pagsasama-sama ng napakalaking maliliit na file sa memorya, at naka-synchronize lang na mga bloke ng data kapag bahagyang binago ang mga file.

## Paghahambing ng bilis sa rsync

### Transoceanic network single source na aktwal na pagsukat (China → United States, delay 260 ms)

| Sitwasyon ng paghahatid | Laki ng data | modelo | `sync0` | `rsync -az` | Dagdagan ang multiplier |
| :--- | :---: | :---: | :---: | :---: | :---: |
| 500 maliliit na file | 1.91 MB | Buong pag-synchronize | 18.84 Mbps | 5.32 Mbps | 3.5x |
| 500 maliliit na file | 1.91 MB | Incremental na pag-synchronize | 27.15 Mbps | 3.83 Mbps | 7.1x |
| Code at archive packages (`.tar`) | 10.0 MB | Buong pag-synchronize | 139.70 Mbps | 24.97 Mbps | 5.6x |
| Code at archive packages (`.tar`) | 10.0 MB | Incremental na pag-synchronize | 83.38 Mbps | 26.02 Mbps | 3.2x |
| Video file (`.mp4`) | 15.0 MB | Buong pag-synchronize | 54.06 Mbps | 27.04 Mbps | 2.0x |
| Video file (`.mp4`) | 15.0 MB | Incremental na pag-synchronize | 216.83 Mbps | 21.51 Mbps | 10.1x |

### Transoceanic network P2P multi-node pull test (China → United States dual node, delay 260 ms)

Sinusukat na paghahambing ng multi-source seed node na may striped concurrent pull:

| Sitwasyon ng paghahatid | Laki ng data | Single node seeding | Dalawang-node na P2P na sabay-sabay na seeding | Dagdagan ang multiplier |
| :--- | :---: | :---: | :---: | :---: |
| 500 maliliit na file | 1.97 MB | 5.28 Mbps | 7.87 Mbps | 1.5x |
| Code at archive packages (`.tar`) | 30.0 MB | 56.35 Mbps | 83.17 Mbps | 1.5x |
| Video file (`.mp4`) | 30.0 MB | 27.00 Mbps | 39.46 Mbps | 1.5x |

### Aktwal na pagsukat ng transcontinental high-speed dedicated line (Europe → North America, delay 86 ms)

| Sitwasyon ng paghahatid | Laki ng data | `sync0` | `rsync` | Dagdagan ang multiplier |
| :--- | :---: | :---: | :---: | :---: |
| 10,000 maliliit na file | 77.6 MB | 688 Mbps | 128 Mbps | 5.4x |
| Code at archive packages (`.tar`) | 512 MB | 2111 Mbps | 438 Mbps | 4.8x |
| Video file (`.mp4`) | 512 MB | 556 Mbps | 338 Mbps | 1.6x |
| Incremental na pag-synchronize (baguhin ang 1% na nilalaman) | 512 MB | 34133 Mbps | 2214 Mbps | 15.4x |

## I-install

### Paraan 1: I-install sa pamamagitan ng Cargo

```bash
cargo install sync0
```

### Paraan 2: Mag-download ng mga precompiled na binary

Pumunta sa [GitHub Releases](https://github.com/webc-site/sync0/releases) para i-download ang `.tar.xz` compressed package para sa kaukulang system (sumusuporta sa Linux, macOS, Windows, Android, FreeBSD, NetBSD), i-unzip at ilagay ito sa `PATH` na direktoryo:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Magsimula nang mabilis

Ginagamit ng `sync0` ang `~/.config/sync0` bilang direktoryo ng configuration bilang default (kung kailangan mong lumipat sa iba pang mga direktoryo, maaari kang magdagdag ng `-c <directory>`).

### 1. Tingnan ang lokal na node address at setting ng business card

```bash
# Tingnan ang lokal na node address (ang susi ay awtomatikong nabuo sa ~/.config/sync0 sa unang pagkakataon)
sync0 id

# Bumuo ng personal na node business card (sumusuporta sa user name, lungsod, email, website at lagda sa loob ng 256 na salita)
sync0 profile -u alice --region SHA --email alice@example.com --url https://example.com -b "Shanghai node"
```

### 2. Mag-publish ng maramihang mga grupo sa pagbabahagi ng file at magtakda ng mga pahintulot ng miyembro ng grupo (`.nt` Configuration)

`sync0` ay naka-configure sa [NestedText](https://nestedtext.org) (`.nt`) na format. Ang bawat user ay may hiwalay na `.nt` configuration file. Maaari mong tukuyin ang direktoryo ng profile ng user sa pamamagitan ng `users_dir` sa `config.nt` (na awtomatikong kasama ang lahat ng `.nt` file sa direktoryong ito). Ang user at ang path prefix na pinapayagang ma-upload ay gumagamit ng isang hindi nakaayos na listahan (`-`). **Kapag ang upload path prefix ay hindi napunan, ang pag-upload sa root directory ng share ay pinapayagan bilang default**.

1. Gumawa ng hiwalay na `.nt` configuration file para sa bawat miyembro sa `~/.config/sync0/users/` na direktoryo:

`~/.config/sync0/users/alice.nt` (read-only na miyembro, limitado ang rate `80MB`, `rate` ay sumusuporta sa `GB`, `MB`, `KB` na mga unit):

```yaml
id: 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c
perm: ro
rate: 80MB
```

`~/.config/sync0/users/bob.nt` (restricted upload member: pinapayagan lang mag-upload sa `uploads/bob` at `shared/common` na mga direktoryo):

```yaml
id: 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f
perm: rw
write_prefix:
  - uploads/bob
  - shared/common
```

`~/.config/sync0/users/charlie.nt` (Default na pag-upload ng miyembro: Kapag ang `write_prefix` ay tinanggal, ang pag-upload sa root directory ng share ay pinapayagan bilang default):

```yaml
id: 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
perm: rw
```

2. I-configure ang direktoryo ng user at maramihang mga pangkat sa pagbabahagi ng file sa `~/.config/sync0/config.nt`:

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

Simulan ang pagbabahagi ng serbisyo:

```bash
# Direktang pagsisimula (awtomatikong nilo-load ang lahat ng .nt na configuration ng user sa ~/.config/sync0/config.nt at mga user_dir na direktoryo)
sync0 share --region SHA

# O mabilis na mag-publish ng maraming grupo sa pagbabahagi at direktang pahintulutan ang mga miyembro ng grupo sa pamamagitan ng command line
sync0 share dataset:./data team-vault:./vault \
  --auth \
  --allow-ro 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c \
  --allow-rw 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f:uploads/bob,shared/common \
  --allow-rw 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
```

### 3. Hilahin ang lahat ng nilalaman o pumili ng maramihang mga file at direktoryo (`sync0 pull`)

Kumonekta sa anumang peer node para awtomatikong makuha ang lahat ng available na seed node sa pagbabahagi ng grupo, at sabay na hilahin mula sa maraming seed node:

```bash
# Hilahin ang buong pangkat sa pagbabahagi ng dataset sa local ./local_data
sync0 pull dataset:// ./local_data -p <node address>

# Multi-select synchronization: sabay-sabay na hilahin ang solong file na docs/readme.md at mga subdirectory na asset/icon, src/core
sync0 pull dataset://docs/readme.md,assets/icons,src/core ./local_data -p <node address>
```

### 4. Mag-upload ng file o direktoryo sa isang pagbabahaging grupo (`sync0 push`)

Ang mga miyembrong may mga pahintulot sa pag-upload (`rw` o `admin`) ay maaaring mag-upload ng mga lokal na file o direktoryo sa malayuang pagbabahagi ng mga grupo:

```bash
# Mga miyembrong may pahintulot sa pag-upload ng root directory: Direktang mag-upload sa nakabahaging root directory
sync0 push team-vault:// ./new_release -p <node address>

# Mga miyembro lang na pinahintulutan sa isang partikular na prefix (gaya ng mga pag-upload/bob): mag-upload sa pinapayagang prefix ng landas
sync0 push team-vault://uploads/bob/report.pdf ./report.pdf -p <node address>
```

### 5. Mag-subscribe sa awtomatikong pag-sync at seeding (`sync0 sub`)

Pagkatapos mag-subscribe sa pagbabahagi ng grupo ng isang partikular na node, ang lokal na server ay parehong tumatanggap na kliyente at isang bagong seeding server; kapag na-update ang source node file, awtomatiko itong isi-synchronize at nagbibigay ng mga chunked download para sa iba pang mga node:

```bash
# Mag-subscribe sa buong grupo ng pagbabahagi at awtomatikong mag-seed
sync0 sub dataset:// ./local_mirror -p <node address>

# Multi-select lang upang mag-subscribe sa mga tinukoy na file at subdirectory at i-seed ang mga ito
sync0 sub dataset://models/llama.onnx,configs ./local_mirror -p <node address>
```

### 6. Multi-machine group peer synchronization (`sync0 group`)

Maraming makina ang sumasali sa parehong grupo ng pagbabahagi, awtomatikong natutuklasan ng bawat isa ang mga available na seed node at nagpapanatili ng two-way na pag-synchronize (sinusuportahan ang buong pag-synchronize o maraming seleksyon ng mga file at direktoryo):

```bash
# Ganap na sumali sa grupo at two-way synchronization
sync0 group team-docs:// ./docs -p <node address>

# Tanging ang wal directory at meta.json file sa multi-select na synchronization group
sync0 group team-docs://wal,meta.json ./docs -p <node address>
```

### 7. Point-to-point na direktang paghahatid sa pagitan ng dalawang makina (`send` / `recv`)

Hindi na kailangang i-pre-configure ang isang grupo, at maaari kang pansamantalang maglipat ng mga file o direktoryo nang direkta sa pagitan ng dalawang machine:

```bash
# Pagsubaybay ng tatanggap
sync0 recv 0.0.0.0:9000 ./backup

# Ang dulo ng pagpapadala ay direktang naglilipat ng mga file o direktoryo (opsyonal --tanggalin ang salamin upang linisin ang mga labis na file, --rate na limitasyon ng bilis)
sync0 send ./project 192.168.1.10:9000
```

### 8. Multi-language switching

Sinusuportahan ng `sync0` ang 75 wika, at awtomatikong lumilipat ayon sa kapaligiran ng wika ng system bilang default; maaari rin itong tukuyin nang manu-mano sa pamamagitan ng `SYNC0_LANG`:

```bash
SYNC0_LANG=en sync0 --help
SYNC0_LANG=ja sync0 --help
SYNC0_LANG=de sync0 --help
```