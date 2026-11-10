# sync0

`sync0` er et desentralisert P2P-fildelings-, sanntidssynkroniserings- og distribusjonsverktøy for flere kilder som brukes til å erstatte `rsync` - den målte hastigheten til transkontinentalt WAN er opp til `rsync`s **5,89×** (enkeltpunkts geometrisk gjennomsnittshastighet **2,67×-**, P3P dual hastighet **, P3P dual hastighet **, P2P dual speed **, P3P dual speed **. transoceanisk høylatens-link P2P-akselerasjon er opptil **7,68×** (full-link 24-scenario omfattende geometrisk gjennomsnittshastighet **2,56×**).

Hver maskin er både en server og en klient: det er ikke behov for en sentral server eller en offentlig nettverksadresse, automatisk hullstansing er basert på den distribuerte nettverkets offentlige nøkkeladresse, og den støtter direkte overføring mellom to maskiner, hosting for multideling av gruppetillatelser, flere valg av underkataloger på forespørsel, inkrementell sanntidssynkronisering på millisekundnivå og samtidig P2P-akselerasjon.

## Installer

### Metode 1: Lasteinstallasjon

```bash
cargo install sync0
```

### Metode 2: Last ned forhåndskompilerte binærfiler

Gå til [GitHub Releases](https://github.com/webc-site/sync0/releases) for å laste ned den `.tar.xz` komprimerte pakken som tilsvarer systemet (støtter Linux, macOS, Windows, Android, iOS, FreeBSD), pakk ut og legg inn i `PATH`:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Kom raskt i gang

### 1. Nullkonfigurasjon direkte overføring mellom to maskiner (`send` / `recv`)

Du trenger ikke å konfigurere en delingsgruppe, overføre filer eller kataloger direkte mellom to maskiner med høy hastighet:

```bash
# Mottakeren lytter på porten og skriver til ./backup
sync0 recv 0.0.0.0:9000 ./backup

# Direkte overføring fra avsender (valgfritt --slett speil for å rydde opp i overflødige filer, -- rate 50 MB hastighetsgrense)
sync0 send ./project 192.168.1.10:9000
```

### 2. Deling av innlegg og trekk/opplasting på forespørsel (`share` / `pull` / `push`)

```bash
# Server: Publiser delingsgruppen med én kommandolinje (skriv ut den IP-frie iroh-nodeadressen etter oppstart)
sync0 share dataset:./data

# Klient: Trekk hele datasettdelingsgruppen til den lokale ./local_data
sync0 pull dataset:// ./local_data -p <nodeadresse>

# Klient: Trekk flere valg på forespørsel (synkroniser bare spesifiserte enkeltfiler og underkataloger)
sync0 pull dataset://docs/readme.md,assets/icons,src/core ./local_data -p <node address>

# Klient: Last opp filer eller kataloger til autoriserte delingsgrupper
sync0 push datasett://releases/v1 ./build_output -p <nodeadresse>
```

### 3. Abonnementssøking i sanntid og toveissynkronisering med flere maskiner (`sub` / `group`)

Etter å ha etablert en lang tilkobling, hvis en fil **tilføyelse, modifisering, sletting, endring av tillatelser eller modifikasjonstid** skjer på den eksterne enden, vil den lokale katalogen oppdateres synkront i millisekunder i sanntid (nye/modifiserte store filer vil bare trekke forskjellsblokkene og oppdatere dem på plass, slette filer vil automatisk rydde opp i lokale filer og tomme kataloger, og bare endre amp-tillatelser direkte):

- **`sync0 sub`(Enveis sanntidsspeiling + automatisk seeding)**: Den lokale sanntiden følger tilleggene, slettingene og modifikasjonene av delingsenden, og fungerer automatisk som en P2P-frøkilde for å gi samtidig akselerasjon for andre trekkende noder.
- **`sync0 group`(Multi-maskin node-til-node toveis sanntidssynkronisering)**: Hvis filer legges til, slettes eller endres på en hvilken som helst maskin i gruppen, vil alle andre online noder oppdateres synkront i sanntid og fungere som frø for hverandre.

```bash
# Abonner på hele delingsgruppen: eksterne tillegg, slettinger og modifikasjoner synkroniseres lokalt i sanntid og sees automatisk
sync0 underdatasett:// ./local_mirror -p <nodeadresse>

# On-demand multi-select abonnement: bare synkroniser og sett spesifiserte enkeltfiler og underkataloger i sanntid
sync0 underdatasett://models/llama.onnx,configs ./local_mirror -p <nodeadresse>

# Multi-maskin peer group toveis sanntidssynkronisering (støtter også full- eller komma-multivalgstier)
sync0 group team-docs:// ./docs -p <node address>
```

### 4. Konfigurasjon av flerdelingsgruppe og medlemstillatelse (`.nt` konfigurasjon)

Standard konfigurasjonskatalog er `~/.config/sync0` (kan byttes via `-c <katalog>`), ved å bruke det minimalistiske [NestedText](https://nestedtext.org) (`.nt`) formatet:
- **Støtter begrensning av offentlig nettverkslesing (kun autoriserte brukere kan lese)**: Så lenge delingsgruppen er konfigurert med en `users:` medlemsliste (eller kommandolinjen spesifiserer `--allow-r` / `--allow-rw` / `--allow-w` / `--auth`), slår systemet **automatisk på anonyme nettverk (`users` er offentlige og skrivebeskyttede som standard (hvis du ønsker å være offentlige og skrivebeskyttede og åpne for opplastinger kun til spesifiserte medlemmer, kan du eksplisitt angi `auth: false`).
- **Fint lese-skrive- og underkatalogleveringstillatelser**: Medlemmer er skrivebeskyttet som standard (`r` trenger ikke å skrives eksplisitt), og støtter `r` (skrivebeskyttet), `w` (bare skriv til drop-boksen, ikke lesbar), `rw` (read-limit id e) og < (read-limit id e) laster opp).

**Eksempel på medlemskonfigurasjon (`~/.config/sync0/users/`):**

```yaml
# ~/.config/sync0/users/alice.nt (standard skrivebeskyttet, hastighetsgrense 80 MB)
id: 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c
rate: 80MB
```

```yaml
# ~/.config/sync0/users/bob.nt (lese- og skrivetillatelser, kun tillatt å laste opp til spesifisert underkatalog)
id: 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f
perm: rw
write_prefix:
  - uploads/bob
  - shared/common
```

```yaml
# ~/.config/sync0/users/charlie.nt (bare skrive leveringstillatelse, lesing er forbudt)
id: 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
perm: w
```

**Eksempel på konfigurasjon av delingsgruppe (`~/.config/sync0/config.nt`):**

```yaml
users_dir: ./users

groups:
  # Ikke konfigurerte brukere: standard offentlig nettverk skrivebeskyttet
  dataset:
    path: ./data
    rate: 50MB

  # Konfigurerte brukere: automatisk begrens offentlig nettverkslesing (bare Alice og Bob kan lese, Charlie kan bare levere og laste opp)
  team-vault:
    path: ./vault
    users:
      - alice
      - bob
      - charlie
```

Start tjenesten (last inn konfigurasjonsfilen eller bruk kommandolinjeparametere direkte for å begrense offentlig nettverkslesing og autorisere medlemmer):

```bash
# Last inn ~/.config/sync0/config.nt for å starte
sync0 share --region SHA

# Eller publiser en privat delingsgruppe via ren kommandolinje (spesifiser --allow-* for automatisk å begrense offentlig nettverkslesing og bare autoriserte brukere kan få tilgang til den)
sync0 share team-vault:./vault \
  --allow-r 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c \
  --allow-rw 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f:uploads/bob,shared/common \
  --allow-w 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
```

### 5. Nodeadresse, visittkort og flerspråk

```bash
# Se den lokale IP-frie nodeadressen (nøkkelen genereres automatisk for første kjøring)
sync0 id

# Generer Ed25519 selvsignert node visittkort (kallenavn, bykode, e-post, nettside, introduksjon)
sync0 profile -u alice --region SHA --email alice@example.com -b "Shanghai node"

# Det er 75 innebygde språk, som følger systemspråket som standard og kan byttes gjennom miljøvariabler.
SYNC0_LANG=en sync0 --help
```

## Ytelse og kjernefunksjoner

### Faktisk måling på tvers av kontinentale WAN (sammenligning `rsync`)

Faktisk målemiljø: USA `c1` → Europa `n1` (RTT 65 ms), datasettet er Linux-kjernen `linux-7.3.3`:

#### 1) A la carte-modus kontra `rsync`

| Datasett | størrelse | `sync0` Enkeltpunkt<br>(Mbps) | `rsync`<br>(Mbps) | Øk hastighet (`sync0` / `rsync`) |
| :--- | ---: | ---: | ---: | ---: |
| `linux-7.3.3.tar.xz`(ikke komprimerbar) | 155 MB | **236.48** | 108.74 | **2,17×** |
| `linux-7.3.3.tar`(komprimerbar) | 1.6 GB | **843.83** | 385.06 | **2,19×** |
| `linux-7.3.3.tar`(lokal endring) | 1.6 GB | **8,014.15** | 1,361.72 | **5,89×** |
| `linux-7.3.3/` (95 979 små filer) | 1.5 GB | **627.58** | 347.80 | **1,80×** |
| **geometrisk gjennomsnitt** | — | **1,000.91** | 375.26 | **2,67×** |

#### 2) P2P dual-source-modus vs `rsync`

| Datasett | størrelse | `sync0` P2P Dual Source<br>(Mbps) | `rsync`<br>(Mbps) | Øk hastighet (`sync0` / `rsync`) |
| :--- | ---: | ---: | ---: | ---: |
| `linux-7.3.3.tar.xz`(ikke komprimerbar) | 155 MB | **330.09** | 108.74 | **3,04×** |
| `linux-7.3.3.tar`(komprimerbar) | 1.6 GB | **1,551.86** | 385.06 | **4,03×** |
| `linux-7.3.3.tar`(lokal endring) | 1.6 GB | **5,474.17** | 1,361.72 | **4,02×** |
| `linux-7.3.3/` (95 979 små filer) | 1.5 GB | **706.56** | 347.80 | **2,03×** |
| **geometrisk gjennomsnitt** | — | **1,186.42** | 375.26 | **3,16×** |

> Merk: I P2P dual-source-modus samarbeider europeiske noder `s1` for seeding, og den tverrkontinentale ryggraden RTT er 65 ms.

### Kjerneproduktfunksjoner

- **Public nøkkeladressering automatisk hullstansing**: Ikke behov for en sentral server og offentlig nettverksadresse, automatisk NAT-hullstansing basert på den distribuerte nettverkets offentlige nøkkeladresse.
- **Millisekund-nivå sanntidssynkronisering og metadatabevissthet**: Basert på opprinnelig filhendelsesovervåking (`FSEvents` / `inotify` / `ReadDirectoryChangesW`), sanntids push av filtilføyelser, slettinger, POSIX-tillatelser (`mode`) og millisekund-nivå modifikasjonstid="5">; null datablokkoverføring når bare tillatelser eller tid er endret.
- **Samtidig nedlasting og opplasting i BT-stil og regional tilhørighetsplanlegging**: Nedlastingsnoden trenger ikke å vente på at hele beløpet skal lastes ned. Alle blokkintervallene (`Op::Have`) som har bestått 128-bits hash-sjekken blir umiddelbart låst i minnet/midlertidig basseng for andre noder å trekke samtidig. Sammen med dynamisk PEX-nodeutveksling og langsom nodehalvforkjøp (Work-Stealing), maksimeres klyngegjennomstrømningen.
- **Anti-ondsinnet node-tukling og klyngeforgiftningsimmunitet**: autoritativ kildeliste `root_hash` Selvkonsistent låsing + obligatorisk maskinvareakselerasjon `gxhash128` fordøyelsesverifisering før hver blokk plasseres på disken og sees eksternt; hvis en manipulert blokk blir funnet, vil kretsbryteren på millisekundnivå slå ut den ondsinnede noden og automatisk gjenopprette fra den ærlige kilden.
- **FastCDC in-situ inkrement og aggregering av små filer**: Bare differensialblokker overføres for delvis modifisering av store filer og plasseres på disk; massiv minneinnpakning for små filer og prefiksdifferensiell komprimering eliminerer fil-for-fil rundtur-overhead.
- **Selvhelbredende etter strømbrudd og gjenopptagelse etter omstart**: Redis-lignende RDB + AOF hybridformat (`.s0_subs.bin`) brukes for å opprettholde seedingstatus og filliste, daglige endringer legges til i mikrosekunder, og blir automatisk omskrevet og organisert regelmessig. Etter at serveren er startet på nytt, blir offline-endringer automatisk skjøvet til seeding-noden.

## Teknisk arkitektur og dokumentasjon

- **Dobbelt plan avkoblingsprotokoll**:
  - **Kontroll- og endringsoverflate i sanntid (`iroh` QUIC / TCP, første byte `PROTO_VERSION` versjonshåndtrykk) **: Ansvarlig for Ed25519 visittkortverifisering, tillatelsesverifisering, Swarm-frø-oppdagelse og støtte for `u64::MAX` nivåmasseendring-strømming og `mzstd` sanntid (push id="4"> id="6"> / `FsOp::Remove` / `FsOp::Meta` Tre primitiver).
  - **Datadistribusjonsoverflate (native UDP + `compio` asynkron I/O) **: 20B kompakt header, 18B TACK periodisk tilbakemelding, BBRv3/Copa hybrid overbelastningskontroll, RACK-TLP pakketapsgjenoppretting, førsteblokk Shannon entropi adaptiv Zstd komprimering bypass, og <b id.="-2"> blokkering end-toend.
- **Les mer**:
  - [Resultatmålingsrapport og reproduksjonsskript](https://github.com/webc-site/sync0/blob/main/doc/zh/benchmark.md)
  - [Systemarkitekturdesign](https://github.com/webc-site/sync0/blob/main/doc/zh/design_v4.md)
  - [Binary Wire Protocol og Persistence Specification](https://github.com/webc-site/sync0/blob/main/doc/zh/proto.md)
  - [Identitetsautentisering, autoritetskontroll og integritetsverifisering](https://github.com/webc-site/sync0/blob/main/doc/zh/crypto.md)