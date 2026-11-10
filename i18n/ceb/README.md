# sync0

Ang `sync0` usa ka desentralisadong P2P file sharing, real-time nga pag-synchronize ug multi-source distribution tool nga gigamit sa pag-ilis sa `rsync` - ang gisukod nga katulin sa transcontinental WAN kay hangtod sa `rsync`'s **5.89×** (single point geometric average speed **2.67×**, P2P×an dual-source speed **2.67×**, P2P×an nga katulin sa transkontinente ****), high-latency link P2P acceleration kay hangtod sa **7.68×** (Full-link 24-scenario comprehensive geometric mean speed **2.56×**).

Ang matag makina usa ka server ug kliyente: wala’y kinahanglan alang sa usa ka sentral nga server o usa ka adres sa publiko nga network, ang awtomatik nga pagsuntok sa lungag gibase sa giapod-apod nga network sa publiko nga yawe nga adres, ug gisuportahan niini ang direkta nga transmission tali sa duha nga mga makina, pag-host sa pagtugot sa multi-sharing nga grupo, daghang pagpili sa mga subdirektoryo nga gipangayo, millisecond-level real-time incremental synchronization ug P2P multi-source concurrent a.

## Pag-instalar

### Pamaagi 1: Pag-instalar sa kargamento

```bash
cargo install sync0
```

### Pamaagi 2: Pag-download sa precompiled binaries

