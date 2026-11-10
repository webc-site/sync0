# sync0

`sync0` është një mjet i decentralizuar i ndarjes së skedarëve P2P, sinkronizimit në kohë reale dhe shpërndarjes me shumë burime, i përdorur për të zëvendësuar `rsync` - shpejtësia e matur e rrjetit të zonës së gjerë transkontinentale është deri në **6,95×** e `rsync` (shpejtësia mesatare gjeometrike me një pikë*, shpejtësia mesatare duometrike*2,01 P **3,33×**), dhe sinkronizimi i skedarëve të vegjël masiv nëpër oqeane është aq i shpejtë sa **7,19×**.

Çdo makinë është njëkohësisht një server dhe një klient: nuk ka nevojë për një server qendror ose një adresë rrjeti publik, hapja automatike e vrimave bazohet në adresën e çelësit publik të rrjetit të shpërndarë dhe mbështet transmetimin e drejtpërdrejtë midis dy makinave, pritjen e lejeve të grupit me shumë ndarje, përzgjedhje të shumëfishtë të nëndrejtorive sipas kërkesës, sinkronizim me shumë burime rritëse në kohë reale të nivelit milisekonda dhe P2P.

## Instaloni

### Metoda 1: Instalimi i ngarkesave

```bash
cargo install sync0
```

### Metoda 2: Shkarkoni binarët e parakompiluar

