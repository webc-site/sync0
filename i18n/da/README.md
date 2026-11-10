# sync0

`sync0` er et decentraliseret P2P-fildelings-, realtidssynkroniserings- og distributionsværktøj med flere kilder, der bruges til at erstatte `rsync` - den målte hastighed for det transkontinentale wide area-netværk er op til **6,95×** af `rsync` (enkeltpunkts geometrisk gennemsnitshastighed **P dual×-**,× geometrisk gennemsnitshastighed **P dual×-**, 3** og ** geometrisk hastighed **, 3, **, 3, ** og **, 2. synkroniseringen af massive små filer på tværs af oceaner er så hurtig som **7,19×**.

Hver maskine er både en server og en klient: der er ikke behov for en central server eller en offentlig netværksadresse, automatisk hulning er baseret på den distribuerede netværks offentlige nøgleadresse, og den understøtter direkte transmission mellem to maskiner, hosting af multidelingsgruppetilladelser, flere valg af undermapper efter behov, trinvis synkronisering på millisekundniveau i realtid og samtidig P2P-acceleration.

## Installere

### Metode 1: Lastinstallation

```bash
cargo install sync0
```

### Metode 2: Download prækompilerede binære filer

Gå til [GitHub Releases](https://github.com/webc-site/sync0/releases) for at downloade den `.tar.xz` komprimerede pakke, der svarer til systemet (understøtter Linux, macOS, Windows, Android, iOS, FreeBSD), pak ud og sæt den i `PATH`:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Kom hurtigt i gang

### 1. Nul-konfiguration direkte transmission mellem to maskiner (`send` / `recv`)

Ingen grund til at konfigurere en delingsgruppe, overføre filer eller mapper direkte mellem to maskiner ved høj hastighed:

```bash
# Den modtagende ende lytter på porten og skriver til ./backup
sync0 recv 0.0.0.0:9000 ./backup

# Direkte transmission fra afsenderen (valgfrit -- slet spejl for at rydde op i overskydende filer, -- rate 50 MB hastighedsgrænse)
sync0 send ./project 192.168.1.10:9000
```

### 2. Indlægsdeling og on-demand pull/upload (`share` / `pull` / `push`)

```bash
# Server: Udgiv delingsgruppen med én kommandolinje (udsend den IP-frie iroh-nodeadresse efter opstart)
sync0 share dataset:./data

# Klient: Træk hele datasætdelingsgruppen til den lokale ./local_data
sync0 pull dataset:// ./local_data -p <nodeadresse>

# Klient: Træk flere valg efter behov (synkroniser kun specificerede enkelte filer og undermapper)
sync0 pull dataset://docs/readme.md,assets/icons,src/core ./local_data -p <node address>

# Klient: Upload filer eller mapper til autoriserede delingsgrupper
sync0 push dataset://releases/v1 ./build_output -p <nodeadresse>
```

### 3. Realtidsabonnementsåning og tovejssynkronisering med flere maskiner (`sub` / `group`)

Efter at have etableret en lang forbindelse, hvis en fil **tilføjelse, ændring, sletning, ændring af tilladelser eller ændringstid** forekommer på den eksterne ende, vil den lokale mappe blive opdateret synkront i millisekunder i realtid (nye/ændrede store filer vil kun trække forskelsblokkene og opdatere dem på plads, slette filer vil automatisk rydde op i lokale filer og tomme mapper med tilladelser, og kun ændrer amp-tilladelser direkte):

- **`sync0 sub`(Envejs-realtidsspejling + automatisk seeding)**: Den lokale realtid følger tilføjelser, sletninger og ændringer af delingsenden og fungerer automatisk som en P2P-seed-kilde for at give samtidig acceleration for andre trækkende noder.
- **`sync0 group`(Multi-machine peer-to-peer tovejs realtidssynkronisering)**: Hvis filer tilføjes, slettes eller ændres på en hvilken som helst maskine i gruppen, vil alle andre online noder blive opdateret synkront i realtid og fungere som frø for hinanden.

```bash
# Abonner på hele delingsgruppen: Fjerntilføjelser, sletninger og ændringer synkroniseres lokalt i realtid og seedes automatisk
sync0 underdatasæt:// ./local_mirror -p <nodeadresse>

# On-demand multi-select abonnement: kun synkroniser og seed specificerede enkelte filer og undermapper i realtid
sync0 underdatasæt://models/llama.onnx,configs ./local_mirror -p <nodeadresse>

# Multi-maskine peer group to-vejs realtidssynkronisering (understøtter også fuld eller komma multi-select stier)
sync0 group team-docs:// ./docs -p <nodeadresse>
```

### 4. Konfiguration af teammultidelingsgruppe og medlemstilladelse (`.nt`konfiguration)

Standardkonfigurationsmappen er `~/.config/sync0` (kan skiftes via `-c <directory>`), ved brug af det minimalistiske [NestedText](https://nestedtext.org) (`.nt`) format:
- **Understøtter begrænsning af offentlig netværkslæsning (kun autoriserede brugere kan læse)**: Så længe delingsgruppen er konfigureret med en `users:` medlemsliste (eller kommandolinjen specificerer `--allow-r` / `--allow-rw` / `--allow-w` / `--auth`), aktiverer systemet **automatisk anonymitet 6 (<)privat autentisk ** og offentligt autentisk netværk. node optælling eller læsning, og P2P Abonner på seed noder vil også arve hvidlisten samtidigt, så kun autoriserede brugere kan læse; delegrupper, der ikke er konfigureret med `users` er offentlige og skrivebeskyttede som standard (hvis du vil være offentlige og skrivebeskyttet og kun åbne for uploads til specificerede medlemmer, kan du udtrykkeligt indstille `auth: false`).
- **Finkoret læse-skrive- og undermappeleveringstilladelser**: Medlemmer er skrivebeskyttet som standard (`r` behøver ikke at blive skrevet eksplicit), og understøtter `r` (skrivebeskyttet), `w` (skriv kun til drop-boksen, kan ikke læses), <b id="4="4"> (læsebegrænsning id e) og <(read-limit id e) uploader).

**Eksempel på medlemskonfiguration (`~/.config/sync0/users/`):**

```yaml
# ~/.config/sync0/users/alice.nt (standard skrivebeskyttet, hastighedsgrænse 80 MB)
id: 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c
rate: 80MB
```

```yaml
# ~/.config/sync0/users/bob.nt (læse- og skrivetilladelser, kun tilladt at uploade til specificeret undermappe)
id: 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f
perm: rw
write_prefix:
  - uploads/bob
  - shared/common
```

```yaml
# ~/.config/sync0/users/charlie.nt (kun skrive leveringstilladelse, læsning er forbudt)
id: 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
perm: w
```

**Eksempel på konfiguration af delingsgruppe (`~/.config/sync0/config.nt`):**

```yaml
users_dir: ./users

groups:
  # Ikke konfigurerede brugere: standard offentlige netværk skrivebeskyttet
  dataset:
    path: ./data
    rate: 50MB

  # Konfigurerede brugere: begrænser automatisk offentlig netværkslæsning (kun alice og bob kan læse, charlie kan kun levere og uploade)
  team-vault:
    path: ./vault
    users:
      - alice
      - bob
      - charlie
```

Start tjenesten (indlæs konfigurationsfilen eller brug direkte kommandolinjeparametre til at begrænse offentlig netværkslæsning og godkende medlemmer):

```bash
# Indlæs ~/.config/sync0/config.nt for at starte
sync0 share --region SHA

# Eller udgiv en privat delingsgruppe via ren kommandolinje (angiv --allow-* for automatisk at begrænse offentlig netværkslæsning, og kun autoriserede brugere kan få adgang til den)
sync0 share team-vault:./vault \
  --allow-r 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c \
  --allow-rw 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f:uploads/bob,shared/common \
  --allow-w 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
```

### 5. Nodeadresse, visitkort og flersprog

```bash
# Se den lokale IP-fri nodeadresse (nøglen genereres automatisk til første kørsel)
sync0 id

# Generer Ed25519 selvsigneret node visitkort (kaldenavn, bykode, e-mail, hjemmeside, introduktion)
sync0 profile -u alice --region SHA --email alice@example.com -b "Shanghai node"

# Der er 75 indbyggede sprog, som følger systemsproget som standard og kan skiftes gennem miljøvariabler.
SYNC0_LANG=en sync0 --help
```

## Ydeevne og kerneegenskaber

### Tværkontinentalt WAN faktisk måling (sammenligning `rsync`)

Faktisk målemiljø: USA `c1` → Europa `n1` (RTT 65 ms), datasættet er Linux-kerne `linux-7.3.3`:

#### 1) A la carte-tilstand vs `rsync`

| Datasæt | størrelse | `sync0` Enkelt punkt<br>(Mbps) | `rsync`<br>(Mbps) | Fremskyndelse (`sync0` / `rsync`) |
| :--- | ---: | ---: | ---: | ---: |
| `linux-7.3.3.tar.xz`(ikke komprimerbar) | 155 MB | **121.07** | 107.89 | **1,12×** |
| `linux-7.3.3.tar`(komprimerbar) | 1.6 GB | **442.07** | 506.62 | **0,87×** |
| `linux-7.3.3.tar`(lokal ændringsstigning) | 1.6 GB | **8,123.87** | 1,169.49 | **6,95×** |
| `linux-7.3.3/` (95.979 små filer) | 1.5 GB | **565.49** | 235.27 | **2,40×** |
| **geometrisk middelværdi** | — | **704.17** | 350.19 | **2,01×** |

#### 2) P2P-dobbeltkildetilstand vs `rsync`

| Datasæt | størrelse | `sync0` P2P Dual Source<br>(Mbps) | `rsync`<br>(Mbps) | Fremskyndelse (`sync0` / `rsync`) |
| :--- | ---: | ---: | ---: | ---: |
| `linux-7.3.3.tar.xz`(ikke komprimerbar) | 155 MB | **466.85** | 107.89 | **4,33×** |
| `linux-7.3.3.tar`(komprimerbar) | 1.6 GB | **992.17** | 506.62 | **1,96×** |
| `linux-7.3.3.tar`(lokal ændringsstigning) | 1.6 GB | **5,666.84** | 1,169.49 | **4,85×** |
| `linux-7.3.3/` (95.979 små filer) | 1.5 GB | **703.30** | 235.27 | **2,99×** |
| **geometrisk middelværdi** | — | **1,165.63** | 350.19 | **3,33×** |

> Bemærk: I P2P dual-source-tilstand samarbejder europæiske noder `s1` om seeding, og den tværkontinentale backbone RTT er 65 ms.

### Kerneproduktegenskaber

- **Public nøgle adressering automatisk hulning**: Intet behov for en central server og offentlig netværksadresse, automatisk NAT hulning baseret på den distribuerede netværks offentlige nøgleadresse.
- **Millisekund-niveau realtidssynkronisering og metadatabevidsthed**: Baseret på indbygget filhændelsesovervågning (`FSEvents` / `inotify` / `ReadDirectoryChangesW`), realtids push af filtilføjelser, sletninger, POSIX-tilladelser (`mode`) og millisekund-niveau ændringstid="5"> (`ReadDirectoryChangesW`); nul datablokoverførsel, når kun tilladelser eller tidspunkt er ændret.
- **Samtidig download og upload i BT-stil og regional affinitetsplanlægning**: Downloadingsnoden behøver ikke at vente på, at det fulde beløb bliver downloadet. Alle blokintervaller (`Op::Have`), der har bestået 128-bit hash-kontrollen, låses øjeblikkeligt i hukommelsen/midlertidig pool, så andre noder kan trække samtidigt. Sammen med dynamisk PEX-node-udveksling og langsom node-halv-preemption (Work-Stealing) maksimeres klyngegennemløbet.
- **Anti-ondsindet knudemanipulation og klyngeforgiftningsimmunitet**: autoritativ kildeliste `root_hash` Selvkonsistent låsning + obligatorisk hardwareacceleration `gxhash128` digest-verifikation før hver blok placeres på disken og seedes eksternt; hvis der findes en manipuleret blok, vil afbryderen på millisekundniveau mørklægge den ondsindede node og automatisk genoprette fra den ærlige kilde.
- **FastCDC in-situ stigning og aggregering af små filer**: Kun differentielle blokke overføres til delvis ændring af store filer og placeres på disk; massiv indpakning af hukommelse til små filer og differentiel komprimering af præfiks eliminerer overhead fra fil-for-fil.
- **Selvhelbredende efter strømafbrydelse og genoptagelse efter genstart**: Redis-lignende RDB + AOF hybridformat (`.s0_subs.bin`) bruges til at bevare seedingstatus og filliste, daglige ændringer tilføjes i mikrosekunder og omskrives automatisk og organiseres regelmæssigt. Når serveren er genstartet, skubbes offlineændringer automatisk til seed-noden.

## Teknisk arkitektur og dokumentation

- **Dobbelt plan afkoblingsprotokol**:
  - **Kontrol- og realtidsændringsoverflade (`iroh` QUIC / TCP, første byte `PROTO_VERSION` versionshåndtryk) **: Ansvarlig for Ed25519 visitkortverifikation, tilladelsesbekræftelse, Swarm Seed-opdagelse og understøttelse af `u64::MAX` niveaumasseændringsstreaming og <b id="b id="b">realtid (push id="b"> id="6"> / `FsOp::Remove` / `FsOp::Meta` Tre primitiver).
  - **Datadistributionsoverflade (native UDP + `compio` asynkron I/O) **: 20B kompakt header, 18B TACK periodisk feedback, BBRv3/Copa hybrid overbelastningskontrol, RACK-TLP pakketabsgendannelse, første-blok Shannon entropi adaptiv Zstd komprimering bypass, og <b id.="-2"> blokering end-toend.
- **Læs mere**:
  - [Præstationsmålingsrapport og reproduktionsscript](https://github.com/webc-site/sync0/blob/main/doc/zh/benchmark.md)
  - [Systemarkitektur design](https://github.com/webc-site/sync0/blob/main/doc/zh/design_v4.md)
  - [Binary Wire Protocol og Persistence Specification](https://github.com/webc-site/sync0/blob/main/doc/zh/proto.md)
  - [Identitetsgodkendelse, myndighedskontrol og integritetsverifikation](https://github.com/webc-site/sync0/blob/main/doc/zh/crypto.md)