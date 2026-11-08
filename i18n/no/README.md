# sync0

`sync0` er et peer-to-peer (P2P) fildelings- og synkroniseringsverktøy for flere maskiner som konkurrerer med Resilio Sync.

Hver maskin er både en server og en klient: enhver node kan publisere flere fildelingsgrupper. Etter at andre noder blir med, kan de automatisk hente tilgjengelige frønoder fra peer-serveren og hente dem samtidig fra flere kilder. Etter at synkroniseringen er fullført, opprettes frø automatisk for flere noder for å akselerere nedlastinger.

## Kjernehøydepunkter

- **Alle er både en server og en klient**: Kan sammenlignes med Resilio Sync, ingen sentral serveravhengighet. Enhver node kan opprette flere fildelingsgrupper, og kan også bli med i andres delingsgrupper; noder automatisk utveksler og skaffer tilgjengelige frønoder for å oppnå samtidig trekking og sanntidssynkronisering av flere maskiner.
- **IP-fri nodeadresse**: Som standard genereres nodenøkkelen og `base64url` kodet `iroh` adresse automatisk i `~/.config/sync0` uten å måtte huske eller eksponere den offentlige IP-en.
- **Multi-select synkronisering på forespørsel**: Støtter synkronisering av hele delingsgruppen, og støtter også synkronisering på forespørsel ved å velge flere filer eller underkataloger samtidig.
- **Hierarkiske tillatelser for gruppemedlem og opplastingskatalog**: Støtter skrivebeskyttet (`ro`), lese-skriveopplasting (`rw`) og administrasjonsrettigheter (`admin`), og kan individuelt spesifisere baneprefikset som hvert medlem har lov til å laste opp (som standard er opplasting til rotkatalogen til delingen tillatt).
- **Fullhastighets og inkrementell synkronisering på tvers av nettverk**: Samtidig overføring av store filer, aggregering av massive små filer i minnet, og bare synkroniserte datablokker når filer er delvis modifisert.

## Hastighetssammenligning med rsync

### Transoceanisk nettverk enkelt kilde faktisk måling (Kina → USA, forsinkelse 260 ms)

| Overføringsscenario | Datastørrelse | modell | `sync0` | `rsync -az` | Øk multiplikatoren |
| :--- | :---: | :---: | :---: | :---: | :---: |
| 500 små filer | 1.91 MB | Full synkronisering | 18.84 Mbps | 5.32 Mbps | 3.5x |
| 500 små filer | 1.91 MB | Inkrementell synkronisering | 27.15 Mbps | 3.83 Mbps | 7.1x |
| Kode og arkivpakker (`.tar`) | 10.0 MB | Full synkronisering | 139.70 Mbps | 24.97 Mbps | 5.6x |
| Kode og arkivpakker (`.tar`) | 10.0 MB | Inkrementell synkronisering | 83.38 Mbps | 26.02 Mbps | 3.2x |
| Videofil (`.mp4`) | 15.0 MB | Full synkronisering | 54.06 Mbps | 27.04 Mbps | 2.0x |
| Videofil (`.mp4`) | 15.0 MB | Inkrementell synkronisering | 216.83 Mbps | 21.51 Mbps | 10.1x |

### Transoceanisk nettverk P2P multi-node pull test (Kina → USA doble noder, forsinkelse 260 ms)

Målt sammenligning av multi-kilde frø node stripet samtidig trekk:

| Overføringsscenario | Datastørrelse | Enkel node seeding | To-node P2P samtidig seeding | Øk multiplikatoren |
| :--- | :---: | :---: | :---: | :---: |
| 500 små filer | 1.97 MB | 5.28 Mbps | 7.87 Mbps | 1.5x |
| Kode og arkivpakker (`.tar`) | 30.0 MB | 56.35 Mbps | 83.17 Mbps | 1.5x |
| Videofil (`.mp4`) | 30.0 MB | 27.00 Mbps | 39.46 Mbps | 1.5x |

### Faktisk måling av transkontinental høyhastighets dedikert linje (Europa → Nord-Amerika, forsinkelse 86 ms)

| Overføringsscenario | Datastørrelse | `sync0` | `rsync` | Øk multiplikatoren |
| :--- | :---: | :---: | :---: | :---: |
| 10 000 små filer | 77.6 MB | 688 Mbps | 128 Mbps | 5.4x |
| Kode og arkivpakker (`.tar`) | 512 MB | 2111 Mbps | 438 Mbps | 4.8x |
| Videofil (`.mp4`) | 512 MB | 556 Mbps | 338 Mbps | 1.6x |
| Inkrementell synkronisering (endre 1 % innhold) | 512 MB | 34133 Mbps | 2214 Mbps | 15.4x |

## Installer

### Metode 1: Installer via Cargo

```bash
cargo install sync0
```

### Metode 2: Last ned forhåndskompilerte binærfiler