Shkoni te [GitHub Releases](https://github.com/webc-site/sync0/releases) për të shkarkuar paketën e ngjeshur `.tar.xz` që korrespondon me sistemin (mbështet Linux, macOS, Windows, Android, iOS, FreeBSD), shkëputeni dhe vendoseni në `PATH`:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Filloni shpejt

### 1. Transmetim i drejtpërdrejtë me konfigurim zero ndërmjet dy makinave (`send` / `recv`)

Nuk ka nevojë të konfiguroni një grup shkëmbimi, të transferoni direkt skedarë ose drejtori midis dy makinave me shpejtësi të lartë:

```bash
# Fundi marrës dëgjon në port dhe shkruan në ./backup
sync0 recv 0.0.0.0:9000 ./backup

# Transmetim i drejtpërdrejtë nga dërguesi (opsionale -- fshini pasqyrën për të pastruar skedarët e tepërt, -- norma kufiri i shpejtësisë 50 MB)
sync0 send ./project 192.168.1.10:9000
```

### 2. Ndarja e postimit dhe tërheqja/ngarkimi sipas kërkesës (`share` / `pull` / `push`)

```bash
# Serveri: Publikoni grupin e ndarjes me një linjë komande (nxjerr adresën e nyjës iroh pa IP pas fillimit)
sync0 share dataset:./data

# Klienti: Tërhiqeni të gjithë grupin e ndarjes së të dhënave në lokalin ./local_data
sync0 pull dataset:// ./local_data -p <adresa e nyjes>

# Klienti: Tërhiq zgjedhje të shumta sipas kërkesës (sinkronizoni vetëm skedarë dhe nëndrejtori të veçanta të specifikuara)
sync0 tërheqëse të dhënash://docs/readme.md,assets/icons,src/core ./local_data -p <adresa e nyjës>

# Klienti: Ngarko skedarët ose drejtoritë në grupet e autorizuara të ndarjes
sync0 push database://releases/v1 ./build_output -p <adresa e nyjes>
```

### 3. Mbjellja e abonimit në kohë reale dhe sinkronizimi i dyanshëm me shumë makina (`sub` / `group`)

Pas vendosjes së një lidhjeje të gjatë, nëse ndonjë skedar **shtimi, modifikimi, fshirja, ndryshimi i lejeve ose koha e modifikimit** ndodh në skajin e largët, direktoria lokale do të përditësohet në mënyrë sinkrone në milisekonda në kohë reale (skedarët e mëdhenj/të rinj/të modifikuar vetëm do të tërheqin blloqet e dallimeve dhe do t'i përditësojnë ato në vend, fshirja e skedarëve do të pastrojë automatikisht ose do të ndryshojë skedarët e të dhënave të zbrazëta dhe do të ndryshojë automatikisht vetëm skedarët e kohës për çdo drejtori bosh

- **`sync0 sub`(Pasqyrim me një drejtim në kohë reale + mbjellje automatike)**: Lokal në kohë reale ndjek shtesat, fshirjet dhe modifikimet e fundit të ndarjes dhe shërben automatikisht si një burim i farës P2P për të ofruar përshpejtim të njëkohshëm për nyjet e tjera tërheqëse.
- **`sync0 group`(Sinkronizimi i dyanshëm në kohë reale nga shumë-makina nga kolegët në kolegë)**: Nëse skedarët shtohen, fshihen ose modifikohen në ndonjë makinë në grup, të gjitha nyjet e tjera në internet do të përditësohen në mënyrë sinkrone në kohë reale dhe do të shërbejnë si bazë për njëri-tjetrin.

```bash
# Abonohu ​​në të gjithë grupin e ndarjes: shtesat, fshirjet dhe modifikimet në distancë sinkronizohen në nivel lokal në kohë reale dhe vendosen automatikisht
sync0 subsets:// ./local_mirror -p <adresa e nyjes>

# Abonim me shumë përzgjedhje sipas kërkesës: sinkronizoni dhe vendosni vetëm skedarë dhe nëndrejtori të veçanta të specifikuara në kohë reale
sync0 subsets://models/llama.onnx,configs ./local_mirror -p <adresa e nyjës>

# Sinkronizimi i dyanshëm në kohë reale i grupit të kolegëve me shumë makina (gjithashtu mbështet shtigjet me shumë përzgjedhje të plota ose me presje)
sync0 group team-docs:// ./docs -p <adresa e nyjes>
```

### 4. Grupi me shumë ndarje të ekipit dhe konfigurimi i lejeve të anëtarëve (konfigurim`.nt`)

Drejtoria e parazgjedhur e konfigurimit është `~/.config/sync0` (mund të ndërrohet nëpërmjet `-c <directory>`), duke përdorur formatin minimalist [NestedText](https://nestedtext.org) (`.nt`):
- **Mbështet kufizimin e leximit të rrjetit publik (vetëm përdoruesit e autorizuar mund të lexojnë)**: Për sa kohë që grupi i ndarjes është konfiguruar me një listë anëtarësh `users:` (ose linja e komandës specifikon `--allow-r` / `--allow-rw` / `--allow-w` / `--auth` / `--auth`, sistemi ndizet (`--auth` automatikisht id="6">)** dhe ndalon numërimin ose leximin e nyjeve anonime të rrjetit publik, dhe Abonimi P2P në nyjet e mbjelljes do të trashëgojë gjithashtu listën e bardhë në të njëjtën kohë, duke lejuar vetëm përdoruesit e autorizuar të lexojnë; grupet e ndarjes që nuk janë konfiguruar me `users` janë publike dhe vetëm për lexim sipas parazgjedhjes (nëse dëshironi të jeni publike dhe vetëm për lexim dhe të hapura për ngarkime vetëm për anëtarët e specifikuar, mund të vendosni në mënyrë eksplicite `auth: false`).
- **Lejet e hollësishme për lexim-shkrim dhe për dorëzimin e nëndirektorisë**: Anëtarët janë vetëm për lexim sipas parazgjedhjes (`r` nuk ka nevojë të shkruhen në mënyrë eksplicite) dhe mbështesin `r` (vetëm për lexim), `w` (vetëm shkruani në kutinë e lëshimit, jo të lexueshëm), <b> dhe <id="1">" (kufizoni prefiksin e nëndirektorisë së lejuar për ngarkim).

**Shembulli i konfigurimit të anëtarit (`~/.config/sync0/users/`):**

```yaml
# ~/.config/sync0/users/alice.nt (e parazgjedhur vetëm për lexim, kufiri i shpejtësisë 80 MB)
id: 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c
rate: 80MB
```

```yaml
# ~/.config/sync0/users/bob.nt (lejet e leximit dhe shkrimit, lejohen vetëm të ngarkohen në nëndirektorinë e specifikuar)
id: 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f
perm: rw
write_prefix:
  - uploads/bob
  - shared/common
```

```yaml
# ~/.config/sync0/users/charlie.nt (vetëm leja e dorëzimit të shkrimit, leximi është i ndaluar)
id: 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
perm: w
```

**Shembulli i konfigurimit të grupit të përbashkët (`~/.config/sync0/config.nt`):**

```yaml
users_dir: ./users

groups:
  # Përdorues të pakonfiguruar: rrjeti publik i paracaktuar vetëm për lexim
  dataset:
    path: ./data
    rate: 50MB

  # Përdoruesit e konfiguruar: kufizojnë automatikisht leximin e rrjetit publik (vetëm Alice dhe Bob mund të lexojnë, charlie mund të dorëzojë dhe ngarkojë vetëm)
  team-vault:
    path: ./vault
    users:
      - alice
      - bob
      - charlie
```

Nisni shërbimin (ngarkoni skedarin e konfigurimit ose përdorni drejtpërdrejt parametrat e linjës së komandës për të kufizuar leximin e rrjetit publik dhe për të autorizuar anëtarët):

```bash
# Ngarko ~/.config/sync0/config.nt për të filluar
sync0 share --region SHA

# Ose publikoni një grup privat të ndarjes përmes linjës së pastër të komandës (specifikoni --allow-* që të kufizojë automatikisht leximin e rrjetit publik dhe vetëm përdoruesit e autorizuar mund ta qasen atë)
sync0 share team-vault:./vault \
  --allow-r 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c \
  --allow-rw 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f:uploads/bob,shared/common \
  --allow-w 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
```

### 5. Adresa e nyjës, kartëvizita dhe shumëgjuhëshe

```bash
# Shikoni adresën lokale të nyjës pa IP (çelësi gjenerohet automatikisht për ekzekutimin e parë)
sync0 id

# Gjeneroni kartëvizitën e nyjës së vetë-nënshkruar Ed25519 (pseudonimi, kodi i qytetit, emaili, uebsajti, prezantimi)
sync0 profile -u alice --region SHA --email alice@example.com -b "Shanghai node"

# Ka 75 gjuhë të integruara, të cilat ndjekin gjuhën e sistemit si parazgjedhje dhe mund të ndërrohen përmes variablave të mjedisit.
SYNC0_LANG=en sync0 --help
```

## Performanca dhe karakteristikat kryesore

### Matja aktuale e WAN ndër-kontinentale (Krahasimi `rsync`)

Mjedisi aktual i matjes: Shtetet e Bashkuara `c1` → Evropë `n1` (RTT 65 ms), grupi i të dhënave është kernel Linux `linux-7.3.3`:

#### 1) Modaliteti A la carte kundrejt `rsync`

| Kompleti i të dhënave | madhësia | `sync0` Pika e vetme<br>(Mbps) | `rsync`<br>(Mbps) | Përshpejtimi (`sync0` / `rsync`) |
| :--- | ---: | ---: | ---: | ---: |
| `linux-7.3.3.tar.xz`(jo i kompresueshëm) | 155 MB | **121.07** | 107.89 | **1,12×** |
| `linux-7.3.3.tar`(i kompresueshëm) | 1.6 GB | **442.07** | 506.62 | **0,87×** |
| `linux-7.3.3.tar`(rritje lokale e ndryshimit) | 1.6 GB | **8,123.87** | 1,169.49 | **6,95×** |
| `linux-7.3.3/` (95,979 skedarë të vegjël) | 1.5 GB | **565.49** | 235.27 | **2,40×** |
| **mesatarja gjeometrike** | - | **704.17** | 350.19 | **2.01×** |

#### 2) Modaliteti me burim të dyfishtë P2P kundrejt `rsync`

| Kompleti i të dhënave | madhësia | `sync0` Burimi i dyfishtë P2P<br>(Mbps) | `rsync`<br>(Mbps) | Përshpejtimi (`sync0` / `rsync`) |
| :--- | ---: | ---: | ---: | ---: |
| `linux-7.3.3.tar.xz`(jo i kompresueshëm) | 155 MB | **466.85** | 107.89 | **4,33×** |
| `linux-7.3.3.tar`(i kompresueshëm) | 1.6 GB | **992.17** | 506.62 | **1,96×** |
| `linux-7.3.3.tar`(rritje lokale e ndryshimit) | 1.6 GB | **5,666.84** | 1,169.49 | **4,85×** |
| `linux-7.3.3/` (95,979 skedarë të vegjël) | 1.5 GB | **703.30** | 235.27 | **2,99×** |
| **mesatarja gjeometrike** | - | **1,165.63** | 350.19 | **3,33×** |

> Shënim: Në modalitetin me burim të dyfishtë P2P, nyjet evropiane `s1` bashkëpunojnë për mbjellje, dhe shtylla kurrizore ndër-kontinentale RTT është 65 ms.

### Karakteristikat kryesore të produktit

- **Shpimi automatik i vrimave me adresimin e çelësit publik**: Nuk ka nevojë për një server qendror dhe adresë rrjeti publik, hapje automatike e vrimave NAT bazuar në adresën e çelësit publik të rrjetit të shpërndarë.
- **Sinkronizimi në kohë reale i nivelit milisekonda dhe ndërgjegjësimi i meta të dhënave**: Bazuar në monitorimin e ngjarjeve të skedarit vendas (`FSEvents` / `inotify` / `ReadDirectoryChangesW`), shtytje në kohë reale të shtesave, fshirjeve të skedarëve, lejeve POSIX (`mode`) dhe kohës së modifikimit të millise (`mode`->milivel="d); transmetimi i bllokut të të dhënave zero kur modifikohen vetëm lejet ose koha.
- **Shkarkimi dhe ngarkimi i njëkohshëm i stilit BT dhe planifikimi i afinitetit rajonal**: Nyja e shkarkimit nuk ka nevojë të presë që shuma e plotë të shkarkohet. Të gjitha intervalet e bllokut (`Op::Have`) që kanë kaluar kontrollin hash 128-bit janë të kyçur menjëherë në memorie/pool të përkohshëm që nyjet e tjera të tërhiqen njëkohësisht. Së bashku me shkëmbimin dinamik të nyjeve PEX dhe gjysmën e parandalimit të ngadalshëm të nyjeve (Vjedhja e punës), xhiroja e grupit maksimizohet.
- **Ndjenja kundër keqdashjes dhe imuniteti i helmimit të grupimeve**: lista autoritative e burimeve `root_hash` Kyçja e qëndrueshme + përshpejtimi i detyrueshëm i harduerit `gxhash128` verifikimi i tretjes përpara se çdo bllok të vendoset në disk dhe të vendoset jashtë; nëse gjendet një bllok i manipuluar, ndërprerësi i qarkut të nivelit milisekonda do të errësojë nyjen me qëllim të keq dhe do të rikuperohet automatikisht nga burimi i sinqertë.
- **Rritja e FastCDC in-situ dhe grumbullimi i skedarëve të vegjël**: Transferohen vetëm blloqe diferenciale për modifikim të pjesshëm të skedarëve të mëdhenj dhe vendosen në disk; paketimi masiv i memories së skedarëve të vegjël dhe ngjeshja diferenciale e prefiksit eliminojnë numrin e sipërm të udhëtimit vajtje-ardhje skedar pas skedari.
- **Vetë-shërimi pas ndërprerjes së energjisë dhe rifillimi pas rinisjes**: Formati hibrid RDB + AOF i ngjashëm me Redis (`.s0_subs.bin`) përdoret për të vazhduar statusin e mbjelljes dhe listën e skedarëve, ndryshimet ditore shtohen në mikrosekonda dhe rishkruhen dhe organizohen automatikisht rregullisht. Pas rinisjes së serverit, ndryshimet jashtë linje shtyhen automatikisht në nyjen fillestare.

## Arkitektura teknike dhe dokumentacioni

- **Protokolli i dyfishtë i shkëputjes së aeroplanit**:
  - **Kontrolli dhe sipërfaqja e ndryshimit në kohë reale (`iroh` QUIC / TCP, bajt i parë `PROTO_VERSION` versioni shtrëngim duarsh) **: Përgjegjës për verifikimin e kartës së biznesit Ed25519, verifikimin e lejes, zbulimin e farës së Swarm dhe mbështetjen për `u64::MAX` transmetimin e transmetimit të ndryshimit masiv të nivelit dhe <b> compressed="4" në kohë reale (`FsOp::Upsert` / `FsOp::Remove` / `FsOp::Meta` Tre primitivë).
  - **Sipërfaqja e shpërndarjes së të dhënave (UDP amtare + `compio` I/O asinkrone) **: koka kompakte 20B, reagime periodike 18B TACK, kontroll hibrid i mbingarkesës BBRv3/Copa, rikuperimi i humbjes së paketave RACK-TLP, bllokimi i parë i entropisë së Shannon <bllokimi adaptiv i Zstd="2" dhe Zstd" verifikimi nga fundi në fund.
- **Mësoni më shumë**:
  - [Raporti i matjes së performancës dhe skenari i riprodhimit](https://github.com/webc-site/sync0/blob/main/doc/zh/benchmark.md)
  - [Dizajni i arkitekturës së sistemit](https://github.com/webc-site/sync0/blob/main/doc/zh/design_v4.md)
  - [Protokolli Binary Wire dhe Specifikimi i qëndrueshmërisë](https://github.com/webc-site/sync0/blob/main/doc/zh/proto.md)
  - [Vërtetimi i identitetit, kontrolli i autoritetit dhe verifikimi i integritetit](https://github.com/webc-site/sync0/blob/main/doc/zh/crypto.md)