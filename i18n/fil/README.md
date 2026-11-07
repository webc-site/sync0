# sync0

Ang `sync0` ay isang command line tool para sa paglilipat ng mga file sa mga network, unti-unting pag-synchronize ng mga direktoryo, at sabay-sabay na pamamahagi ng data sa maraming machine.

## Orihinal na layunin ng disenyo

Sa mga cross-machine room, bansa, o environment na may mga pagkaantala sa network at pagkawala ng packet, ang mga tool gaya ng `rsync` at `scp` ay nakabatay sa iisang pagkumpirma ng sequence ng koneksyon, na naglilimita sa paggamit ng bandwidth; kapag ang maramihang mga makina ay humila ng data mula sa parehong server sa parehong oras, ang source server network card ay madaling maging isang bottleneck.

Ang layunin ng `sync0` ay lutasin ang dalawang problema:

1. Hayaang magpatakbo ng buong pisikal na bandwidth ang file at direktoryo sa pagitan ng dalawang makina, at ilipat lamang ang binagong bahagi kapag bahagyang binago ang file.
2. Maaaring ilipat ng maraming makina ang mga bloke ng data sa isa't isa kapag nagsi-synchronize sa parehong direktoryo, binabawasan ang pagkarga sa source server at pinaikli ang kabuuang oras ng pag-synchronize.

## Paghahambing ng bilis sa rsync

Kapaligiran ng pagsubok: European node $\leftrightarrow$ North American node, round-trip delay na 86 ms, sinusukat ang kumpletong pag-ubos ng oras kasama ang pag-scan ng direktoryo, paghahambing ng deduplikasyon, paghahatid ng network at pag-verify ng disk placement:

| Sitwasyon ng paghahatid | Laki ng data | `sync0` | `rsync` | maramihan |
| :--- | :---: | :---: | :---: | :---: |
| 10,000 maliliit na file | 77.6 MB | 0.90 s (688 Mbps) | 4.86 s (128 Mbps) | 5.4x |
| Code at archive packages (`.tar`) | 512 MB | 1.94 s (2111 Mbps) | 9.36 s (438 Mbps) | 4.8x |
| Video file (`.mp4`) | 512 MB | 7.36 s (556 Mbps) | 12.11 s (338 Mbps) | 1.6x |
| Incremental na pag-synchronize (baguhin ang 1% na nilalaman) | 512 MB | 0.12 s | 1.85 s | 15.4x |

## Mga paraan ng pagpapabuti ng pagganap

- **UDP concurrent transmission**: Magpadala ng mga bloke ng data sa pamamagitan ng UDP, at muling ipadala ang mga nawalang fragment upang maiwasan ang isang packet loss na humaharang sa mga kasunod na paglilipat ng file.
- **Pagse-segment at pag-deduplication ng content**: Hatiin ang file sa mga bloke ng data ayon sa content at kalkulahin ang check value. Ang parehong nilalaman na nasa receiving end ay direktang ginagamit nang lokal nang hindi duplicate ang network.
- **Awtomatikong isaayos ang compression ayon sa bilis ng network**: I-on ang compression para sa compressible na content gaya ng text at code; awtomatikong laktawan ang compression para sa mga video, larawan, naka-compress na pakete o sa ilalim ng high-speed LAN upang bawasan ang pagkonsumo ng CPU.
- **Multi-node na malapit na paghila**: Kapag maraming machine ang humila sa parehong direktoryo, ang mga bloke ng data ay sabay-sabay na kukunin mula sa mga na-download na node sa parehong lungsod o may mas mababang latency.

## I-install

### Paraan 1: I-install sa pamamagitan ng Cargo

```bash
cargo install sync0
```

### Paraan 2: Mag-download ng mga precompiled na binary

