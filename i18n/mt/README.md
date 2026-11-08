# sync0

`sync0` hija għodda ta' qsim ta' fajls bejn il-pari (P2P) u ta' sinkronizzazzjoni ta' diversi magni li tikkompeti ma' Resilio Sync.

Kull magna hija kemm server kif ukoll klijent: kull nodu jista 'jippubblika gruppi multipli ta' qsim ta 'fajls. Wara li nodi oħra jingħaqdu, jistgħu awtomatikament jiksbu nodi taż-żerriegħa disponibbli mis-server tal-pari u jiġbduhom fl-istess ħin minn sorsi multipli. Wara li titlesta s-sinkronizzazzjoni, iż-żrieragħ jinħolqu awtomatikament għal aktar nodi biex jaċċelleraw it-tniżżil.

## Punti ewlenin tal-qalba

- **Kulħadd huwa kemm server kif ukoll klijent**: Komparabbli ma' Resilio Sync, l-ebda dipendenza fuq server ċentrali. Kwalunkwe nodu jista 'joħloq gruppi ta' qsim ta 'fajls multipli, u jista' wkoll jingħaqad ma 'gruppi ta' qsim ta 'nies oħra; nodi awtomatikament jiskambjaw u jiksbu nodi taż-żerriegħa disponibbli biex jiksbu ġbid konkorrenti u sinkronizzazzjoni f'ħin reali ta 'magni multipli.
- **Indirizz tan-node mingħajr IP**: B'mod awtomatiku, iċ-ċavetta tan-node u l-indirizz `base64url` kodifikat `iroh` huma ġġenerati awtomatikament f'`~/.config/sync0` mingħajr il-ħtieġa li tiftakar jew tesponi l-IP pubbliku.
- **Sinkronizzazzjoni b'għażla multipla fuq talba**: Jappoġġja s-sinkronizzazzjoni tal-grupp kollu tal-qsim, u jappoġġja wkoll is-sinkronizzazzjoni fuq talba billi tagħżel fajls jew subdirettorji multipli fl-istess ħin.
- **Permessi ġerarkiċi tal-membri tal-grupp u kontroll tad-direttorju tat-tlugħ**: Jappoġġja permessi ta’ upload ta’ qari biss (`ro`), qari-kitba (`rw`), u ta’ ġestjoni (`admin`), u jista’ jispeċifika individwalment il-prefiss tal-passaġġ li kull membru jitħalla jtella’ (b’mod awtomatiku, it-tlugħ tad-direttorju tal-għeruq huwa permess).
- **Sinkronizzazzjoni b'veloċità sħiħa u inkrementali fin-netwerks**: Trażmissjoni konkorrenti ta' fajls kbar, aggregazzjoni ta' fajls żgħar kbar fil-memorja, u blokki tad-dejta sinkronizzati biss meta l-fajls jiġu modifikati parzjalment.

## Tqabbil tal-veloċità ma 'rsync

### Kejl attwali ta' sors wieħed tan-netwerk transoċeaniku (Ċina → Istati Uniti, dewmien ta' 260 ms)

| Xenarju ta' trażmissjoni | Daqs tad-dejta | mudell | `sync0` | `rsync -az` | Żid multiplikatur |
| :--- | :---: | :---: | :---: | :---: | :---: |
| 500 fajl żgħir | 1.91 MB | Sinkronizzazzjoni sħiħa | 18.84 Mbps | 5.32 Mbps | 3.5x |
| 500 fajl żgħir | 1.91 MB | Sinkronizzazzjoni inkrementali | 27.15 Mbps | 3.83 Mbps | 7.1x |
| Pakketti ta' kodiċi u arkivju (`.tar`) | 10.0 MB | Sinkronizzazzjoni sħiħa | 139.70 Mbps | 24.97 Mbps | 5.6x |
| Pakketti ta' kodiċi u arkivju (`.tar`) | 10.0 MB | Sinkronizzazzjoni inkrementali | 83.38 Mbps | 26.02 Mbps | 3.2x |
| Fajl tal-vidjo (`.mp4`) | 15.0 MB | Sinkronizzazzjoni sħiħa | 54.06 Mbps | 27.04 Mbps | 2.0x |
| Fajl tal-vidjo (`.mp4`) | 15.0 MB | Sinkronizzazzjoni inkrementali | 216.83 Mbps | 21.51 Mbps | 10.1x |

### Test tal-ġibda tan-netwerk transoċeaniku P2P b'ħafna nodi (Ċina → l-Istati Uniti nodi doppji, dewmien ta '260 ms)

Tqabbil imkejjel ta 'ġbid konkorrenti strixxat ta' nodi taż-żerriegħa b'ħafna sorsi:

