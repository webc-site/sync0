# sync0

`sync0` is 'n gedesentraliseerde P2P-lêerdeling, intydse sinchronisasie en multi-bronverspreidingsnutsding wat gebruik word om `rsync` te vervang - die gemete spoed van die transkontinentale wye area netwerk is tot **6.95×** van `rsync` (enkelpunt geometriese gemiddelde spoed **P dual x**,× geometriese gemiddelde spoed **P 2.01×**,× ** en P2. die sinchronisasie van massiewe klein lêers oor oseane is so vinnig as **7.19×**.

Elke masjien is beide 'n bediener en 'n kliënt: daar is geen behoefte aan 'n sentrale bediener of 'n publieke netwerkadres nie, outomatiese gatpons is gebaseer op die verspreide netwerk publieke sleuteladres, en dit ondersteun direkte transmissie tussen twee masjiene, multi-deel groeptoestemming hosting, veelvuldige seleksie van subgidse op aanvraag, millisekonde-vlak intydse inkrementele sinchronisasie en P2P multi-bron versnelling.

## Installeer

### Metode 1: Vraginstallasie

```bash
cargo install sync0
```

### Metode 2: Laai vooraf saamgestelde binaries af

Gaan na [GitHub Releases](https://github.com/webc-site/sync0/releases) om die `.tar.xz` saamgeperste pakket af te laai wat ooreenstem met die stelsel (ondersteun Linux, macOS, Windows, Android, iOS, FreeBSD), unzip en sit in `PATH`:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Begin vinnig

### 1. Geen konfigurasie direkte transmissie tussen twee masjiene (`send` / `recv`)

Dit is nie nodig om 'n deelgroep op te stel nie, lêers of gidse direk oor te dra tussen twee masjiene teen 'n hoë spoed:

```bash
# Die ontvangkant luister op die port en skryf na ./backup
sync0 recv 0.0.0.0:9000 ./backup

# Direkte oordrag vanaf die sender (opsioneel --vee spieël uit om oortollige lêers skoon te maak, --koers 50MB spoedlimiet)
sync0 send ./project 192.168.1.10:9000
```

### 2. Plasingdeling en op-aanvraag trek/oplaai (`share` / `pull` / `push`)

```bash
# Bediener: Publiseer die deelgroep met een reël van opdrag (voer die IP-vrye iroh node adres uit na opstart)
sync0 share dataset:./data

# Kliënt: Trek die hele datasteldeelgroep na die plaaslike ./local_data
sync0 trek datastel:// ./local_data -p <node-adres>

# Kliënt: Trek veelvuldige keuses op aanvraag (sinkroniseer slegs gespesifiseerde enkellêers en subgidse)
sync0 trek datastel://docs/readme.md,assets/icons,src/core ./local_data -p <node address>

# Kliënt: Laai lêers of gidse op na gemagtigde deelgroepe
sync0 push datastel://releases/v1 ./build_output -p <node-adres>
```

### 3. Intydse intekeningsaai en multi-masjien tweerigtingsinchronisasie (`sub` / `group`)

Na die totstandkoming van 'n lang verbinding, as enige lêer **toevoeging, wysiging, uitvee, verandering van toestemmings of wysigingstyd** op die afgeleë kant plaasvind, sal die plaaslike gids sinchronies opgedateer word in millisekondes in reële tyd (nuwe/gewysigde groot lêers sal slegs die verskilblokke trek en hulle op hul plek opdateer, lêers uitvee sal outomaties plaaslike lêers en leë gidse opruim, en net tyd verander blokkeer permissies):

- **`sync0 sub`(Eenrigting-intydsspieëling + outomatiese saai)**: Die plaaslike intydse tyd volg die byvoegings, skrappings en wysigings van die deelpunt, en dien outomaties as 'n P2P-saadbron om gelyktydige versnelling vir ander treknodusse te verskaf.
- **`sync0 group`(Multi-masjien eweknie-tot-ewe tweerigting-intydse sinchronisasie)**: Indien lêers bygevoeg, uitgevee of gewysig word op enige masjien in die groep, sal alle ander aanlyn nodusse sinchroon in reële tyd opgedateer word en as saad vir mekaar dien.

```bash
# Teken in op die hele deelgroep: afstandbyvoegings, skrappings en wysigings word intyds plaaslik gesinchroniseer en outomaties gesaai
sync0 sub datastel:// ./local_mirror -p <node adres>

# Op-aanvraag multi-seleksie-intekening: sinchroniseer en saai slegs spesifieke enkele lêers en subgidse in reële tyd
sync0 subdatastel://models/llama.onnx,configs ./local_mirror -p <node-adres>

# Multi-masjien portuurgroep tweerigting-intydse sinchronisasie (ondersteun ook volle of komma-multi-seleksie-paaie)
sync0 groep span-docs:// ./docs -p <node-adres>
```

### 4. Span multi-deling groep en lid toestemming konfigurasie (`.nt` konfigurasie)

Die verstekkonfigurasiegids is `~/.config/sync0` (kan oorgeskakel word via `-c <gids>`), deur die minimalistiese [NestedText](https://nestedtext.org) (`.nt`) formaat te gebruik:
- **Ondersteun die beperking van publieke netwerklees (slegs gemagtigde gebruikers kan lees)**: Solank as wat die deelgroep gekonfigureer is met 'n `users:` ledelys (of die opdragreël spesifiseer `--allow-r` / `--allow-rw` / `--allow-w` / `--auth`), skakel die stelsel **outomaties anonieme (<)privaat outentieke ** en publieke outentieke netwerk aan nodus opsomming of lees, en P2P Inteken op saai nodusse sal ook die witlys gelyktydig erf, sodat slegs gemagtigde gebruikers kan lees; deelgroepe wat nie met `users` gekonfigureer is nie, is by verstek publiek en leesalleen (as jy publiek en leesalleen wil wees en oop wil wees vir oplaaie net aan gespesifiseerde lede, kan jy uitdruklik `auth: false` stel).
- **Fynkorrekte lees-skryf- en subgidsafleweringstoestemmings**: Lede is by verstek leesalleen (`r` hoef nie eksplisiet geskryf te word nie), en ondersteun `r` (leesalleen), `w` (skryf net na die drop box, nie leesbaar nie), `rw` en `rw` (lees-skryf id e) en `rw` (lees-beperk id e) oplaai).

**Ledekonfigurasievoorbeeld (`~/.config/sync0/users/`):**

```yaml
# ~/.config/sync0/users/alice.nt (verstek leesalleen, spoedbeperking 80MB)
id: 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c
rate: 80MB
```

```yaml
# ~/.config/sync0/users/bob.nt (lees- en skryftoestemmings, slegs toegelaat om op te laai na gespesifiseerde subgids)
id: 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f
perm: rw
write_prefix:
  - uploads/bob
  - shared/common
```

```yaml
# ~/.config/sync0/users/charlie.nt (skryf slegs afleweringstoestemming, lees is verbode)
id: 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
perm: w
```

**Voorbeeld van deelgroepopstelling (`~/.config/sync0/config.nt`):**

```yaml
users_dir: ./users

groups:
  # Nie-gekonfigureerde gebruikers: verstek publieke netwerk leesalleen
  dataset:
    path: ./data
    rate: 50MB

  # Gekonfigureerde gebruikers: beperk outomaties publieke netwerklees (slegs Alice en Bob kan lees, Charlie kan net aflewer en oplaai)
  team-vault:
    path: ./vault
    users:
      - alice
      - bob
      - charlie
```

Begin die diens (laai die konfigurasielêer of gebruik direk opdragreëlparameters om publieke netwerklees te beperk en lede te magtig):

```bash
# Laai ~/.config/sync0/config.nt om te begin
sync0 share --region SHA

# Of publiseer 'n private deelgroep deur suiwer opdragreël (spesifiseer --allow-* om publieke netwerklees outomaties te beperk en slegs gemagtigde gebruikers kan toegang daartoe kry)
sync0 share team-vault:./vault \
  --allow-r 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c \
  --allow-rw 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f:uploads/bob,shared/common \
  --allow-w 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
```

### 5. Nodusadres, besigheidskaartjie en multi-taal

```bash
# Bekyk die plaaslike IP-vrye nodusadres (sleutel word outomaties gegenereer vir die eerste lopie)
sync0 id

# Genereer Ed25519 self-ondertekende node besigheidskaartjie (bynaam, stad kode, e-pos, webwerf, inleiding)
sync0 profile -u alice --region SHA --email alice@example.com -b "Shanghai node"

# Daar is 75 ingeboude tale, wat by verstek die stelseltaal volg en deur omgewingsveranderlikes omgeskakel kan word.
SYNC0_LANG=en sync0 --help
```

## Prestasie en kernkenmerke

### Kruiskontinentale WAN werklike meting (Vergelyking `rsync`)

Werklike metingsomgewing: Verenigde State `c1` → Europa `n1` (RTT 65 ms), die datastel is Linux-kern `linux-7.3.3`:

#### 1) A la carte-modus teenoor `rsync`

| Datastel | grootte | `sync0` Enkelpunt<br>(Mbps) | `rsync`<br>(Mbps) | Versnel (`sync0` / `rsync`) |
| :--- | ---: | ---: | ---: | ---: |
| `linux-7.3.3.tar.xz`(nie saampersbaar nie) | 155 MB | **121.07** | 107.89 | **1,12×** |
| `linux-7.3.3.tar`(saamdrukbaar) | 1.6 GB | **442.07** | 506.62 | **0,87×** |
| `linux-7.3.3.tar`(plaaslike verandering inkrement) | 1.6 GB | **8,123.87** | 1,169.49 | **6,95×** |
| `linux-7.3.3/` (95 979 klein lêers) | 1.5 GB | **565.49** | 235.27 | **2,40×** |
| **meetkundige gemiddelde** | — | **704.17** | 350.19 | **2,01×** |

#### 2) P2P-dubbelbronmodus teenoor `rsync`

| Datastel | grootte | `sync0` P2P Dubbele Bron<br>(Mbps) | `rsync`<br>(Mbps) | Versnel (`sync0` / `rsync`) |
| :--- | ---: | ---: | ---: | ---: |
| `linux-7.3.3.tar.xz`(nie saampersbaar nie) | 155 MB | **466.85** | 107.89 | **4,33×** |
| `linux-7.3.3.tar`(saamdrukbaar) | 1.6 GB | **992.17** | 506.62 | **1,96×** |
| `linux-7.3.3.tar`(plaaslike verandering inkrement) | 1.6 GB | **5,666.84** | 1,169.49 | **4,85×** |
| `linux-7.3.3/` (95 979 klein lêers) | 1.5 GB | **703.30** | 235.27 | **2,99×** |
| **meetkundige gemiddelde** | — | **1,165.63** | 350.19 | **3,33×** |

> Let wel: In P2P-dubbelbronmodus werk Europese nodusse `s1` saam vir saai, en die kruis-kontinentale ruggraat RTT is 65 ms.

### Kernprodukkenmerke

- **Publieke sleutel adresseer outomatiese gatpons**: Geen behoefte aan 'n sentrale bediener en publieke netwerkadres nie, outomatiese NAT-gatpons gebaseer op die verspreide netwerk publieke sleuteladres.
- **Millisekonde-vlak intydse sinchronisasie en metadata-bewustheid**: Gebaseer op inheemse lêergebeurtenismonitering (`FSEvents` / `inotify` / `ReadDirectoryChangesW`), intydse stoot van lêerbyvoegings, skrapings, POSIX-toestemmings (`mode`) en millisekonde-wysigingstyd="5"> (`ReadDirectoryChangesW`); nul datablok-oordrag wanneer slegs toestemmings of tyd gewysig word.
- **BT-styl gelyktydige aflaai en oplaai en streeksaffiniteitskedulering**: Die aflaainodus hoef nie te wag vir die volle bedrag om afgelaai te word nie. Al die blokintervalle (`Op::Have`) wat die 128-bis hash-tjek geslaag het, word onmiddellik in die geheue/tydelike poel gesluit sodat ander nodusse gelyktydig kan trek. Saam met dinamiese PEX-node-uitruiling en stadige node-halfvoorrang (Work-Stealing), word die cluster-deurset gemaksimeer.
- **Anti-kwaadwillige nodus peuter en tros vergiftiging immuniteit**: gesaghebbende bronne lys `root_hash` Self-konsekwente sluiting + verpligte hardeware versnelling `gxhash128` vertering verifikasie voordat elke blok op die skyf geplaas en ekstern gesaai word; as 'n gepeuterde blok gevind word, sal die millisekondevlak-stroombreker die kwaadwillige nodus verduister en outomaties van die eerlike bron herstel.
- **FastCDC in-situ inkrement en klein lêer samevoeging**: Slegs differensiële blokke word oorgedra vir gedeeltelike wysiging van groot lêers en op skyf geplaas; massiewe klein lêer geheue verpakking en voorvoegsel differensiële kompressie elimineer lêer-vir-lêer heen-en-weer bokoste.
- **Selfgenesing na kragonderbreking en hervatting na herbegin**: Redis-agtige RDB + AOF baster formaat (`.s0_subs.bin`) word gebruik om die saaistatus en lêerlys te behou, daaglikse veranderinge word in mikrosekondes bygevoeg, en word outomaties herskryf en gereeld georganiseer. Nadat die bediener herbegin is, word vanlyn veranderinge outomaties na die saainodus gedruk.

## Tegniese argitektuur en dokumentasie

- **Dubbelvlak ontkoppelingsprotokol**:
  - **Beheer en intydse veranderingsoppervlak (`iroh` QUIC / TCP, eerste greep `PROTO_VERSION` weergawe handdruk) **: Verantwoordelik vir Ed25519 besigheidskaartjie verifikasie, toestemming verifikasie, Swarm saad ontdekking, en ondersteuning vir `u64::MAX` vlak massa verandering streaming bonding en <b id=" id="b "> (push id="4"> intydse id="6"> / `FsOp::Remove` / `FsOp::Meta` Drie primitiewe).
  - **Dataverspreidingsoppervlak (inheemse UDP + `compio` asynchrone I/O) **: 20B kompakte kopskrif, 18B TACK periodieke terugvoer, BBRv3/Copa hibriede opeenhopingsbeheer, RACK-TLP pakketverliesherwinning, eerste-blok Shannon-entropie-aanpasbare Zstd-kompressie-omleiding, en <b id="-2"> blokkering.
- **Kom meer te wete**:
  - [Prestasiemetingsverslag en reproduksieskrif](https://github.com/webc-site/sync0/blob/main/doc/zh/benchmark.md)
  - [Stelsel argitektuur ontwerp](https://github.com/webc-site/sync0/blob/main/doc/zh/design_v4.md)
  - [Binêre draadprotokol en volhardingspesifikasie](https://github.com/webc-site/sync0/blob/main/doc/zh/proto.md)
  - [Identiteitstawing, gesagsbeheer en integriteitsverifikasie](https://github.com/webc-site/sync0/blob/main/doc/zh/crypto.md)