# sync0

`sync0` er et peer-to-peer (P2P) fildelings- og synkroniseringsværktøj til flere maskiner, der konkurrerer med Resilio Sync.

Hver maskine er både en server og en klient: enhver node kan udgive flere fildelingsgrupper. Efter at andre noder er tilsluttet, kan de automatisk hente tilgængelige seed noder fra peer-serveren og trække dem samtidig fra flere kilder. Når synkroniseringen er fuldført, oprettes der automatisk frø til flere noder for at fremskynde downloads.

## Kernehøjdepunkter

- **Alle er både en server og en klient**: Kan sammenlignes med Resilio Sync, ingen central serverafhængighed. Enhver node kan oprette flere fildelingsgrupper og kan også tilslutte sig andres delingsgrupper; noder udveksler og opnår automatisk tilgængelige seed noder for at opnå samtidig træk og realtidssynkronisering af flere maskiner.
- **IP-fri nodeadresse**: Som standard genereres nodenøglen og den `base64url` kodede `iroh` adresse automatisk i `~/.config/sync0` uden at skulle huske eller afsløre den offentlige IP.
- **Multi-select synkronisering on demand**: Understøtter synkronisering af hele delingsgruppen, og understøtter også synkronisering on demand ved at vælge flere filer eller undermapper på samme tid.
- **Hierarkiske tilladelser for gruppemedlemmer og kontrol af uploadbibliotek**: Understøtter skrivebeskyttet (`ro`), læse-skrive-upload (`rw`) og administrationstilladelser (`admin`), og kan individuelt angive det stipræfiks, som hvert medlem har tilladelse til at uploade (som standard er upload til rodmappen for delingen tilladt).
- **Fuld hastighed og trinvis synkronisering på tværs af netværk**: Samtidig transmission af store filer, aggregering af massive små filer i hukommelsen og kun synkroniserede datablokke, når filer er delvist ændret.

## Hastighedssammenligning med rsync

### Transoceanisk netværk enkelt kilde faktisk måling (Kina → USA, forsinkelse 260 ms)

| Transmissionsscenarie | Datastørrelse | model | `sync0` | `rsync -az` | Forøg multiplikator |
| :--- | :---: | :---: | :---: | :---: | :---: |
| 500 små filer | 1.91 MB | Fuld synkronisering | 18.84 Mbps | 5.32 Mbps | 3.5x |
| 500 små filer | 1.91 MB | Inkrementel synkronisering | 27.15 Mbps | 3.83 Mbps | 7.1x |
| Kode- og arkivpakker (`.tar`) | 10.0 MB | Fuld synkronisering | 139.70 Mbps | 24.97 Mbps | 5.6x |
| Kode- og arkivpakker (`.tar`) | 10.0 MB | Inkrementel synkronisering | 83.38 Mbps | 26.02 Mbps | 3.2x |
| Videofil (`.mp4`) | 15.0 MB | Fuld synkronisering | 54.06 Mbps | 27.04 Mbps | 2.0x |
| Videofil (`.mp4`) | 15.0 MB | Inkrementel synkronisering | 216.83 Mbps | 21.51 Mbps | 10.1x |

### Transoceanisk netværk P2P multi-node pull test (Kina → USA dobbeltknuder, forsinkelse 260 ms)

Målt sammenligning af multi-source seed node stribet samtidig pull:

| Transmissionsscenarie | Datastørrelse | Enkelt node seeding | To-node P2P samtidig seeding | Forøg multiplikator |
| :--- | :---: | :---: | :---: | :---: |
| 500 små filer | 1.97 MB | 5.28 Mbps | 7.87 Mbps | 1.5x |
| Kode- og arkivpakker (`.tar`) | 30.0 MB | 56.35 Mbps | 83.17 Mbps | 1.5x |
| Videofil (`.mp4`) | 30.0 MB | 27.00 Mbps | 39.46 Mbps | 1.5x |

### Faktisk måling af transkontinental dedikeret højhastighedslinje (Europa → Nordamerika, forsinkelse 86 ms)

| Transmissionsscenarie | Datastørrelse | `sync0` | `rsync` | Forøg multiplikator |
| :--- | :---: | :---: | :---: | :---: |
| 10.000 små filer | 77.6 MB | 688 Mbps | 128 Mbps | 5.4x |
| Kode- og arkivpakker (`.tar`) | 512 MB | 2111 Mbps | 438 Mbps | 4.8x |
| Videofil (`.mp4`) | 512 MB | 556 Mbps | 338 Mbps | 1.6x |
| Inkrementel synkronisering (rediger 1 % indhold) | 512 MB | 34133 Mbps | 2214 Mbps | 15.4x |

## Installere

### Metode 1: Installer via last

```bash
cargo install sync0
```

### Metode 2: Download prækompilerede binære filer