| Xenarju ta' trażmissjoni | Daqs tad-dejta | Żrigħ ta' nodu wieħed | Żrigħ konkorrenti b'żewġ nodi P2P | Żid multiplikatur |
| :--- | :---: | :---: | :---: | :---: |
| 500 fajl żgħir | 1.97 MB | 5.28 Mbps | 7.87 Mbps | 1.5x |
| Pakketti ta' kodiċi u arkivju (`.tar`) | 30.0 MB | 56.35 Mbps | 83.17 Mbps | 1.5x |
| Fajl tal-vidjo (`.mp4`) | 30.0 MB | 27.00 Mbps | 39.46 Mbps | 1.5x |

### Kejl attwali ta' linja ddedikata transkontinentali ta' veloċità għolja (Ewropa → Amerika ta' Fuq, dewmien ta' 86 ms)

| Xenarju ta' trażmissjoni | Daqs tad-dejta | `sync0` | `rsync` | Żid multiplikatur |
| :--- | :---: | :---: | :---: | :---: |
| 10,000 fajl żgħir | 77.6 MB | 688 Mbps | 128 Mbps | 5.4x |
| Pakketti ta' kodiċi u arkivju (`.tar`) | 512 MB | 2111 Mbps | 438 Mbps | 4.8x |
| Fajl tal-vidjo (`.mp4`) | 512 MB | 556 Mbps | 338 Mbps | 1.6x |
| Sinkronizzazzjoni inkrementali (immodifika kontenut ta' 1%) | 512 MB | 34133 Mbps | 2214 Mbps | 15.4x |

## Installa

### Metodu 1: Installa permezz tal-Merkanzija

```bash
cargo install sync0
```

### Metodu 2: Niżżel binarji kkumpilati minn qabel

Mur f'[GitHub Releases](https://github.com/webc-site/sync0/releases) biex tniżżel il-`.tar.xz` pakkett ikkompressat għas-sistema korrispondenti (jappoġġja Linux, macOS, Windows, Android, FreeBSD, NetBSD), aqla u poġġih fid-direttorju `PATH`:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Ibda malajr

`sync0` juża `~/.config/sync0` bħala d-direttorju tal-konfigurazzjoni awtomatikament (jekk għandek bżonn taqleb għal direttorji oħra, tista 'żżid `-c <directory>`).

### 1. Ara l-indirizz tan-nodu lokali u l-karta tan-negozju tas-setting

```bash
# Ara l-indirizz tan-nodu lokali (iċ-ċavetta hija ġġenerata awtomatikament f'~/.config/sync0 għall-ewwel darba)
sync0 id

# Iġġenera karta tan-negozju tan-node personali (jappoġġja l-isem tal-utent, il-belt, l-email, il-websajt u l-firma fi żmien 256 kelma)
sync0 profile -u alice --region SHA --email alice@example.com --url https://example.com -b "Shanghai node"
```

### 2. Ippubblika gruppi multipli tal-qsim tal-fajls u ssettja l-permessi tal-membri tal-grupp (`.nt` Konfigurazzjoni)

`sync0` huwa kkonfigurat fil-format [NestedText](https://nestedtext.org) (`.nt`). Kull utent għandu fajl ta' konfigurazzjoni `.nt` separat. Tista' tispeċifika d-direttorju tal-profil tal-utent permezz ta' `users_dir` f'`config.nt` (li awtomatikament jinkludi l-`.nt` fajls kollha f'dan id-direttorju). Kemm l-utent kif ukoll il-prefiss tal-passaġġ permessi li jittellgħu jużaw lista mhux ordnata (`-`). **Meta l-prefiss tal-passaġġ tal-upload ma jimtelax, it-tlugħ fid-direttorju tal-għeruq tas-sehem huwa permess b'mod awtomatiku**.

1. Oħloq `.nt` fajls ta' konfigurazzjoni separati għal kull membru fid-direttorju `~/.config/sync0/users/`:

`~/.config/sync0/users/alice.nt` (membru li jinqara biss, rata limitata `80MB`, `rate` jappoġġja l-unitajiet `GB`, `MB`, `KB`):

```yaml
id: 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c
perm: ro
rate: 80MB
```

`~/.config/sync0/users/bob.nt` (membri ta' upload ristrett: permessi biss itellgħu fid-direttorji `uploads/bob` u `shared/common`):

```yaml
id: 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f
perm: rw
write_prefix:
  - uploads/bob
  - shared/common
```

`~/.config/sync0/users/charlie.nt` (Tlugħ tal-membri default: Meta `write_prefix` titħalla barra, it-tlugħ fid-direttorju tal-għeruq tas-sehem huwa permess b'mod awtomatiku):

```yaml
id: 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
perm: rw
```

2. Ikkonfigura d-direttorju tal-utenti u l-gruppi multipli tal-qsim tal-fajls f'`~/.config/sync0/config.nt`:

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

Ibda servizz ta' qsim:

```bash
# Startup dirett (awtomatikament jgħabbi l-konfigurazzjonijiet kollha tal-utent .nt fid-direttorji ~/.config/sync0/config.nt u users_dir)
sync0 share --region SHA

# Jew malajr tippubblika gruppi ta 'qsim multipli u jawtorizza lill-membri tal-grupp direttament permezz tal-linja tal-kmand
sync0 share dataset:./data team-vault:./vault \
  --auth \
  --allow-ro 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c \
  --allow-rw 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f:uploads/bob,shared/common \
  --allow-rw 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
```

### 3. Iġbed il-kontenut kollu jew agħżel fajls u direttorji multipli (`sync0 pull`)

Qabbad ma' kwalunkwe node peer biex tikseb awtomatikament in-nodi taż-żerriegħa kollha disponibbli fil-grupp tal-qsim, u iġbed minn nodi taż-żerriegħa multipli fl-istess ħin:

```bash
# Iġbed il-grupp kollu tal-kondiviżjoni tas-sett tad-dejta għall-./local_data lokali
sync0 pull dataset:// ./local_data -p <indirizz tan-node>

# Sinkronizzazzjoni multi-select: simultanjament iġbed il-fajl wieħed docs/readme.md u sottodirettorji assi/ikoni, src/core
sync0 pull dataset://docs/readme.md,assets/icons,src/core ./local_data -p <indirizz tan-node>
```

### 4. Tella' fajl jew direttorju fi grupp ta' qsim (`sync0 push`)

Membri b'permessi ta' upload (`rw` jew `admin`) jistgħu jtellgħu fajls jew direttorji lokali fi gruppi ta' kondiviżjoni mill-bogħod:

```bash
# Membri b'permess għat-tlugħ tad-direttorju tal-għeruq: Ittella direttament fid-direttorju tal-għeruq kondiviż
sync0 push team-vault:// ./new_release -p <indirizz tan-node>

# Membri awtorizzati biss għal prefiss speċifiku (bħal uploads/bob): ittella fil-prefiss tal-passaġġ permess
sync0 push team-vault://uploads/bob/report.pdf ./report.pdf -p <indirizz tan-node>
```

### 5. Abbona għas-sinkronizzazzjoni u t-tfigħ awtomatiku (`sync0 sub`)

Wara li jissottoskrivi għall-grupp ta 'qsim ta' ċertu node, is-server lokali huwa kemm klijent li jirċievi kif ukoll server ġdid ta 'seeding; meta l-fajl tan-node tas-sors jiġi aġġornat, huwa awtomatikament sinkronizzat u jipprovdi downloads imqassma għal nodi oħra:

```bash
# Abbona għall-grupp kollu tal-qsim u awtomatikament żerriegħa
sync0 sub dataset:// ./local_mirror -p <indirizz tan-node>

# Multi-select biss biex tabbona għal fajls u sottodirettorji speċifikati u żerriegħahom
sync0 sub dataset://models/llama.onnx,configs ./local_mirror -p <indirizz tan-node>
```

### 6. Sinkronizzazzjoni bejn il-pari tal-gruppi b'diversi magni (`sync0 group`)

Magni multipli jingħaqdu mal-istess grupp ta 'qsim, xulxin awtomatikament jiskopru nodi taż-żerriegħa disponibbli u jżommu sinkronizzazzjoni f'żewġ direzzjonijiet (jappoġġja sinkronizzazzjoni sħiħa jew għażla multipla ta' fajls u direttorji):

```bash
# Ingħaqad bis-sħiħ mal-grupp u sinkronizzazzjoni f'żewġ direzzjonijiet
sync0 group team-docs:// ./docs -p <indirizz tan-node>

# Id-direttorju wal u l-fajl meta.json biss fil-grupp ta 'sinkronizzazzjoni multi-select
sync0 group team-docs://wal,meta.json ./docs -p <indirizz tan-node>
```

### 7. Trażmissjoni diretta minn punt għal punt bejn żewġ magni (`send` / `recv`)

M'hemmx bżonn li tikkonfigura minn qabel grupp, u tista' tittrasferixxi temporanjament fajls jew direttorji direttament bejn żewġ magni:

```bash
# Monitoraġġ tar-riċevitur
sync0 recv 0.0.0.0:9000 ./backup

# It-tarf li jibgħat jittrasferixxi direttament fajls jew direttorji (fakultattiv --ħassar mera biex tnaddaf fajls żejda, --rata limitu tal-veloċità)
sync0 send ./project 192.168.1.10:9000
```

### 8. Qlib b'ħafna lingwi

`sync0` jappoġġja 75 lingwa, u awtomatikament jaqleb skont l-ambjent tal-lingwa tas-sistema b'mod awtomatiku; jista' jiġi speċifikat ukoll manwalment permezz ta' `SYNC0_LANG`:

```bash
SYNC0_LANG=en sync0 --help
SYNC0_LANG=ja sync0 --help
SYNC0_LANG=de sync0 --help
```