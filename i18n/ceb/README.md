# sync0

`sync0` kay usa ka peer-to-peer (P2P) file sharing ug multi-machine synchronization tool nga nakigkompetensya sa Resilio Sync.

Ang matag makina usa ka server ug kliyente: ang bisan unsang node mahimong mag-publish sa daghang mga grupo sa pagpaambit sa file. Pagkahuman sa ubang mga node nga moapil, sila mahimo nga awtomatiko nga makakuha nga magamit nga mga node sa binhi gikan sa peer server ug ibira kini dungan gikan sa daghang mga gigikanan. Human makompleto ang pag-synchronize, ang mga liso awtomatik nga himoon alang sa dugang nga mga node aron mapadali ang pag-download.

## Panguna nga mga Highlight

- **Ang tanan usa ka server ug kliyente**: Ikatandi sa Resilio Sync, walay dependency sa sentral nga server. Ang bisan unsang node makahimo og daghang mga grupo sa pagpaambit sa mga file, ug mahimo usab nga moapil sa mga grupo sa pagpaambit sa ubang mga tawo; Awtomatikong magbayloay ug makakuha og mga node sa binhi aron makab-ot ang dungan nga pagbira ug real-time nga pag-synchronize sa daghang makina.
- **IP-free node address**: Sa default, ang node key ug `base64url` encoded `iroh` adres awtomatik nga namugna sa `~/.config/sync0` nga dili kinahanglan nga hinumdoman o ibutyag ang publikong IP.
- **Multi-select synchronization on demand**: Nagsuporta sa pag-synchronize sa tibuok grupo sa pagpaambit, ug nagsuporta usab sa synchronization on demand pinaagi sa pagpili sa daghang mga file o subdirectory sa samang higayon.
- **Mga permiso sa hierarchical nga membro sa grupo ug kontrol sa direktoryo sa pag-upload**: Nagsuporta sa read-only (`ro`), read-write upload (`rw`), ug mga permiso sa pagdumala (`admin`), ug mahimong tagsa-tagsa nga magpiho sa prefix sa dalan nga gitugotan sa matag miyembro nga i-upload (sa default, ang pag-upload sa bahin nga direktoryo gitugotan).
- **Full-speed ug incremental nga pag-synchronize sa mga network**: Kadungan nga pagpasa sa dagkong mga file, paghugpong sa dagkong gagmay nga mga file sa memorya, ug gi-synchronize lang nga mga bloke sa datos kung ang mga file partially modified.

## Ang pagtandi sa tulin sa rsync

### Transoceanic network single source aktuwal nga pagsukod (China → United States, delay 260 ms)

| Sitwasyon sa transmission | Gidak-on sa datos | modelo | `sync0` | `rsync -az` | Dugangi ang multiplier |
| :--- | :---: | :---: | :---: | :---: | :---: |
| 500 ka gagmay nga mga file | 1.91 MB | Bug-os nga pag-synchronize | 18.84 Mbps | 5.32 Mbps | 3.5x |
| 500 ka gagmay nga mga file | 1.91 MB | Dugang nga pag-synchronize | 27.15 Mbps | 3.83 Mbps | 7.1x |
| Code ug archive packages (`.tar`) | 10.0 MB | Bug-os nga pag-synchronize | 139.70 Mbps | 24.97 Mbps | 5.6x |
| Code ug archive packages (`.tar`) | 10.0 MB | Dugang nga pag-synchronize | 83.38 Mbps | 26.02 Mbps | 3.2x |
| Video file (`.mp4`) | 15.0 MB | Bug-os nga pag-synchronize | 54.06 Mbps | 27.04 Mbps | 2.0x |
| Video file (`.mp4`) | 15.0 MB | Dugang nga pag-synchronize | 216.83 Mbps | 21.51 Mbps | 10.1x |

### Transoceanic network P2P multi-node pull test (China → United States dual nodes, paglangan 260 ms)

Gisukod nga pagtandi sa multi-source seed node striped concurrent pull:

| Sitwasyon sa transmission | Gidak-on sa datos | Usa ka node seeding | Duha ka node nga P2P nga dungan nga pagpugas | Dugangi ang multiplier |
| :--- | :---: | :---: | :---: | :---: |
| 500 ka gagmay nga mga file | 1.97 MB | 5.28 Mbps | 7.87 Mbps | 1.5x |
| Code ug archive packages (`.tar`) | 30.0 MB | 56.35 Mbps | 83.17 Mbps | 1.5x |
| Video file (`.mp4`) | 30.0 MB | 27.00 Mbps | 39.46 Mbps | 1.5x |

