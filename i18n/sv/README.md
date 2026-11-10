# sync0

`sync0` är ett decentraliserat P2P-fildelnings-, realtidssynkroniserings- och distributionsverktyg för flera källor som används för att ersätta `rsync` - den uppmätta hastigheten för transkontinentalt WAN är upp till `rsync`s **5,89×** (enpunkts geometrisk medelhastighet **2,67×-**, P3P dual hastighet ** och P2P dual speed **, P2P dual speed **, P2P dual speed **. transoceanisk höglatens länk P2P-acceleration är upp till **7,68×** (fulllänkad 24-scenario omfattande geometrisk medelhastighet **2,56×**).

Varje maskin är både en server och en klient: det finns inget behov av en central server eller en offentlig nätverksadress, automatisk håltagning baseras på den distribuerade nätverkets publika nyckeladress, och den stöder direkt överföring mellan två maskiner, värd för flerdelningsgruppsbehörighet, flera urval av underkataloger på begäran, inkrementell realtidssynkronisering på millisekundnivå och P2P-acceleration med flera källor.

## Installera

### Metod 1: Lastinstallation

```bash
cargo install sync0
```

### Metod 2: Ladda ner förkompilerade binärer

Gå till [GitHub Releases](https://github.com/webc-site/sync0/releases) för att ladda ner det `.tar.xz` komprimerade paketet som motsvarar systemet (stöder Linux, macOS, Windows, Android, iOS, FreeBSD), packa upp och lägg i `PATH`:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Kom igång snabbt

### 1. Noll-konfiguration direkt överföring mellan två maskiner (`send` / `recv`)

Inget behov av att konfigurera en delningsgrupp, direkt överföra filer eller kataloger mellan två maskiner med hög hastighet:

```bash
# Mottagaren lyssnar på porten och skriver till ./backup
sync0 recv 0.0.0.0:9000 ./backup

# Direkt överföring från avsändaren (valfritt --radera spegel för att rensa upp överflödiga filer, --hastighetsgräns på 50 MB)
sync0 send ./project 192.168.1.10:9000
```

### 2. Inläggsdelning och on-demand pull/upload (`share` / `pull` / `push`)

```bash
# Server: Publicera delningsgruppen med en kommandorad (mata ut den IP-fria iroh-nodadressen efter uppstart)
sync0 share dataset:./data

# Klient: Dra hela datasetdelningsgruppen till den lokala ./local_data
sync0 pull dataset:// ./local_data -p <nodadress>

# Klient: Dra flera val på begäran (synkronisera endast specificerade enstaka filer och underkataloger)
sync0 pull dataset://docs/readme.md,assets/icons,src/core ./local_data -p <nodadress>

# Klient: Ladda upp filer eller kataloger till auktoriserade delningsgrupper
sync0 push dataset://releases/v1 ./build_output -p <nodadress>
```

### 3. Prenumerationssådd i realtid och tvåvägssynkronisering med flera maskiner (`sub` / `group`)

Efter att ha upprättat en lång anslutning, om någon fil **tillägg, modifiering, radering, ändring av behörigheter eller modifieringstid** inträffar på den avlägsna änden, kommer den lokala katalogen att uppdateras synkront i millisekunder i realtid (nya/modifierade stora filer kommer bara att dra skillnadsblocken och uppdatera dem på plats, radering av filer kommer automatiskt att rensa upp lokala filer och tomma kataloger, och bara ändra amp-permissioner direkt):

- **`sync0 sub`(Envägsspegling i realtid + automatisk seedning)**: Den lokala realtiden följer tilläggen, raderingarna och ändringarna av delningsänden och fungerar automatiskt som en P2P-frökälla för att ge samtidig acceleration för andra dragnoder.
- **`sync0 group`(Multi-machine peer-to-peer tvåvägs realtidssynkronisering)**: Om filer läggs till, raderas eller ändras på någon maskin i gruppen kommer alla andra onlinenoder att uppdateras synkront i realtid och fungera som frön för varandra.

```bash
# Prenumerera på hela delningsgruppen: fjärrtillägg, raderingar och ändringar synkroniseras lokalt i realtid och seedas automatiskt
sync0 underdataset:// ./local_mirror -p <nodadress>

# On-demand multi-select prenumeration: synkronisera endast och seed specificerade enskilda filer och underkataloger i realtid
sync0 subdataset://models/llama.onnx,configs ./local_mirror -p <nodadress>

# Multi-maskin peer group tvåvägs realtidssynkronisering (stöder även hela eller komma-multivalsvägar)
sync0 group team-docs:// ./docs -p <nodadress>
```

### 4. Konfiguration av grupp- och medlemsbehörighet för flera delning (`.nt`-konfiguration)

Standardkonfigurationskatalogen är `~/.config/sync0` (kan växlas via `-c <katalog>`), med det minimalistiska formatet [NestedText](https://nestedtext.org) (`.nt`):
- **Stöder begränsning av offentlig nätverksläsning (endast auktoriserade användare kan läsa)**: Så länge som delningsgruppen är konfigurerad med en `users:` medlemslista (eller kommandoraden anger `--allow-r` / `--allow-rw` / `--allow-w` / `--auth`), aktiverar systemet **automatiskt anonyma (<)privata autentiska (<)privata ** anonyma nätverk och offentliga nätverk. noduppräkning eller läsning och P2P Att prenumerera på såddnoder kommer också att ärva vitlistan samtidigt, vilket tillåter endast behöriga användare att läsa; delningsgrupper som inte är konfigurerade med `users` är offentliga och skrivskyddade som standard (om du vill vara offentliga och skrivskyddade och endast öppna för uppladdningar till angivna medlemmar kan du uttryckligen ställa in `auth: false`).
- **Fint skriv- och underkatalogleveransbehörigheter**: Medlemmar är skrivskyddade som standard (`r` behöver inte skrivas explicit), och stöder `r` (skrivskyddad), `w` (skriv bara till drop-boxen, inte läsbar), `rw` (läs-begränsning) för preskription av subkatalogen och `rw` (läsgräns id e) laddar upp).

**Exempel på medlemskonfiguration (`~/.config/sync0/users/`):**

```yaml
# ~/.config/sync0/users/alice.nt (skrivskyddad standard, hastighetsgräns 80 MB)
id: 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c
rate: 80MB
```

```yaml
# ~/.config/sync0/users/bob.nt (läs- och skrivbehörigheter, endast tillåtet att ladda upp till specificerad underkatalog)
id: 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f
perm: rw
write_prefix:
  - uploads/bob
  - shared/common
```

```yaml
# ~/.config/sync0/users/charlie.nt (endast skrivtillstånd för leverans, läsning är förbjuden)
id: 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
perm: w
```

**Exempel på konfiguration av delningsgrupp (`~/.config/sync0/config.nt`):**

```yaml
users_dir: ./users

groups:
  # Ej konfigurerade användare: skrivskyddat standardnätverk
  dataset:
    path: ./data
    rate: 50MB

  # Konfigurerade användare: begränsa läsningen av det offentliga nätverket automatiskt (endast Alice och Bob kan läsa, Charlie kan bara leverera och ladda upp)
  team-vault:
    path: ./vault
    users:
      - alice
      - bob
      - charlie
```

Starta tjänsten (läs in konfigurationsfilen eller använd kommandoradsparametrar direkt för att begränsa läsning av det offentliga nätverket och auktorisera medlemmar):

```bash
# Ladda ~/.config/sync0/config.nt för att starta
sync0 share --region SHA

# Eller publicera en privat delningsgrupp via ren kommandorad (ange --allow-* för att automatiskt begränsa läsning av offentliga nätverk och endast behöriga användare kan komma åt den)
sync0 share team-vault:./vault \
  --allow-r 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c \
  --allow-rw 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f:uploads/bob,shared/common \
  --allow-w 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
```

### 5. Nodadress, visitkort och flera språk

```bash
# Visa den lokala IP-fria nodadressen (nyckeln genereras automatiskt för första körningen)
sync0 id

# Generera Ed25519 självsignerat nodvisitkort (smeknamn, stadskod, e-post, webbplats, introduktion)
sync0 profile -u alice --region SHA --email alice@example.com -b "Shanghai node"

# Det finns 75 inbyggda språk, som följer systemspråket som standard och kan växlas genom miljövariabler.
SYNC0_LANG=en sync0 --help
```

## Prestanda och kärnfunktioner

### Tvärkontinentalt WAN faktisk mätning (jämförelse `rsync`)

Faktisk mätmiljö: USA `c1` → Europa `n1` (RTT 65 ms), datamängden är Linux-kärnan `linux-7.3.3`:

#### 1) A la carte-läge kontra `rsync`

| Datauppsättning | storlek | `sync0` Single Point<br>(Mbps) | `rsync`<br>(Mbps) | Snabbare (`sync0` / `rsync`) |
| :--- | ---: | ---: | ---: | ---: |
| `linux-7.3.3.tar.xz`(ej komprimerbar) | 155 MB | **236.48** | 108.74 | **2,17×** |
| `linux-7.3.3.tar`(komprimerbar) | 1.6 GB | **843.83** | 385.06 | **2,19×** |
| `linux-7.3.3.tar`(lokal förändringsökning) | 1.6 GB | **8,014.15** | 1,361.72 | **5,89×** |
| `linux-7.3.3/` (95 979 små filer) | 1.5 GB | **627.58** | 347.80 | **1,80×** |
| **geometriskt medelvärde** | — | **1,000.91** | 375.26 | **2,67×** |

#### 2) P2P-läge med dubbla källor jämfört med `rsync`

| Datauppsättning | storlek | `sync0` P2P Dual Source<br>(Mbps) | `rsync`<br>(Mbps) | Snabbare (`sync0` / `rsync`) |
| :--- | ---: | ---: | ---: | ---: |
| `linux-7.3.3.tar.xz`(ej komprimerbar) | 155 MB | **330.09** | 108.74 | **3,04×** |
| `linux-7.3.3.tar`(komprimerbar) | 1.6 GB | **1,551.86** | 385.06 | **4,03×** |
| `linux-7.3.3.tar`(lokal förändringsökning) | 1.6 GB | **5,474.17** | 1,361.72 | **4,02×** |
| `linux-7.3.3/` (95 979 små filer) | 1.5 GB | **706.56** | 347.80 | **2,03×** |
| **geometriskt medelvärde** | — | **1,186.42** | 375.26 | **3,16×** |

> Obs: I P2P-läge med dubbla källor samarbetar europeiska noder `s1` för seedning, och den gränsöverskridande ryggradens RTT är 65 ms.

### Kärnproduktens funktioner

- **Public key-adressering automatisk hålslagning**: Inget behov av en central server och offentlig nätverksadress, automatisk NAT-hålslagning baserat på den distribuerade nätverkets publika nyckeladress.
- **Millisekund-nivå realtidssynkronisering och metadatamedvetenhet**: Baserat på inbyggd filhändelseövervakning (`FSEvents` / `inotify` / `ReadDirectoryChangesW`), realtids push av filtillägg, raderingar, POSIX-behörigheter (`mode`) och millisekunds-nivåmodifieringstid="5">; noll datablocköverföring när endast behörigheter eller tid ändras.
- **Samtidig nedladdning och uppladdning i BT-stil samt regional affinitetsschemaläggning**: Nedladdningsnoden behöver inte vänta på att hela beloppet ska laddas ned. Alla blockintervall (`Op::Have`) som har klarat 128-bitars hash-kontrollen låses omedelbart i minnet/tillfälliga poolen för andra noder att dra samtidigt. Tillsammans med dynamiskt PEX-nodutbyte och långsam nodhalvförändring (Work-Stealing) maximeras klustrets genomströmning.
- **Immunitet mot skadlig nod och klusterförgiftning**: auktoritativ källlista `root_hash` Självkonsekvent låsning + obligatorisk hårdvaruacceleration `gxhash128` sammanfattningsverifiering innan varje block placeras på disken och seedas externt; om ett manipulerat block hittas kommer strömbrytaren på millisekundnivå att släcka den skadliga noden och automatiskt återhämta sig från den ärliga källan.
- **FastCDC in-situ inkrement och aggregering av små filer**: Endast differentiella block överförs för partiell modifiering av stora filer och placeras på disk; massivt minnespaket för små filer och differentiell komprimering av prefix eliminerar fil-för-fil-tur-och-retur-overhead.
- **Självläkande efter strömavbrott och återupptagande efter omstart**: Redis-liknande RDB + AOF hybridformat (`.s0_subs.bin`) används för att bevara seedningsstatus och fillista, dagliga ändringar läggs till i mikrosekunder och skrivs automatiskt om och organiseras regelbundet. Efter att servern har startat om skjuts offlineändringar automatiskt till såddnoden.

## Teknisk arkitektur och dokumentation

- **Protokoll för frånkoppling av två plan**:
  - **Kontroll- och förändringsyta i realtid (`iroh` QUIC / TCP, första byte `PROTO_VERSION` versionshandskakning) **: Ansvarig för Ed25519 visitkortsverifiering, behörighetsverifiering, upptäckt av Swarm Seed och stöd för `u64::MAX` nivå massförändring strömmande batching och `mzstd` realtid (compressed="4"> id="6"> / `FsOp::Remove` / `FsOp::Meta` Tre primitiver).
  - **Datadistributionsyta (native UDP + `compio` asynkron I/O) **: 20B kompakt huvud, 18B TACK periodisk återkoppling, BBRv3/Copa hybrid överbelastningskontroll, RACK-TLP paketförluståterställning, första block Shannon entropi adaptiv Zstd komprimering bypass, och <b id.="-2"> blockering end-toend.
- **Läs mer**:
  - [Resultatmätningsrapport och reproduktionsskript](https://github.com/webc-site/sync0/blob/main/doc/zh/benchmark.md)
  - [Design av systemarkitektur](https://github.com/webc-site/sync0/blob/main/doc/zh/design_v4.md)
  - [Binary Wire Protocol och Persistence Specification](https://github.com/webc-site/sync0/blob/main/doc/zh/proto.md)
  - [Identitetsautentisering, auktoritetskontroll och integritetsverifiering](https://github.com/webc-site/sync0/blob/main/doc/zh/crypto.md)