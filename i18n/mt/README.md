# sync0

`sync0` hija għodda deċentralizzata għall-qsim ta' fajls P2P, sinkronizzazzjoni f'ħin reali u distribuzzjoni b'ħafna sorsi użati biex jissostitwixxu `rsync` - il-veloċità mkejla tal-WAN transkontinentali hija sa `rsync`'s **5.89×** (veloċità medja ġeometrika ta' punt wieħed **2.67×**, P2P medja veloċità ** **cea 3o ), 3o ġeometrika **-cea . L-aċċelerazzjoni P2P tal-link b'latenza għolja hija sa **7.68×** (Veloċità medja ġeometrika komprensiva ta' 24 xenarju b'rabta sħiħa **2.56×**).

Kull magna hija kemm server kif ukoll klijent: m'hemmx bżonn ta 'server ċentrali jew indirizz tan-netwerk pubbliku, l-ippanċjar awtomatiku tat-toqob huwa bbażat fuq l-indirizz taċ-ċavetta pubblika tan-netwerk distribwit, u jappoġġja trasmissjoni diretta bejn żewġ magni, permess ta' grupp ta 'kondiviżjoni multipla, għażla multipla ta' sottodirettorji fuq talba, sinkronizzazzjoni inkrementali f'ħin reali f'livell ta 'millisekondi u aċċelerazzjoni konkorrenti P2P b'ħafna sorsi.

## Installa

### Metodu 1: Installazzjoni tal-merkanzija

```bash
cargo install sync0
```

### Metodu 2: Niżżel binarji kkumpilati minn qabel

Mur f'[GitHub Releases](https://github.com/webc-site/sync0/releases) biex tniżżel il-`.tar.xz` pakkett ikkompressat li jikkorrispondi mas-sistema (jappoġġja Linux, macOS, Windows, Android, iOS, FreeBSD), unzip u poġġi f'`PATH`:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Ibda malajr

### 1. Trażmissjoni diretta b'konfigurazzjoni żero bejn żewġ magni (`send` / `recv`)

M'hemmx bżonn li jiġi kkonfigurat grupp ta' qsim, tittrasferixxi direttament fajls jew direttorji bejn żewġ magni b'veloċità għolja:

```bash
# It-tarf li jirċievi jisma' fuq il-port u jikteb lil ./backup
sync0 recv 0.0.0.0:9000 ./backup

# Trażmissjoni diretta minn min jibgħat (fakultattiv --ħassar il-mera biex tnaddaf il-fajls żejda, --rata limitu ta 'veloċità ta' 50MB)
sync0 send ./project 192.168.1.10:9000
```

### 2. Post kondiviżjoni u pull/upload on-demand (`share` / `pull` / `push`)

```bash
# Server: Ippubblika l-grupp tal-qsim b'linja waħda ta' kmand (ħruġ l-indirizz tan-node iroh mingħajr IP wara l-istartjar)
sync0 share dataset:./data

# Klijent: Iġbed il-grupp kollu tal-kondiviżjoni tas-sett tad-dejta għall-./local_data lokali
sync0 pull dataset:// ./local_data -p <indirizz tan-node>

# Klijent: Iġbed selezzjonijiet multipli fuq talba (sinkronizza biss fajls u sottodirettorji speċifiċi speċifikati)
sync0 pull dataset://docs/readme.md,assets/icons,src/core ./local_data -p <indirizz tan-node>

# Klijent: Tella' fajls jew direttorji għal gruppi ta' qsim awtorizzati
sync0 push dataset://releases/v1 ./build_output -p <indirizz tan-node>
```

### 3. Seeding ta' abbonament f'ħin reali u sinkronizzazzjoni f'żewġ direzzjonijiet b'ħafna magni (`sub` / `group`)

Wara li tiġi stabbilita konnessjoni twila, jekk xi fajl **żieda, modifika, tħassir, tibdil ta' permessi jew ħin ta' modifika** iseħħ fit-tarf remot, id-direttorju lokali se jiġi aġġornat b'mod sinkroniku f'millisekondi f'ħin reali (fajls kbar ġodda/modifikati se jiġbdu biss il-blokki tad-differenza u jaġġornawhom f'posthom, ħassar fajls awtomatikament inaddfu fajls lokali u direttorji vojta, u jbiddlu direttament id-data ta 'blokki ta' permess żero, jew timmarka biss):

