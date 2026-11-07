# sync0

Ang `sync0` usa ka himan sa command line para sa pagbalhin sa mga file sa mga network, padayon nga pag-synchronize sa mga direktoryo, ug dungan nga pag-apod-apod sa datos sa daghang mga makina.

## Orihinal nga katuyoan sa disenyo

Sa cross-machine nga mga lawak, mga nasud, o mga palibot nga adunay mga paglangan sa network ug pagkawala sa packet, ang mga himan sama sa `rsync` ug `scp` gibase sa usa ka pagkumpirma sa han-ay sa koneksyon, nga naglimite sa paggamit sa bandwidth; kung daghang mga makina ang mobira sa datos gikan sa parehas nga server sa parehas nga oras, ang source server network card dali nga mahimong usa ka bottleneck.

Ang tumong sa `sync0` mao ang pagsulbad sa duha ka problema:

1. Himoa nga ang file ug directory synchronization tali sa duha ka mga makina modagan sa bug-os nga pisikal nga bandwidth, ug ibalhin lamang ang nabag-o nga bahin kung ang file partially modified.
2. Ang daghang mga makina mahimong magbalhin sa mga bloke sa datos sa usag usa kung mag-synchronize sa parehas nga direktoryo, makunhuran ang load sa source server ug mub-an ang kinatibuk-ang oras sa pag-synchronize.

## Ang pagtandi sa tulin sa rsync

Pagsulay sa palibot: European node $\leftrightarrow$ North American node, round-trip delay 86 ms, pagsukod sa kompleto nga pag-usik sa panahon lakip na ang pag-scan sa direktoryo, pagtandi sa deduplication, transmission sa network ug pag-verify sa pagbutang sa disk:

| Sitwasyon sa transmission | Gidak-on sa datos | `sync0` | `rsync` | daghan |
| :--- | :---: | :---: | :---: | :---: |
| 10,000 ka gagmay nga mga file | 77.6 MB | 0.90 s (688 Mbps) | 4.86 s (128 Mbps) | 5.4x |
| Code ug archive packages (`.tar`) | 512 MB | 1.94 s (2111 Mbps) | 9.36 s (438 Mbps) | 4.8x |
| Video file (`.mp4`) | 512 MB | 7.36 s (556 Mbps) | 12.11 s (338 Mbps) | 1.6x |
| Dugang nga pag-synchronize (bag-ohon ang 1% nga sulud) | 512 MB | 0.12 s | 1.85 s | 15.4x |

## Mga pamaagi sa pagpaayo sa performance

- **UDP concurrent transmission**: Ipadala ang mga data block pinaagi sa UDP, ug i-transmit lang pag-usab ang nawala nga mga tipik aron malikayan ang usa ka packet loss nga makababag sa sunod nga mga pagbalhin sa file.
- **Pagbahin sa sulud ug pag-deduplication**: Bahina ang file sa mga bloke sa datos sumala sa sulud ug kuwentaha ang kantidad sa tseke. Ang parehas nga sulud nga naa na sa nakadawat nga katapusan gigamit usab direkta sa lokal nga wala’y pagdoble sa network.
- **Awtomatikong i-adjust ang compression sumala sa speed sa network**: I-on ang compression para sa ma-compress nga content sama sa text ug code; Awtomatikong laktawan ang compression para sa mga video, hulagway, compressed packages o ubos sa high-speed LAN aron makunhuran ang konsumo sa CPU.
- **Multi-node duol nga pagbira**: Kung daghang makina ang mobira sa samang direktoryo, ang mga bloke sa datos dungan nga kuhaon gikan sa na-download nga mga node sa samang siyudad o ubos ang latency.

## Pag-instalar

### Pamaagi 1: Pag-instalar pinaagi sa Cargo

```bash
cargo install sync0
```

### Pamaagi 2: Pag-download sa precompiled binaries