Gå til [GitHub Releases](https://github.com/webc-site/sync0/releases) for å laste ned den `.tar.xz` komprimerte pakken for det tilsvarende systemet (støtter Linux, macOS, Windows, Android, FreeBSD, NetBSD), pakk ut og legg den inn i `PATH`-katalogen:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Kom raskt i gang

`sync0` bruker `~/.config/sync0` som konfigurasjonskatalog som standard (hvis du trenger å bytte til andre kataloger, kan du legge til `-c <katalog>`).

### 1. Se den lokale nodeadressen og innstillingsvisitkortet

```bash
# Se den lokale nodeadressen (nøkkelen genereres automatisk i ~/.config/sync0 for første gang)
sync0 id

# Generer et personlig node visittkort (støtter brukernavn, by, e-post, nettside og signatur på 256 ord)
sync0 profile -u alice --region SHA --email alice@example.com --url https://example.com -b "Shanghai node"
```

### 2. Publiser flere fildelingsgrupper og angi gruppemedlemstillatelser (`.nt` konfigurasjon)

`sync0` er konfigurert i formatet [NestedText](https://nestedtext.org) (`.nt`). Hver bruker har en separat `.nt` konfigurasjonsfil. Du kan spesifisere brukerprofilkatalogen gjennom `users_dir` i `config.nt` (som automatisk inkluderer alle `.nt` filer i denne katalogen). Både brukeren og baneprefikset som tillates lastet opp bruker en uordnet liste (`-`). **Når opplastingsbaneprefikset ikke er fylt ut, er opplasting til rotkatalogen til delingen tillatt som standard**.

1. Opprett separate `.nt` konfigurasjonsfiler for hvert medlem i `~/.config/sync0/users/`-katalogen:

`~/.config/sync0/users/alice.nt` (skrivebeskyttet medlem, prisbegrenset `80MB`, `rate` støtter `GB`, `MB`, `KB` enheter):

```yaml
id: 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c
perm: ro
rate: 80MB
```

`~/.config/sync0/users/bob.nt` (medlemmer med begrenset opplasting: kun tillatt å laste opp til `uploads/bob` og `shared/common` kataloger):

```yaml
id: 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f
perm: rw
write_prefix:
  - uploads/bob
  - shared/common
```

`~/.config/sync0/users/charlie.nt` (Standard medlemsopplasting: Når `write_prefix` er utelatt, er opplasting til rotkatalogen til delingen tillatt som standard):

```yaml
id: 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
perm: rw
```

2. Konfigurer brukerkatalogen og flere fildelingsgrupper i `~/.config/sync0/config.nt`:

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

Begynn å dele tjeneste:

```bash
# Direkte oppstart (laster automatisk alle .nt-brukerkonfigurasjoner i ~/.config/sync0/config.nt og users_dir-kataloger)
sync0 share --region SHA

# Eller publiser raskt flere delingsgrupper og autoriser gruppemedlemmer direkte via kommandolinjen
sync0 share dataset:./data team-vault:./vault \
  --auth \
  --allow-ro 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c \
  --allow-rw 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f:uploads/bob,shared/common \
  --allow-rw 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
```

### 3. Trekk alt innhold eller velg flere filer og kataloger (`sync0 pull`)

Koble til en hvilken som helst node for å automatisk hente alle tilgjengelige frønoder i delingsgruppen, og trekk fra flere frønoder samtidig:

```bash
# Trekk hele datasettdelingsgruppen til den lokale ./local_data
sync0 pull dataset:// ./local_data -p <nodeadresse>

# Flervalgssynkronisering: trekk samtidig enkeltfilen docs/readme.md og underkataloger assets/ikoner, src/core
sync0 pull dataset://docs/readme.md,assets/icons,src/core ./local_data -p <node address>
```

### 4. Last opp en fil eller katalog til en delingsgruppe (`sync0 push`)

Medlemmer med opplastingstillatelser (`rw` eller `admin`) kan laste opp lokale filer eller kataloger til eksterne delingsgrupper:

```bash
# Medlemmer med rotkatalogopplastingstillatelse: Last opp direkte til den delte rotkatalogen
sync0 push team-vault:// ./new_release -p <nodeadresse>

# Kun medlemmer som er autorisert til et spesifikt prefiks (som opplastinger/bob): last opp til det tillatte baneprefikset
sync0 push team-vault://uploads/bob/report.pdf ./report.pdf -p <node address>
```

### 5. Abonner på automatisk synkronisering og seeding (`sync0 sub`)

Etter å ha abonnert på delingsgruppen til en bestemt node, er den lokale serveren både en mottakende klient og en ny seeding-server; når kildenodefilen oppdateres, synkroniseres den automatisk og gir nedlastinger i biter for andre noder:

```bash
# Abonner på hele delingsgruppen og seed automatisk
sync0 underdatasett:// ./local_mirror -p <nodeadresse>

# Bare multivelg for å abonnere på spesifiserte filer og underkataloger og se dem
sync0 underdatasett://models/llama.onnx,configs ./local_mirror -p <nodeadresse>
```

### 6. Gruppesynkronisering med flere maskiner (`sync0 group`)

Flere maskiner blir med i samme delingsgruppe, hverandre oppdager automatisk tilgjengelige frønoder og opprettholder toveissynkronisering (støtter full synkronisering eller flere utvalg av filer og kataloger):

```bash
# Bli helt med i gruppen og toveis synkronisering
sync0 group team-docs:// ./docs -p <node address>

# Bare wal-katalogen og meta.json-filen i multi-select synkroniseringsgruppen
sync0 group team-docs://wal,meta.json ./docs -p <node address>
```

### 7. Punkt-til-punkt direkte overføring mellom to maskiner (`send` / `recv`)

Det er ikke nødvendig å forhåndskonfigurere en gruppe, og du kan midlertidig overføre filer eller kataloger direkte mellom to maskiner:

```bash
# Mottakerovervåking
sync0 recv 0.0.0.0:9000 ./backup

# Sendeenden overfører filer eller kataloger direkte (valgfritt --slett speil for å rydde opp i overflødige filer, --hastighetshastighetsgrense)
sync0 send ./project 192.168.1.10:9000
```

### 8. Bytting av flere språk

`sync0` støtter 75 språk, og bytter automatisk i henhold til systemspråkmiljøet som standard; det kan også spesifiseres manuelt gjennom `SYNC0_LANG`:

```bash
SYNC0_LANG=en sync0 --help
SYNC0_LANG=ja sync0 --help
SYNC0_LANG=de sync0 --help
```