Adto sa [GitHub Releases](https://github.com/webc-site/sync0/releases) para i-download ang `.tar.xz` compressed package nga katugbang sa system (nagsuporta sa Linux, macOS, Windows, Android, iOS, FreeBSD), unzip ug ibutang sa `PATH`:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Pagsugod dayon

### 1. Zero-configuration direkta nga transmission tali sa duha ka makina (`send` / `recv`)

Dili kinahanglan nga i-configure ang usa ka grupo sa pagpaambit, direkta nga ibalhin ang mga file o direktoryo tali sa duha nga mga makina sa taas nga tulin:

```bash
# Ang nagdawat nga katapusan naminaw sa pantalan ug nagsulat sa ./backup
sync0 recv 0.0.0.0:9000 ./backup

# Direkta nga pagpasa gikan sa nagpadala (opsyonal --pagtangtang sa salamin aron limpyohan ang sobra nga mga file, --rate ang 50MB nga limitasyon sa tulin)
sync0 send ./project 192.168.1.10:9000
```

### 2. Pagpaambit sa post ug on-demand nga pull/upload (`share` / `pull` / `push`)

```bash
# Server: I-publish ang sharing group nga adunay usa ka linya sa command (i-output ang IP-free nga iroh node address human sa pagsugod)
sync0 share dataset:./data

# Kliyente: Ibira ang tibuok grupo sa pagpaambit sa dataset ngadto sa lokal nga ./local_data
sync0 pull dataset:// ./local_data -p <node address>

# Kliyente: Pagbitad sa daghang mga pagpili kung gipangayo (i-synchronize lang ang piho nga mga file ug subdirectory)
sync0 pull dataset://docs/readme.md,assets/icons,src/core ./local_data -p <node address>

# Kliyente: Pag-upload sa mga file o mga direktoryo sa awtorisado nga mga grupo sa pagpaambit
sync0 push dataset://releases/v1 ./build_output -p <node address>
```

### 3. Real-time nga subscription seeding ug multi-machine two-way synchronization (`sub` / `group`)

Human sa pag-establisar og taas nga koneksyon, kung adunay bisan unsang file ** pagdugang, pagbag-o, pagtangtang, pagbag-o sa mga permiso o oras sa pagbag-o ** mahitabo sa hilit nga tumoy, ang lokal nga direktoryo dungan nga ma-update sa millisecond sa tinuud nga oras (bag-o / gibag-o nga dagkong mga file mobira lamang sa mga bloke sa kalainan ug i-update kini sa lugar, ang pagtangtang sa mga file awtomatiko nga limpyohan ang mga lokal nga oras ug wala’y sulod nga mga direktoryo, ug direkta nga magbag-o sa mga permiso sa mga zero.

- **`sync0 sub`(One-way real-time mirroring + automatic seeding)**: Ang lokal nga real-time nagsunod sa mga pagdugang, pagtangtang ug mga kausaban sa sharing end, ug awtomatikong nagsilbi nga P2P seed source para makahatag ug dungan nga pagpatulin para sa ubang mga pulling node.
- **`sync0 group`(Multi-machine peer-to-peer two-way real-time synchronization)**: Kung ang mga file idugang, tangtangon, o usbon sa bisan unsang makina sa grupo, ang tanang ubang online node ma-update nga dungan sa tinuod nga panahon ug magsilbi nga mga liso sa usag usa.

```bash
# Mag-subscribe sa tibuok nga grupo sa pagpaambit: ang mga hilit nga pagdugang, pagtangtang, ug mga pagbag-o gi-synchronize sa lokal sa tinuod nga panahon ug awtomatik nga gisabwag
sync0 sub dataset:// ./local_mirror -p <node address>

# On-demand nga multi-select nga suskrisyon: pag-synchronize lang ug gipiho nga binhi ang usa ka file ug subdirectory sa tinuud nga oras
sync0 sub dataset://models/llama.onnx,configs ./local_mirror -p <node address>

# Multi-machine peer group two-way real-time synchronization (nagsuporta usab sa full o comma multi-select nga mga dalan)
sync0 group team-docs:// ./docs -p <node address>
```

### 4. Ang grupo sa multi-sharing nga grupo ug pagtugot sa miyembro nga configuration (`.nt` configuration)

Ang default configuration directory mao ang `~/.config/sync0` (mahimong ibalhin pinaagi sa `-c <directory>`), gamit ang minimalist [NestedText](https://nestedtext.org) (`.nt`) format:
- **Gisuportahan ang pagpugong sa pagbasa sa publiko nga network (ang awtorisado nga mga tiggamit lamang ang makabasa)**: Hangtud nga ang grupo sa pagpaambit gi-configure nga adunay `users:` lista sa miyembro (o ang linya sa command nagtino sa `--allow-r`=" / `--allow-rw` / `--allow-w` / `--auth`), ang sistema **awtomatikong gi-on ug gipugngan sa publiko (`users` publiko ug read-only pinaagi sa default (kung gusto nimo nga mahimong publiko ug read-only ug bukas sa pag-upload lamang sa mga espisipikong miyembro, mahimo nimong klaro nga itakda ang `auth: false`).
- **Fine-grained read-write ug subdirectory delivery permissions**: Ang mga miyembro kay read-only pinaagi sa default (`r` dili kinahanglang isulat nga klaro), ug suportahan ang `r` (read-only), `w` (isulat lang sa drop box, dili mabasa), <b id = "iwrite">" prefix nga gitugotan sa pag-upload).

**Ehemplo sa pagsumpo sa miyembro (`~/.config/sync0/users/`):**

```yaml
# ~/.config/sync0/users/alice.nt (default read-only, speed limit 80MB)
id: 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c
rate: 80MB
```

```yaml
# ~/.config/sync0/users/bob.nt (pagbasa ug pagsulat permiso, gitugotan lang sa pag-upload sa espesipikong subdirectory)
id: 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f
perm: rw
write_prefix:
  - uploads/bob
  - shared/common
```

```yaml
# ~/.config/sync0/users/charlie.nt (isulat lang ang pagtugot sa pagpadala, gidili ang pagbasa)
id: 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
perm: w
```

**Pagpaambit nga panig-ingnan sa configuration sa grupo (`~/.config/sync0/config.nt`):**

```yaml
users_dir: ./users

groups:
  # Dili gi-configure nga mga tiggamit: default public network read-only
  dataset:
    path: ./data
    rate: 50MB

  # Gi-configure nga mga tiggamit: awtomatik nga higpitan ang pagbasa sa publiko nga network (si alice ug bob ra ang makabasa, si charlie ra ang maghatud ug mag-upload)
  team-vault:
    path: ./vault
    users:
      - alice
      - bob
      - charlie
```

Pagsugod sa serbisyo (pag-load sa configuration file o direkta nga paggamit sa mga parameter sa command line aron higpitan ang pagbasa sa publiko nga network ug pagtugot sa mga miyembro):

```bash
# Ikarga ~/.config/sync0/config.nt para magsugod
sync0 share --region SHA

# O imantala ang usa ka pribado nga grupo sa pagpakigbahin pinaagi sa lunsay nga command line (specify --allow-* aron awtomatikong higpitan ang pagbasa sa publiko nga network ug ang mga awtorisado nga tiggamit lamang ang maka-access niini)
sync0 share team-vault:./vault \
  --allow-r 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c \
  --allow-rw 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f:uploads/bob,shared/common \
  --allow-w 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
```

### 5. Node address, business card ug multi-language

```bash
# Tan-awa ang lokal nga IP-free node nga adres (awtomatiko nga gihimo ang yawe alang sa una nga pagdagan)
sync0 id

# Paghimo Ed25519 self-signed node business card (nickname, city code, email, website, introduction)
sync0 profile -u alice --region SHA --email alice@example.com -b "Shanghai node"

# Adunay 75 ka built-in nga mga pinulongan, nga nagsunod sa sistema nga pinulongan pinaagi sa default ug mahimong ibalhin pinaagi sa environment variables.
SYNC0_LANG=en sync0 --help
```

## Pagganap ug kinauyokan nga mga bahin

### Aktuwal nga pagsukod sa cross-continental WAN (Pagtandi `rsync`)

Aktuwal nga sukod sa palibot: United States `c1` → Europe `n1` (RTT 65 ms), ang data set mao ang Linux kernel `linux-7.3.3`:

#### 1) A la carte mode vs `rsync`

| Set sa datos | gidak-on | `sync0` Usa ka Punto<br>(Mbps) | `rsync`<br>(Mbps) | Speedup (`sync0` / `rsync`) |
| :--- | ---: | ---: | ---: | ---: |
| `linux-7.3.3.tar.xz`(dili ma-compress) | 155 MB | **236.48** | 108.74 | **2.17×** |
| `linux-7.3.3.tar`(makompres) | 1.6 GB | **843.83** | 385.06 | **2.19×** |
| `linux-7.3.3.tar`(lokal nga pag-usbaw sa kausaban) | 1.6 GB | **8,014.15** | 1,361.72 | **5.89×** |
| `linux-7.3.3/` (95,979 ka gagmay nga mga file) | 1.5 GB | **627.58** | 347.80 | **1.80×** |
| ** geometriko nga kahulogan ** | — | **1,000.91** | 375.26 | **2.67×** |

#### 2) P2P dual-source mode vs `rsync`

| Set sa datos | gidak-on | `sync0` P2P Dual Source<br>(Mbps) | `rsync`<br>(Mbps) | Speedup (`sync0` / `rsync`) |
| :--- | ---: | ---: | ---: | ---: |
| `linux-7.3.3.tar.xz`(dili ma-compress) | 155 MB | **330.09** | 108.74 | **3.04×** |
| `linux-7.3.3.tar`(makompres) | 1.6 GB | **1,551.86** | 385.06 | **4.03×** |
| `linux-7.3.3.tar`(lokal nga pag-usbaw sa kausaban) | 1.6 GB | **5,474.17** | 1,361.72 | **4.02×** |
| `linux-7.3.3/` (95,979 ka gagmay nga mga file) | 1.5 GB | **706.56** | 347.80 | **2.03×** |
| ** geometriko nga kahulogan ** | — | **1,186.42** | 375.26 | **3.16×** |

> Mubo nga sulat: Sa P2P dual-source mode, ang European nodes `s1` magtinabangay para sa pagpugas, ug ang cross-continental backbone RTT kay 65 ms.

### Panguna nga mga bahin sa produkto

- **Pampubliko nga yawe nga nagtubag sa awtomatik nga pagsuntok sa lungag**: Dili kinahanglan ang usa ka sentro nga server ug ang adres sa publikong network, awtomatik nga NAT hole punching base sa giapod-apod nga network sa publiko nga yawe nga adres.
- **Millisecond-level real-time synchronization ug metadata awareness**: Base sa native file event monitoring (`FSEvents` / `inotify` / `ReadDirectoryChangesW`), real-time nga pagduso sa mga pagdugang sa file, pagtangtang, POSIX permiso (`mode`) ug millisecond-level modification time (`mtime`); zero data block transmission kung ang mga pagtugot o oras ra ang giusab.
- **BT-style nga dungan nga pag-download ug pag-upload ug regional affinity scheduling**: Ang downloading node dili kinahanglan nga maghulat sa tibuok kantidad nga ma-download. Ang tanang block intervals (`Op::Have`) nga nakapasar sa 128-bit hash check gi-lock dayon sa memory/temporary pool para sa ubang mga node nga dungan nga mabira. Uban sa dinamikong PEX node exchange ug hinay nga node half preemption (Work-Stealing), ang cluster throughput gi-maximize.
- **Anti-malisyoso nga node tampering ug cluster poisoning immunity**: authoritative source list `root_hash` Self-consistent locking + mandatory hardware acceleration `gxhash128` digest verification sa dili pa ang matag block ibutang sa disk ug externally seeded; kung makit-an ang usa ka tampered block, ang millisecond-level circuit breaker mo-blackout sa malisyosong node ug awtomatik nga mabawi gikan sa matinud-anon nga tinubdan.
- **FastCDC in-situ increment ug gamay nga file aggregation**: Ang differential blocks lang ang gibalhin para sa partial modification sa dagkong files ug gibutang sa disk; dako nga gamay nga file memory packaging ug prefix differential compression nagwagtang sa file-by-file round-trip overhead.
- **Pag-ayo sa kaugalingon pagkahuman sa pagkawala sa kuryente ug pagpadayon pagkahuman sa pag-restart**: Ang sama sa Redis nga RDB + AOF hybrid nga format (`.s0_subs.bin`) gigamit aron ipadayon ang kahimtang sa pagpugas ug lista sa file, ang adlaw-adlaw nga pagbag-o gidugang sa mga microsecond, ug awtomatiko nga gisulat ug giorganisar kanunay. Human ma-restart ang server, ang mga pagbag-o sa offline awtomatik nga iduso ngadto sa seeding node.

## Teknikal nga arkitektura ug dokumentasyon

- **Dual plane decoupling protocol**:
  - **Control ug real-time change surface (`iroh` QUIC / TCP, first byte `PROTO_VERSION` version handshake) **: Responsable para sa Ed25519 business card verification, permiso sa permiso, Swarm seed discovery, ug suporta para sa `u64::MAX` level mass change streaming batching ug `mzstd` compressed="`mzstd` id="6"> / `FsOp::Remove` / `FsOp::Meta` Tulo ka primitibo).
  - **Datas distribution surface (lumad nga UDP + `compio` asynchronous I/O) **: 20B compact header, 18B TACK periodic feedback, BBRv3/Copa hybrid congestion control, RACK-TLP packet loss recovery, first-block Shannon entropy adaptive Zstd compression bypass, ug <b id id.="2"> block end.
- **Pagkat-on pa**:
  - [Report sa pagsukod sa performance ug script sa reproduksyon](https://github.com/webc-site/sync0/blob/main/doc/zh/benchmark.md)
  - [Disenyo sa arkitektura sa sistema](https://github.com/webc-site/sync0/blob/main/doc/zh/design_v4.md)
  - [Binary Wire Protocol ug Persistence Specification](https://github.com/webc-site/sync0/blob/main/doc/zh/proto.md)
  - [Pagpamatuod sa identidad, pagkontrol sa awtoridad ug pag-verify sa integridad](https://github.com/webc-site/sync0/blob/main/doc/zh/crypto.md)