Gå til [GitHub Releases](https://github.com/webc-site/sync0/releases) for at downloade den `.tar.xz` komprimerede pakke til det tilsvarende system (understøtter Linux, macOS, Windows, Android, FreeBSD, NetBSD), pak den ud og læg den i `PATH` mappen:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Kom hurtigt i gang

`sync0` bruger `~/.config/sync0` som konfigurationsmappe som standard (hvis du har brug for at skifte til andre mapper, kan du tilføje `-c <directory>`).

### 1. Se den lokale nodeadresse og indstillingsvisitkort

```bash
# Se den lokale nodeadresse (nøglen genereres automatisk i ~/.config/sync0 for første gang)
sync0 id

# Generer et personligt node-visitkort (understøtter brugernavn, by, e-mail, hjemmeside og signatur inden for 256 ord)
sync0 profile -u alice --region SHA --email alice@example.com --url https://example.com -b "Shanghai node"
```

### 2. Udgiv flere fildelingsgrupper og indstil gruppemedlemstilladelser (`.nt` konfiguration)

`sync0` er konfigureret i formatet [NestedText](https://nestedtext.org) (`.nt`). Hver bruger har en separat `.nt` konfigurationsfil. Du kan angive brugerprofilbiblioteket gennem `users_dir` i `config.nt` (som automatisk inkluderer alle `.nt` filer i denne mappe). Både brugeren og stipræfikset, der må uploades, bruger en uordnet liste (`-`). **Når uploadsti-præfikset ikke er udfyldt, er upload til sharens rodbibliotek tilladt som standard**.

1. Opret separate `.nt` konfigurationsfiler for hvert medlem i `~/.config/sync0/users/` mappen:

`~/.config/sync0/users/alice.nt` (skrivebeskyttet medlem, prisbegrænset `80MB`, `rate` understøtter `GB`, `MB`, `KB` enheder):

```yaml
id: 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c
perm: ro
rate: 80MB
```

`~/.config/sync0/users/bob.nt` (medlemmer med begrænset upload: kun tilladt at uploade til `uploads/bob` og `shared/common` mapper):

```yaml
id: 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f
perm: rw
write_prefix:
  - uploads/bob
  - shared/common
```

`~/.config/sync0/users/charlie.nt` (Standard upload af medlemmer: Når `write_prefix` udelades, er upload til delingens rodbibliotek tilladt som standard):

```yaml
id: 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
perm: rw
```

2. Konfigurer brugerbiblioteket og flere fildelingsgrupper i `~/.config/sync0/config.nt`:

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

Begynd at dele tjeneste:

```bash
# Direkte opstart (indlæser automatisk alle .nt-brugerkonfigurationer i mapperne ~/.config/sync0/config.nt og users_dir)
sync0 share --region SHA

# Eller udgiv hurtigt flere delingsgrupper og godkend gruppemedlemmer direkte via kommandolinjen
sync0 share dataset:./data team-vault:./vault \
  --auth \
  --allow-ro 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c \
  --allow-rw 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f:uploads/bob,shared/common \
  --allow-rw 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
```

### 3. Træk alt indhold eller vælg flere filer og mapper (`sync0 pull`)

Opret forbindelse til en hvilken som helst peer node for automatisk at få alle tilgængelige seed noder i delingsgruppen, og træk fra flere seed noder samtidigt:

```bash
# Træk hele datasætdelingsgruppen til den lokale ./local_data
sync0 pull dataset:// ./local_data -p <nodeadresse>

# Multi-select synkronisering: træk samtidig den enkelte fil docs/readme.md og undermapper assets/ikoner, src/core
sync0 pull dataset://docs/readme.md,assets/icons,src/core ./local_data -p <node address>
```

### 4. Upload en fil eller et bibliotek til en delingsgruppe (`sync0 push`)

Medlemmer med uploadtilladelser (`rw` eller `admin`) kan uploade lokale filer eller mapper til fjerndelingsgrupper:

```bash
# Medlemmer med tilladelse til upload af rodbibliotek: Upload direkte til den delte rodmappe
sync0 push team-vault:// ./new_release -p <nodeadresse>

# Kun medlemmer, der er godkendt til et specifikt præfiks (såsom uploads/bob): upload til det tilladte stipræfiks
sync0 push team-vault://uploads/bob/report.pdf ./report.pdf -p <nodeadresse>
```

### 5. Abonner på automatisk synkronisering og seeding (`sync0 sub`)

Efter at have abonneret på delingsgruppen for en bestemt node, er den lokale server både en modtagende klient og en ny seeding-server; når kildenodefilen opdateres, synkroniseres den automatisk og giver chunked downloads til andre noder:

```bash
# Abonner på hele delingsgruppen og seed automatisk
sync0 underdatasæt:// ./local_mirror -p <nodeadresse>

# Vælg kun flere gange for at abonnere på specificerede filer og undermapper og se dem
sync0 underdatasæt://models/llama.onnx,configs ./local_mirror -p <nodeadresse>
```

### 6. Peer-synkronisering af flere maskiner (`sync0 group`)

Flere maskiner slutter sig til den samme delingsgruppe, hinanden opdager automatisk tilgængelige frøknuder og opretholder tovejssynkronisering (understøtter fuld synkronisering eller flere valg af filer og mapper):

```bash
# Deltag fuldt ud i gruppen og to-vejs synkronisering
sync0 group team-docs:// ./docs -p <nodeadresse>

# Kun wal-mappen og meta.json-filen i multi-select-synkroniseringsgruppen
sync0 group team-docs://wal,meta.json ./docs -p <node address>
```

### 7. Direkte punkt-til-punkt transmission mellem to maskiner (`send` / `recv`)

Det er ikke nødvendigt at forudkonfigurere en gruppe, og du kan midlertidigt overføre filer eller mapper direkte mellem to maskiner:

```bash
# Modtager overvågning
sync0 recv 0.0.0.0:9000 ./backup

# Afsenderenden overfører filer eller mapper direkte (valgfrit --slet spejl for at rydde op i overskydende filer, --hastighedsgrænse)
sync0 send ./project 192.168.1.10:9000
```

### 8. Skift på flere sprog

`sync0` understøtter 75 sprog og skifter automatisk i henhold til systemsprogmiljøet som standard; det kan også angives manuelt gennem `SYNC0_LANG`:

```bash
SYNC0_LANG=en sync0 --help
SYNC0_LANG=ja sync0 --help
SYNC0_LANG=de sync0 --help
```