Pumunta sa [GitHub Releases](https://github.com/webc-site/sync0/releases) para i-download ang `.tar.xz` compressed package para sa kaukulang system (sumusuporta sa Linux x86_64/ARM64, macOS Apple Silicon/Intel, Windows x64/ARM64, Android ARM64/x86_64, FreeBSD, NetBSD). Pagkatapos mag-unzip, ilagay ang `sync0` sa direktoryo ng `PATH`:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Gabay sa Gumagamit

### 1. Direktang maglipat ng mga file o direktoryo sa pagitan ng dalawang makina

Simulan ang pagsubaybay sa receiving machine at tukuyin ang receiving at save path:

```bash
sync0 recv 0.0.0.0:9000 ./backup
```

Magpadala ng isang file o isang buong direktoryo mula sa sending machine:

```bash
sync0 send ./project 192.168.1.10:9000
```

Mga karaniwang ginagamit na parameter:

```bash
# Tanggalin ang mga redundant na file na umiiral sa receiving end ngunit hindi sa pagpapadala (mirror synchronization)
sync0 send ./project 192.168.1.10:9000 --delete

# Limitahan ang rate ng pagpapadala (unit: bytes/segundo, halimbawa 50 MB/s)
sync0 send ./project 192.168.1.10:9000 --rate 50000000

# Silent mode (walang progress bar na ipinapakita)
sync0 send ./project 192.168.1.10:9000 -q
```

### 2. Pag-publish ng mga nakabahaging direktoryo at paghila mula sa maraming node

Mag-publish ng nakabahaging direktoryo (pinangalanang `dataset`) sa pinanggalingang server:

```bash
sync0 share dataset ./data --region SHA
```

Hilahin ang nakabahaging direktoryo sa iba pang mga makina (maaari mong punan ang maraming node address nang sabay-sabay, na pinaghihiwalay ng mga kuwit upang hilahin nang sabay-sabay):

```bash
sync0 pull 10.0.0.1:9000,10.0.0.2:9000 dataset ./local_data
```

Hilahin lamang ang isang partikular na subdirectory sa nakabahaging direktoryo:

```bash
sync0 pull 10.0.0.1:9000 dataset ./local_data --subpath 2026/logs
```

Itulak ang mga lokal na pagbabago pabalik sa malayong nakabahaging direktoryo:

```bash
sync0 push 10.0.0.1:9000 dataset ./local_data
```

### 3. Mag-subscribe sa mga awtomatikong pag-update ng catalog

Mag-subscribe sa nakabahaging direktoryo ng source node. Kapag ang isang file ay idinagdag o binago sa pinagmulang direktoryo, awtomatiko itong masi-synchronize sa lokal na computer. Kasabay nito, awtomatikong magsisilbing node ang lokal na makina para hilahin ng iba pang makina:

```bash
sync0 sub 10.0.0.1:9000 dataset ./local_mirror
```

### 4. Two-way na pag-synchronize ng mga multi-machine group

Maraming machine ang sumali sa parehong pangalan ng grupo, at ang mga pagbabago sa anumang direktoryo ng node ay awtomatikong masi-synchronize sa iba pang mga node sa grupo:

```bash
sync0 group team-docs ./docs 10.0.0.1:9000,10.0.0.2:9000
```

### 5. Pagkakakilanlan ng node at personal na business card

Sinusuportahan ng bawat node ang pagtatakda ng user name, lungsod, email, website at isang personalized na lagda (maximum na 256 character), at ipinapasa ang lokal na pribadong key signature authentication:

```bash
# Tingnan ang lokal na pampublikong key ID (tukuyin ang direktoryo ng pagsasaayos upang mapanatili ang susi)
sync0 id -c ~/.config/sync0

# Bumuo at magpakita ng mga business card ng node na may mga personalized na lagda
sync0 profile -c ~/.config/sync0 \
  -u alice \
  --region SHA \
  --email alice@example.com \
  --url https://example.com \
  -b "Shanghai backup node"
```

Maaari ding ilunsad ang `share`, `pull`, `sub`, `group` gamit ang mga parameter ng profile na ito:

```bash
sync0 share dataset ./data -c ~/.config/sync0 -u alice --region SHA -b "Shanghai backup node"
```

### 6. Multi-language switching

`sync0` Sinusuportahan ang 75 wika. Bilang default, ang tulong sa command line at mga senyas sa kaukulang wika ay awtomatikong ipinapakita ayon sa kapaligiran ng wika ng operating system; kung ang wika ng system ay wala sa sinusuportahang listahan, Ingles ang ginagamit bilang default.

Upang manu-manong tukuyin ang wika, itakda ang environment variable `SYNC0_LANG`:

```bash
SYNC0_LANG=en sync0 --help
SYNC0_LANG=ja sync0 --help
SYNC0_LANG=de sync0 --help
```