### Aktuwal nga pagsukod sa transcontinental high-speed dedicated nga linya (Europe → North America, paglangan 86 ms)

| Sitwasyon sa transmission | Gidak-on sa datos | `sync0` | `rsync` | Dugangi ang multiplier |
| :--- | :---: | :---: | :---: | :---: |
| 10,000 ka gagmay nga mga file | 77.6 MB | 688 Mbps | 128 Mbps | 5.4x |
| Code ug archive packages (`.tar`) | 512 MB | 2111 Mbps | 438 Mbps | 4.8x |
| Video file (`.mp4`) | 512 MB | 556 Mbps | 338 Mbps | 1.6x |
| Dugang nga pag-synchronize (bag-ohon ang 1% nga sulud) | 512 MB | 34133 Mbps | 2214 Mbps | 15.4x |

## Pag-instalar

### Pamaagi 1: Pag-instalar pinaagi sa Cargo

```bash
cargo install sync0
```

### Pamaagi 2: Pag-download sa precompiled binaries

Adto sa [GitHub Releases](https://github.com/webc-site/sync0/releases) para i-download ang `.tar.xz` compressed package para sa katugbang nga sistema (nagsuporta sa Linux, macOS, Windows, Android, FreeBSD, NetBSD), unzip ug ibutang kini sa `PATH` directory:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Pagsugod dayon

`sync0` naggamit `~/.config/sync0` isip configuration directory sa default (kon kinahanglan ka nga mobalhin sa ubang mga direktoryo, mahimo nimong idugang ang `-c <directory>`).

### 1. Tan-awa ang lokal nga node address ug setting sa business card

```bash
# Tan-awa ang lokal nga node address (ang yawe awtomatik nga namugna sa ~/.config/sync0 sa unang higayon)
sync0 id

# Paghimo og personal nga node business card (nagsuporta sa user name, siyudad, email, website ug pirma sulod sa 256 ka pulong)
sync0 profile -u alice --region SHA --email alice@example.com --url https://example.com -b "Shanghai node"
```

### 2. I-publish ang daghang mga grupo sa pagpaambit sa file ug itakda ang mga pagtugot sa miyembro sa grupo (`.nt` Configuration)

`sync0` gi-configure sa [NestedText](https://nestedtext.org) (`.nt`) format. Ang matag user adunay lahi nga `.nt` configuration file. Mahimo nimong itakda ang direktoryo sa profile sa gumagamit pinaagi sa `users_dir` sa `config.nt` (nga awtomatiko nga naglakip sa tanan nga `.nt` mga file sa kini nga direktoryo). Ang user ug ang path prefix nga gitugotan nga ma-upload naggamit ug dili ordered list (`-`). **Kung ang prefix sa agianan sa pag-upload wala mapuno, ang pag-upload sa direktoryo sa ugat sa bahin gitugotan sa default **.

1. Paghimo og bulag nga `.nt` configuration files para sa matag miyembro sa `~/.config/sync0/users/` directory:

`~/.config/sync0/users/alice.nt` (read-only member, limitado ang rate `80MB`, `rate` nagsuporta sa `GB`, `MB`, `KB` units):

```yaml
id: 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c
perm: ro
rate: 80MB
```

`~/.config/sync0/users/bob.nt` (gipugngan nga mga miyembro sa pag-upload: gitugotan lang sa pag-upload sa `uploads/bob` ug `shared/common` mga direktoryo):

```yaml
id: 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f
perm: rw
write_prefix:
  - uploads/bob
  - shared/common
```

`~/.config/sync0/users/charlie.nt` (Default nga pag-upload sa miyembro: Kung ang `write_prefix` wala iapil, ang pag-upload sa root directory sa bahin gitugotan sa default):

```yaml
id: 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
perm: rw
```

2. I-configure ang direktoryo sa tiggamit ug daghang grupo sa pagpaambit sa mga file sa `~/.config/sync0/config.nt`:

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

Pagsugod sa serbisyo sa pagpaambit:

```bash
# Direkta nga pagsugod (awtomatikong nagkarga sa tanang .nt nga mga configuration sa user sa ~/.config/sync0/config.nt ug users_dir nga mga direktoryo)
sync0 share --region SHA

# O dali nga pagmantala sa daghang mga grupo sa pagpaambit ug pagtugot sa mga miyembro sa grupo direkta pinaagi sa linya sa mando
sync0 share dataset:./data team-vault:./vault \
  --auth \
  --allow-ro 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c \
  --allow-rw 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f:uploads/bob,shared/common \
  --allow-rw 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
```

### 3. Kuhaa ang tanang sulod o pagpili og daghang mga file ug mga direktoryo (`sync0 pull`)

Sumpaysumpaya ang bisan unsang peer node aron awtomatik nga makuha ang tanang available nga seed node sa sharing group, ug dungan nga kuhaa gikan sa daghang seed node:

```bash
# Ibira ang tibuok grupo sa pagpaambit sa dataset ngadto sa local ./local_data
sync0 pull dataset:// ./local_data -p <node address>

# Multi-pinili nga pag-synchronize: dungan nga pagbitad sa usa ka file docs/readme.md ug mga subdirectories assets/icons, src/core
sync0 pull dataset://docs/readme.md,assets/icons,src/core ./local_data -p <node address>
```

### 4. Pag-upload og file o direktoryo sa usa ka grupo sa pagpakigbahin (`sync0 push`)

Ang mga miyembro nga adunay pagtugot sa pag-upload (`rw` o `admin`) mahimong mag-upload sa mga lokal nga file o mga direktoryo sa layo nga mga grupo sa pagpaambit:

```bash
# Ang mga miyembro nga adunay pagtugot sa pag-upload sa gamut nga direktoryo: Direkta nga pag-upload sa gipaambit nga direktoryo sa ugat
sync0 push team-vault:// ./new_release -p <node address>

# Ang mga miyembro ra nga gitugotan sa usa ka piho nga prefix (sama sa mga pag-upload/bob): pag-upload sa gitugotan nga prefix sa agianan
sync0 push team-vault://uploads/bob/report.pdf ./report.pdf -p <node address>
```

### 5. Mag-subscribe sa awtomatikong pag-sync ug pagpugas (`sync0 sub`)

Human sa pag-subscribe sa sharing nga grupo sa usa ka node, ang lokal nga server kay nakadawat ug bag-ong seeding server; kung gi-update ang source node file, awtomatiko kini nga gi-synchronize ug naghatag mga chunked download alang sa ubang mga node:

```bash
# Mag-subscribe sa tibuok nga grupo sa pagpaambit ug awtomatiko nga binhi
sync0 sub dataset:// ./local_mirror -p <node address>

# Multi-select lang aron mag-subscribe sa mga piho nga mga file ug mga subdirectory ug liso kini
sync0 sub dataset://models/llama.onnx,configs ./local_mirror -p <node address>
```

### 6. Multi-machine group peer synchronization (`sync0 group`)

Daghang mga makina ang nag-apil sa parehas nga grupo sa pagpaambit, ang usag usa awtomatik nga nakadiskubre sa magamit nga mga node sa binhi ug nagmintinar sa duha ka paagi nga pag-synchronize (nagsuporta sa hingpit nga pag-synchronize o daghang pagpili sa mga file ug direktoryo):

```bash
# Hingpit nga moapil sa grupo ug duha ka paagi nga pag-synchronize
sync0 group team-docs:// ./docs -p <node address>

# Ang wal directory lang ug meta.json file sa multi-select synchronization group
sync0 group team-docs://wal,meta.json ./docs -p <node address>
```

### 7. Point-to-point direkta nga transmission tali sa duha ka makina (`send` / `recv`)

Dili kinahanglan nga pre-configure ang usa ka grupo, ug mahimo nimong temporaryo nga ibalhin ang mga file o direktoryo nga direkta sa taliwala sa duha nga mga makina:

```bash
# Pag-monitor sa tigdawat
sync0 recv 0.0.0.0:9000 ./backup

# Ang katapusan sa pagpadala direkta nga nagbalhin sa mga file o mga direktoryo (opsyonal --pagtangtang sa salamin aron limpyohan ang sobra nga mga file, --rate nga limitasyon sa tulin)
sync0 send ./project 192.168.1.10:9000
```

### 8. Pagbalhin sa daghang pinulongan

`sync0` nagsuporta sa 75 ka lengguwahe, ug awtomatik nga mobalhin sumala sa sistema sa pinulongan nga palibot pinaagi sa default; mahimo usab kining ipiho pinaagi sa mano-mano pinaagi sa `SYNC0_LANG`:

```bash
SYNC0_LANG=en sync0 --help
SYNC0_LANG=ja sync0 --help
SYNC0_LANG=de sync0 --help
```