Adto sa [GitHub Releases](https://github.com/webc-site/sync0/releases) aron i-download ang `.tar.xz` compressed package para sa katugbang nga sistema (nagsuporta sa Linux x86_64/ARM64, macOS Apple Silicon/Intel, Windows x64/ARM64, Android ARM64/x86_64, FreeBSD, NetBSD). Human sa pag-unzip, ibutang ang `sync0` sa `PATH` direktoryo:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Giya sa Gumagamit

### 1. Direkta nga ibalhin ang mga file o direktoryo tali sa duha ka makina

Sugdi ang pag-monitor sa makina sa pagdawat ug ipiho ang agianan sa pagdawat ug pagtipig:

```bash
sync0 recv 0.0.0.0:9000 ./backup
```

Pagpadala usa ka file o usa ka tibuuk nga direktoryo gikan sa nagpadala nga makina:

```bash
sync0 send ./project 192.168.1.10:9000
```

Kasagaran nga gigamit nga mga parameter:

```bash
# Pagtangtang sa sobra nga mga file nga anaa sa nakadawat nga tumoy apan dili sa pagpadala (pag-synchronize sa salamin)
sync0 send ./project 192.168.1.10:9000 --delete

# Limitahan ang rate sa pagpadala (unit: bytes/second, pananglitan 50 MB/s)
sync0 send ./project 192.168.1.10:9000 --rate 50000000

# Silent mode (walay progress bar nga gipakita)
sync0 send ./project 192.168.1.10:9000 -q
```

### 2. Pag-publish sa gipaambit nga mga direktoryo ug pagbira gikan sa daghang mga node

Imantala ang gipaambit nga direktoryo (ginganlan `dataset`) sa gigikanang server:

```bash
sync0 share dataset ./data --region SHA
```

Kuhaa ang gipaambit nga direktoryo sa ubang mga makina (mahimo nimong pun-on ang daghang mga adres sa node sa parehas nga oras, gibulag sa mga koma aron dungan nga pagbitad):

```bash
sync0 pull 10.0.0.1:9000,10.0.0.2:9000 dataset ./local_data
```

Pagbitad lamang sa usa ka subdirectory sa gipaambit nga direktoryo:

```bash
sync0 pull 10.0.0.1:9000 dataset ./local_data --subpath 2026/logs
```

Iduso ang lokal nga mga pagbag-o balik sa hilit nga gipaambit nga direktoryo:

```bash
sync0 push 10.0.0.1:9000 dataset ./local_data
```

### 3. Mag-subscribe sa mga awtomatikong update sa katalogo

Mag-subscribe sa gipaambit nga direktoryo sa gigikanan nga node. Kung ang usa ka file gidugang o giusab sa gigikanan nga direktoryo, awtomatiko kini nga i-synchronize sa lokal nga kompyuter. Sa samang higayon, ang lokal nga makina awtomatik nga magsilbi nga node alang sa ubang mga makina nga ibira:

```bash
sync0 sub 10.0.0.1:9000 dataset ./local_mirror
```

### 4. Duha ka paagi nga pag-synchronize sa daghang mga grupo sa makina

Daghang mga makina ang moapil sa parehas nga ngalan sa grupo, ug ang mga pagbag-o sa bisan unsang direktoryo sa node awtomatiko nga i-synchronize sa ubang mga node sa grupo:

```bash
sync0 group team-docs ./docs 10.0.0.1:9000,10.0.0.2:9000
```

### 5. Node identity ug personal nga business card

Ang matag node nagsuporta sa pagtakda sa user name, siyudad, email, website ug usa ka personalized nga pirma (maximum nga 256 ka mga karakter), ug gipasa ang lokal nga pribado nga key signature authentication:

```bash
# Tan-awa ang lokal nga public key ID (ipiho ang configuration directory aron ipadayon ang yawe)
sync0 id -c ~/.config/sync0

# Paghimo ug pagpakita sa mga business card sa node nga adunay personal nga mga pirma
sync0 profile -c ~/.config/sync0 \
  -u alice \
  --region SHA \
  --email alice@example.com \
  --url https://example.com \
  -b "Shanghai backup node"
```

`share`, `pull`, `sub`, `group` mahimo usab nga ilunsad uban niining mga parametro sa profile:

```bash
sync0 share dataset ./data -c ~/.config/sync0 -u alice --region SHA -b "Shanghai backup node"
```

### 6. Pagbalhin sa daghang pinulongan

`sync0` Nagsuporta sa 75 ka mga pinulongan. Sa kasagaran, ang tabang sa command line ug mga prompt sa katugbang nga pinulongan awtomatik nga gipakita sumala sa palibot sa pinulongan sa operating system; kung ang sistema nga pinulongan wala sa gisuportahan nga lista, ang English gigamit nga default.

Para manu-mano ang pagtino sa pinulongan, itakda ang environment variable `SYNC0_LANG`:

```bash
SYNC0_LANG=en sync0 --help
SYNC0_LANG=ja sync0 --help
SYNC0_LANG=de sync0 --help
```