- **`sync0 sub`(One-way reali-time mirroring + seeding awtomatiku)**: Il-ħin reali lokali jsegwi ż-żidiet, tħassir u modifiki tat-tarf tal-qsim, u awtomatikament iservi bħala sors taż-żerriegħa P2P biex jipprovdi aċċelerazzjoni konkorrenti għal nodi oħra tal-ġbid.
- **`sync0 group`(Multi-machine peer-to-peer two-way f'ħin reali sinkronizzazzjoni)**: Jekk fajls huma miżjuda, imħassra, jew modifikati fuq kwalunkwe magna fil-grupp, in-nodi online l-oħra kollha se jiġu aġġornati b'mod sinkroniku f'ħin reali u jservu bħala żrieragħ għal xulxin.

```bash
# Abbona għall-grupp kollu tal-kondiviżjoni: żidiet remoti, tħassir u modifiki huma sinkronizzati lokalment f'ħin reali u awtomatikament miżrugħa
sync0 sub dataset:// ./local_mirror -p <indirizz tan-node>

# Abbonament b'għażla multipla fuq talba: jissinkronizza u żerriegħa biss fajls u sottodirettorji speċifiċi speċifikati f'ħin reali
sync0 sub dataset://models/llama.onnx,configs ./local_mirror -p <indirizz tan-node>

# Sinkronizzazzjoni f'ħin reali b'diversi magni tal-gruppi tal-pari (jappoġġja wkoll mogħdijiet b'għażla multipla sħiħa jew virgola)
sync0 group team-docs:// ./docs -p <indirizz tan-node>
```

### 4. Konfigurazzjoni tal-permess tal-grupp ta' multi-sharing tat-tim u tal-membri (`.nt` konfigurazzjoni)

Id-direttorju tal-konfigurazzjoni default huwa `~/.config/sync0` (jista' jinbidel permezz ta' `-c <directory>`), bl-użu tal-format minimalista [NestedText](https://nestedtext.org) (`.nt`):
- **Jappoġġja r-restrizzjoni tal-qari tan-netwerk pubbliku (l-utenti awtorizzati biss jistgħu jaqraw)**: Sakemm il-grupp tal-kondiviżjoni jkun ikkonfigurat b'lista ta' membri `users:` (jew il-linja tal-kmand tispeċifika `--allow-r` / `--allow-rw` / `--allow-w` / `--auth`), is-sistema **awtomatikament tixgħel netwerk privat awtentiku = "identiku"> l-enumerazzjoni jew il-qari tan-nodi, u P2P Abbonament għal nodi taż-żrigħ se jirtu wkoll il-lista l-bajda fl-istess ħin, li jippermetti biss lill-utenti awtorizzati jaqraw; gruppi ta' qsim li mhumiex konfigurati b'`users` huma pubbliċi u jinqraw biss b'mod awtomatiku (jekk trid tkun pubblika u tinqara biss u miftuħa għal uploads biss għal membri speċifikati, tista' tissettja espliċitament `auth: false`).
- **Permessi ta' qari-kitba u ta' konsenja ta' subdirectory**: Il-Membri jinqraw biss b'mod awtomatiku (`r` m'għandux għalfejn jinkiteb b'mod espliċitu), u jappoġġaw `r` (qari biss), `w` (ikteb biss fil-kaxxa tad-drop, ma jinqarax), `rw` u jaqra `rw`"dirett (prefiss id="write">) permess għat-tlugħ).

**Eżempju ta' konfigurazzjoni ta' membru (`~/.config/sync0/users/`):**

```yaml
# ~/.config/sync0/users/alice.nt (default read-only, limitu tal-veloċità 80MB)
id: 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c
rate: 80MB
```

```yaml
# ~/.config/sync0/users/bob.nt (permessi għall-qari u l-kitba, permessi biss li jtellgħu fis-sottodirettorju speċifikat)
id: 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f
perm: rw
write_prefix:
  - uploads/bob
  - shared/common
```

```yaml
# ~/.config/sync0/users/charlie.nt (il-permess tal-kunsinna tal-kitba biss, il-qari huwa pprojbit)
id: 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
perm: w
```

**Eżempju ta' konfigurazzjoni tal-grupp ta' kondiviżjoni (`~/.config/sync0/config.nt`):**

```yaml
users_dir: ./users

groups:
  # Utenti mhux konfigurati: netwerk pubbliku default read-only
  dataset:
    path: ./data
    rate: 50MB

  # Utenti kkonfigurati: jirrestrinġu awtomatikament il-qari tan-netwerk pubbliku (alice u bob biss jistgħu jaqraw, charlie jista 'biss jagħti u jtella')
  team-vault:
    path: ./vault
    users:
      - alice
      - bob
      - charlie
```

Ibda s-servizz (tagħbija l-fajl tal-konfigurazzjoni jew uża direttament il-parametri tal-linja tal-kmand biex tirrestrinġi l-qari tan-netwerk pubbliku u tawtorizza lill-membri):

```bash
# Tagħbija ~/.config/sync0/config.nt biex tibda
sync0 share --region SHA

# Jew tippubblika grupp ta' qsim privat permezz ta' linja ta' kmand pura (speċifika --allow-* biex tirrestrinġi awtomatikament il-qari tan-netwerk pubbliku u l-utenti awtorizzati biss jistgħu jaċċessawha)
sync0 share team-vault:./vault \
  --allow-r 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c \
  --allow-rw 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f:uploads/bob,shared/common \
  --allow-w 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
```

### 5. Indirizz tan-node, business card u multi-lingwa

```bash
# Ara l-indirizz tan-node lokali mingħajr IP (iċ-ċavetta hija ġġenerata awtomatikament għall-ewwel ġirja)
sync0 id

# Iġġenera karta tan-negozju tal-node Ed25519 iffirmata waħedha (laqam, kodiċi tal-belt, email, websajt, introduzzjoni)
sync0 profile -u alice --region SHA --email alice@example.com -b "Shanghai node"

# Hemm 75 lingwa integrata, li jsegwu l-lingwa tas-sistema awtomatikament u jistgħu jinqalbu permezz ta' varjabbli ambjentali.
SYNC0_LANG=en sync0 --help
```

## Prestazzjoni u karatteristiċi ewlenin

### Kejl attwali tal-WAN transkontinentali (Tqabbil `rsync`)

Ambjent tal-kejl attwali: L-Istati Uniti `c1` → Ewropa `n1` (RTT 65 ms), is-sett tad-dejta huwa Linux kernel `linux-7.3.3`:

#### 1) Modalità a la carte vs `rsync`

| Sett tad-dejta | daqs | `sync0` Punt Uniku<br>(Mbps) | `rsync`<br>(Mbps) | Tħaffef (`sync0` / `rsync`) |
| :--- | ---: | ---: | ---: | ---: |
| `linux-7.3.3.tar.xz`(mhux kompressabbli) | 155 MB | **236.48** | 108.74 | **2.17×** |
| `linux-7.3.3.tar`(kompressibbli) | 1.6 GB | **843.83** | 385.06 | **2.19×** |
| `linux-7.3.3.tar`(inkrement tal-bidla lokali) | 1.6 GB | **8,014.15** | 1,361.72 | **5.89×** |
| `linux-7.3.3/` (95,979 fajl żgħir) | 1.5 GB | **627.58** | 347.80 | **1.80×** |
| **medja ġeometrika** | — | **1,000.91** | 375.26 | **2.67×** |

#### 2) Modalità P2P b'sors doppju vs `rsync`

| Sett tad-dejta | daqs | `sync0` Sors Doppju P2P<br>(Mbps) | `rsync`<br>(Mbps) | Tħaffef (`sync0` / `rsync`) |
| :--- | ---: | ---: | ---: | ---: |
| `linux-7.3.3.tar.xz`(mhux kompressabbli) | 155 MB | **330.09** | 108.74 | **3.04×** |
| `linux-7.3.3.tar`(kompressibbli) | 1.6 GB | **1,551.86** | 385.06 | **4.03×** |
| `linux-7.3.3.tar`(inkrement tal-bidla lokali) | 1.6 GB | **5,474.17** | 1,361.72 | **4.02×** |
| `linux-7.3.3/` (95,979 fajl żgħir) | 1.5 GB | **706.56** | 347.80 | **2.03×** |
| **medja ġeometrika** | — | **1,186.42** | 375.26 | **3.16×** |

> Nota: Fil-modalità P2P b'sors doppju, in-nodi Ewropej `s1` jikkollaboraw għaż-żrigħ, u s-sinsla transkontinentali RTT hija 65 ms.

### Karatteristiċi ewlenin tal-prodott

- **Ċavetta pubblika li tindirizza l-ippanċjar awtomatiku tat-toqob**: M'hemmx bżonn ta 'server ċentrali u indirizz tan-netwerk pubbliku, ippanċjar awtomatiku tat-toqob NAT ibbażat fuq l-indirizz taċ-ċavetta pubblika tan-netwerk distribwit.
- **Sinkronizzazzjoni f'ħin reali f'livell ta 'millisekondi u għarfien tal-metadejta**: Ibbażat fuq monitoraġġ tal-avvenimenti tal-fajl indiġeni (`FSEvents` / `inotify` / `ReadDirectoryChangesW`), spinta f'ħin reali ta' żidiet ta 'fajls, tħassir, permessi POSIX (`mode`) u ħin ta' modifika fil-livell ta 'millisekondi (`mtime`); trasmissjoni ta' blokk tad-data żero meta l-permessi jew il-ħin biss jiġu modifikati.
- **Tniżżil u upload simultanju ta' stil BT u skedar ta' affinità reġjonali**: In-nodu tat-tniżżil m'għandux għalfejn jistenna biex jitniżżel l-ammont sħiħ. L-intervalli tal-blokki kollha (`Op::Have`) li għaddew mill-kontroll tal-hash 128-bit huma immedjatament msakkra fil-memorja/pool temporanju għal nodi oħra biex jiġbdu fl-istess ħin. Flimkien ma 'skambju dinamiku ta' nodi PEX u nofs preemption ta 'node bil-mod (Serq tax-Xogħol), il-fluss tal-cluster huwa massimizzat.
- **Tbagħbis ta 'nodi kontra l-malizzjużi u immunità ta' avvelenament ta 'cluster**: lista ta' sorsi awtorevoli `root_hash` Qfil awtokonsistenti + aċċelerazzjoni obbligatorja tal-hardware `gxhash128` verifika tad-diġest qabel kull blokka titqiegħed fuq id-diska u miżrugħa esternament; jekk tinstab blokk imbagħbsa, il-circuit breaker ta 'livell ta' millisekonda se blackout in-node malizzjuż u awtomatikament jirkupra mis-sors onest.
- **Inkrement FastCDC in-situ u aggregazzjoni ta 'fajls żgħar**: Blokki differenzjali biss huma trasferiti għal modifika parzjali ta' fajls kbar u mqiegħda fuq disk; l-imballaġġ massiv tal-memorja tal-fajl żgħir u l-kompressjoni differenzjali tal-prefiss jeliminaw l-overhead ta 'round-trip fajl b'fajl.
- **Awto-fejqan wara l-qtugħ tad-dawl u l-bidu mill-ġdid wara l-bidu mill-ġdid**: Il-format ibridu RDB + AOF bħal Redis (`.s0_subs.bin`) jintuża biex jippersisti l-istatus ta 'żrigħ u l-lista tal-fajls, il-bidliet ta' kuljum huma mehmuża f'mikrosekondi, u huma awtomatikament miktuba mill-ġdid u organizzati regolarment. Wara li s-server jerġa' jinbeda, il-bidliet offline jiġu awtomatikament imbuttati lejn in-nodu taż-żerriegħa.

## Arkitettura teknika u dokumentazzjoni

- **Protokoll ta' diżakkoppjar ta' pjan doppju**:
  - **Kontroll u wiċċ tal-bidla fil-ħin reali (`iroh` QUIC / TCP, l-ewwel byte `PROTO_VERSION` verżjoni handshake) **: Responsabbli għall-verifika tal-karta tan-negozju Ed25519, verifika tal-permess, skoperta ta’ żerriegħa Swarm, u appoġġ għal `u64::MAX` livell ta’ bidla tal-massa streaming batching u `mzstd` id="id="4"> ikkompressat (`mzstd` id="5" `mzstd` kompressa `FsOp::Remove` / `FsOp::Meta` Tliet primittivi).
  - **Wiċċ tad-distribuzzjoni tad-dejta (UDP nattiv + `compio` I/O asinkroniku) **: header kompatt 20B, feedback perjodiku 18B TACK, kontroll tal-konġestjoni ibridu BBRv3/Copa, irkupru ta 'telf ta' pakketti RACK-TLP, bypass tal-kompressjoni Zstd adattiv entropy Shannon tal-ewwel blokk, u <b id-="2"> end block verifika.
- **Tgħallem aktar**:
  - [Rapport tal-kejl tal-prestazzjoni u skript tar-riproduzzjoni](https://github.com/webc-site/sync0/blob/main/doc/zh/benchmark.md)
  - [Disinn tal-arkitettura tas-sistema](https://github.com/webc-site/sync0/blob/main/doc/zh/design_v4.md)
  - [Protokoll tal-Wajer Binarju u Speċifikazzjoni tal-Persistenza](https://github.com/webc-site/sync0/blob/main/doc/zh/proto.md)
  - [Awtentikazzjoni tal-identità, kontroll tal-awtorità u verifika tal-integrità](https://github.com/webc-site/sync0/blob/main/doc/zh/crypto.md)