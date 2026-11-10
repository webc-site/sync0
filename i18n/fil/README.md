# sync0

Ang `sync0` ay isang desentralisadong P2P file sharing, real-time na pag-synchronize at multi-source distribution tool na ginagamit upang palitan ang `rsync` - ang sinusukat na bilis ng transcontinental wide area network ay hanggang **6.95×** ng `rsync` (single-point geometric average na bilis **2.01×**. P2c3 average na bilis **2.01×**, P2c3 na average na bilis ******, P2c3 na pinagmumulan. Ang pag-synchronize ng napakalaking maliliit na file sa mga karagatan ay kasing bilis ng **7.19×**.

Ang bawat makina ay parehong server at isang kliyente: hindi na kailangan para sa isang sentral na server o isang pampublikong network address, ang awtomatikong pagsuntok ng butas ay batay sa ipinamahagi na network ng pampublikong key address, at ito ay sumusuporta sa direktang paghahatid sa pagitan ng dalawang makina, multi-sharing group permission hosting, maraming seleksyon ng mga subdirectory on demand, millisecond-level real-time incremental synchronization at P2P multi-source concurrent a.

## I-install

### Paraan 1: Pag-install ng kargamento

```bash
cargo install sync0
```

### Paraan 2: Mag-download ng mga precompiled na binary

Pumunta sa [GitHub Releases](https://github.com/webc-site/sync0/releases) para i-download ang `.tar.xz` compressed package na naaayon sa system (sumusuporta sa Linux, macOS, Windows, Android, iOS, FreeBSD), i-unzip at ilagay sa `PATH`:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Magsimula nang mabilis

### 1. Direktang paghahatid ng zero-configuration sa pagitan ng dalawang makina (`send` / `recv`)

Hindi na kailangang i-configure ang isang grupo sa pagbabahagi, direktang maglipat ng mga file o direktoryo sa pagitan ng dalawang makina sa napakabilis:

```bash
# Ang receiving end ay nakikinig sa port at nagsusulat sa ./backup
sync0 recv 0.0.0.0:9000 ./backup

# Direktang pagpapadala mula sa nagpadala (opsyonal --tanggalin ang salamin upang linisin ang labis na mga file, --rate ng 50MB na limitasyon sa bilis)
sync0 send ./project 192.168.1.10:9000
```

### 2. Pagbabahagi ng post at on-demand na pull/upload (`share` / `pull` / `push`)

```bash
# Server: I-publish ang sharing group na may isang linya ng command (i-output ang IP-free na iroh node address pagkatapos ng startup)
sync0 share dataset:./data

# Kliyente: Hilahin ang buong pangkat sa pagbabahagi ng dataset sa lokal na ./local_data
sync0 pull dataset:// ./local_data -p <node address>

# Kliyente: Hilahin ang maraming mga pagpipilian kapag hinihiling (i-synchronize lamang ang mga tinukoy na solong file at subdirectory)
sync0 pull dataset://docs/readme.md,assets/icons,src/core ./local_data -p <node address>

# Kliyente: Mag-upload ng mga file o direktoryo sa mga awtorisadong grupo ng pagbabahagi
sync0 push dataset://releases/v1 ./build_output -p <node address>
```

### 3. Real-time na subscription seeding at multi-machine two-way synchronization (`sub` / `group`)

Pagkatapos magtatag ng mahabang koneksyon, kung ang anumang file **pagdaragdag, pagbabago, pagtanggal, pagbabago ng mga pahintulot o oras ng pagbabago** ay magaganap sa remote na dulo, ang lokal na direktoryo ay sabay-sabay na ia-update sa millisecond sa real time (ang mga bago/binagong malalaking file ay kukuha lamang ng mga bloke ng pagkakaiba at i-update ang mga ito sa lugar, ang mga magtanggal ng mga file ay awtomatikong maglilinis ng mga lokal na oras at mga walang laman na mga direktoryo, at direktang babaguhin ang mga pahintulot:

- **`sync0 sub`(One-way na real-time na pag-mirror + awtomatikong seeding)**: Ang lokal na real-time ay sumusunod sa mga pagdaragdag, pagtanggal at pagbabago ng pagtatapos ng pagbabahagi, at awtomatikong nagsisilbing P2P seed source upang magbigay ng sabay-sabay na acceleration para sa iba pang mga pulling node.
- **`sync0 group`(Multi-machine peer-to-peer two-way real-time synchronization)**: Kung ang mga file ay idinagdag, tatanggalin, o binago sa anumang machine sa grupo, lahat ng iba pang online na node ay ia-update nang sabay-sabay sa real time at magsisilbing mga buto para sa isa't isa.

```bash
# Mag-subscribe sa buong grupo ng pagbabahagi: ang mga malalayong pagdaragdag, pagtanggal, at pagbabago ay lokal na naka-synchronize sa real time at awtomatikong na-seeded
sync0 sub dataset:// ./local_mirror -p <node address>

# On-demand na multi-select na subscription: i-synchronize lang at i-seed ang mga solong file at subdirectory sa real time
sync0 sub dataset://models/llama.onnx,configs ./local_mirror -p <node address>

# Multi-machine peer group two-way real-time na pag-synchronize (sinusuportahan din ang mga full o comma multi-select na path)
sync0 group team-docs:// ./docs -p <node address>
```

### 4. Konfigurasyon ng pahintulot ng pangkat na maraming pagbabahagi ng pangkat at miyembro (`.nt` configuration)

Ang default na direktoryo ng configuration ay `~/.config/sync0` (maaaring ilipat sa pamamagitan ng `-c <directory>`), gamit ang minimalist na [NestedText](https://nestedtext.org) (`.nt`) na format:
- **Sinusuportahan ang paghihigpit sa pampublikong pagbabasa sa network (ang mga awtorisadong user lang ang makakabasa)**: Hangga't ang grupo sa pagbabahagi ay naka-configure na may `users:` listahan ng miyembro (o ang command line ay tumutukoy sa `--allow-r` / `--allow-rw` / `--allow-w` / `--auth`, ang system **awtomatikong ino-on at pribado ang system (`users` ay pampubliko at read-only bilang default (kung gusto mong maging pampubliko at read-only at bukas sa mga pag-upload lamang sa mga tinukoy na miyembro, maaari mong tahasang itakda ang `auth: false`).
- **Fine-grained read-write at mga pahintulot sa paghahatid ng subdirectory**: Ang mga miyembro ay read-only bilang default (`r` ay hindi kailangang isulat nang tahasan), at suportahan ang `r` (read-only), `w` (sumulat lamang sa drop box, hindi nababasa), `rw` at <b id="write"> (idirekta) prefix na pinapayagan para sa pag-upload).

**Halimbawa ng configuration ng miyembro (`~/.config/sync0/users/`):**

```yaml
# ~/.config/sync0/users/alice.nt (default na read-only, speed limit 80MB)
id: 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c
rate: 80MB
```

```yaml
# ~/.config/sync0/users/bob.nt (mga pahintulot sa pagbasa at pagsulat, pinapayagan lamang na mag-upload sa tinukoy na subdirectory)
id: 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f
perm: rw
write_prefix:
  - uploads/bob
  - shared/common
```

```yaml
# ~/.config/sync0/users/charlie.nt (magsulat lamang ng pahintulot sa paghahatid, ipinagbabawal ang pagbabasa)
id: 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
perm: w
```

**Halimbawa ng configuration ng pagbabahagi ng grupo (`~/.config/sync0/config.nt`):**

```yaml
users_dir: ./users

groups:
  # Hindi naka-configure na mga user: default na pampublikong network na read-only
  dataset:
    path: ./data
    rate: 50MB

  # Mga naka-configure na user: awtomatikong paghigpitan ang pampublikong pagbabasa sa network (si alice at bob lang ang makakabasa, si charlie lang ang makakapagdeliver at makakapag-upload)
  team-vault:
    path: ./vault
    users:
      - alice
      - bob
      - charlie
```

Simulan ang serbisyo (i-load ang configuration file o direktang gumamit ng mga parameter ng command line upang paghigpitan ang pagbabasa ng pampublikong network at pahintulutan ang mga miyembro):

```bash
# I-load ang ~/.config/sync0/config.nt para magsimula
sync0 share --region SHA

# O mag-publish ng pribadong grupo sa pagbabahagi sa pamamagitan ng purong command line (tukuyin ang --allow-* upang awtomatikong paghigpitan ang pagbabasa ng pampublikong network at ang mga awtorisadong user lang ang makaka-access nito)
sync0 share team-vault:./vault \
  --allow-r 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c \
  --allow-rw 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f:uploads/bob,shared/common \
  --allow-w 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
```

### 5. Node address, business card at multi-language

```bash
# Tingnan ang lokal na IP-free node address (awtomatikong nabuo ang key para sa unang pagtakbo)
sync0 id

# Bumuo ng Ed25519 self-signed node business card (palayaw, code ng lungsod, email, website, panimula)
sync0 profile -u alice --region SHA --email alice@example.com -b "Shanghai node"

# Mayroong 75 built-in na wika, na sumusunod sa wika ng system bilang default at maaaring ilipat sa pamamagitan ng mga variable ng kapaligiran.
SYNC0_LANG=en sync0 --help
```

## Pagganap at mga pangunahing tampok

### Cross-continental WAN aktwal na pagsukat (Paghahambing `rsync`)

Aktwal na kapaligiran sa pagsukat: United States `c1` → Europe `n1` (RTT 65 ms), ang data set ay Linux kernel `linux-7.3.3`:

#### 1) A la carte mode vs `rsync`

| Set ng data | laki | `sync0` Isang Punto<br>(Mbps) | `rsync`<br>(Mbps) | Bilis (`sync0` / `rsync`) |
| :--- | ---: | ---: | ---: | ---: |
| `linux-7.3.3.tar.xz`(hindi compressible) | 155 MB | **121.07** | 107.89 | **1.12×** |
| `linux-7.3.3.tar`(compressible) | 1.6 GB | **442.07** | 506.62 | **0.87×** |
| `linux-7.3.3.tar`(lokal na pagtaas ng pagbabago) | 1.6 GB | **8,123.87** | 1,169.49 | **6.95×** |
| `linux-7.3.3/` (95,979 maliliit na file) | 1.5 GB | **565.49** | 235.27 | **2.40×** |
| **geometric mean** | — | **704.17** | 350.19 | **2.01×** |

#### 2) P2P dual-source mode vs `rsync`

| Set ng data | laki | `sync0` P2P Dual Source<br>(Mbps) | `rsync`<br>(Mbps) | Bilis (`sync0` / `rsync`) |
| :--- | ---: | ---: | ---: | ---: |
| `linux-7.3.3.tar.xz`(hindi compressible) | 155 MB | **466.85** | 107.89 | **4.33×** |
| `linux-7.3.3.tar`(compressible) | 1.6 GB | **992.17** | 506.62 | **1.96×** |
| `linux-7.3.3.tar`(lokal na pagtaas ng pagbabago) | 1.6 GB | **5,666.84** | 1,169.49 | **4.85×** |
| `linux-7.3.3/` (95,979 maliliit na file) | 1.5 GB | **703.30** | 235.27 | **2.99×** |
| **geometric mean** | — | **1,165.63** | 350.19 | **3.33×** |

> Tandaan: Sa P2P dual-source mode, ang mga European node `s1` ay nagtutulungan para sa seeding, at ang cross-continental backbone RTT ay 65 ms.

### Mga pangunahing tampok ng produkto

- **Public key addressing automatic hole punching**: Hindi na kailangan ng central server at pampublikong network address, awtomatikong NAT hole punching batay sa distributed network public key address.
- **Millisecond-level real-time na pag-synchronize at metadata awareness**: Batay sa native na pagsubaybay sa kaganapan ng file (`FSEvents` / `inotify` / `ReadDirectoryChangesW`), real-time na push ng mga pagdaragdag ng file, pagtanggal, mga pahintulot sa POSIX (`mode`) at millisecond-level modification time (<b id="); zero data block transmission kapag ang mga pahintulot o oras lamang ang binago.
- **BT-style na sabay-sabay na pag-download at pag-upload at pag-iskedyul ng regional affinity**: Hindi kailangang hintayin ng downloading node para ma-download ang buong halaga. Ang lahat ng mga block interval (`Op::Have`) na nakapasa sa 128-bit na hash check ay agad na naka-lock sa memory/pansamantalang pool para sa iba pang mga node na hilahin nang sabay-sabay. Kasama ng dynamic na PEX node exchange at slow node half preemption (Work-Stealing), ang cluster throughput ay na-maximize.
- **Anti-malicious node tampering at cluster poisoning immunity**: authoritative source list `root_hash` Self-consistent locking + mandatory hardware acceleration `gxhash128` digest verification bago ilagay ang bawat block sa disk at externally seeded; kung may nakitang tampered block, i-blackout ng millisecond-level na circuit breaker ang malisyosong node at awtomatikong mababawi mula sa tapat na pinagmulan.
- **FastCDC in-situ increment at maliit na file aggregation**: Tanging mga differential block ang inililipat para sa bahagyang pagbabago ng malalaking file at inilagay sa disk; napakalaking maliit na file memory packaging at prefix differential compression ay nag-aalis ng file-by-file round-trip overhead.
- **Pagpapagaling sa sarili pagkatapos ng pagkawala ng kuryente at pagpapatuloy pagkatapos ng pag-restart**: Ang mala-redis na RDB + AOF hybrid na format (`.s0_subs.bin`) ay ginagamit upang ipagpatuloy ang seeding status at listahan ng file, ang mga pang-araw-araw na pagbabago ay idinadagdag sa loob ng microseconds, at awtomatikong muling isinusulat at maayos na regular. Pagkatapos ma-restart ang server, awtomatikong itutulak ang mga offline na pagbabago sa seeding node.

## Teknikal na arkitektura at dokumentasyon

- **Dual plane decoupling protocol**:
  - **Control at real-time change surface (`iroh` QUIC / TCP, first byte `PROTO_VERSION` version handshake) **: Responsable para sa Ed25519 business card verification, permission verification, Swarm seed discovery, at suporta para sa `u64::MAX` level mass change streaming batching at `mzstd` compressed (`mzstd` real time compressed (`mzstd` id="6"> / `FsOp::Remove` / `FsOp::Meta` Tatlong primitive).
  - **Paibabaw ng pamamahagi ng data (native UDP + `compio` asynchronous I/O) **: 20B compact header, 18B TACK periodic feedback, BBRv3/Copa hybrid congestion control, RACK-TLP packet loss recovery, first-block Shannon entropy adaptive Zstd compression bypass, at <b id id.="2"> block end.
- **Matuto pa**:
  - [Ulat sa pagsukat ng pagganap at script ng pagpaparami](https://github.com/webc-site/sync0/blob/main/doc/zh/benchmark.md)
  - [Disenyo ng arkitektura ng system](https://github.com/webc-site/sync0/blob/main/doc/zh/design_v4.md)
  - [Binary Wire Protocol at Detalye ng Pagtitiyaga](https://github.com/webc-site/sync0/blob/main/doc/zh/proto.md)
  - [Pagpapatunay ng pagkakakilanlan, kontrol ng awtoridad at pag-verify ng integridad](https://github.com/webc-site/sync0/blob/main/doc/zh/crypto.md)