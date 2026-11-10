# sync0

`sync0` is een gedecentraliseerde tool voor het delen van P2P-bestanden, realtime synchronisatie en distributie van meerdere bronnen die wordt gebruikt ter vervanging van `rsync` - de gemeten snelheid van het transcontinentale WAN-netwerk is maximaal **6,95×** van `rsync` (geometrische gemiddelde snelheid op één punt **2,01×**, geometrische gemiddelde P2P dual-source snelheid **3,33×**), en de synchronisatie van enorme kleine bestanden over oceanen is zo snel als **7,19×**.

Elke machine is zowel een server als een client: er is geen centrale server of een openbaar netwerkadres nodig, automatisch perforeren is gebaseerd op het openbare sleuteladres van het gedistribueerde netwerk en ondersteunt directe overdracht tussen twee machines, hosting met meerdere gedeelde groepsrechten, meerdere selectie van submappen op aanvraag, real-time incrementele synchronisatie op millisecondenniveau en gelijktijdige P2P-versnelling met meerdere bronnen.

## Installeren

### Methode 1: Ladinginstallatie

```bash
cargo install sync0
```

### Methode 2: Voorgecompileerde binaire bestanden downloaden

Ga naar [GitHub Releases](https://github.com/webc-site/sync0/releases) om het `.tar.xz` gecomprimeerde pakket te downloaden dat overeenkomt met het systeem (ondersteunt Linux, macOS, Windows, Android, iOS, FreeBSD), pak het uit en plaats het in `PATH`:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Ga snel aan de slag

### 1. Directe verzending zonder configuratie tussen twee machines (`send` / `recv`)

U hoeft geen deelgroep te configureren, bestanden of mappen rechtstreeks en met hoge snelheid over te zetten tussen twee machines:

```bash
# De ontvangende kant luistert op de poort en schrijft naar ./backup
sync0 recv 0.0.0.0:9000 ./backup

# Directe verzending vanaf de afzender (optioneel – verwijder spiegel om overtollige bestanden op te ruimen, – snelheidslimiet van 50 MB)
sync0 send ./project 192.168.1.10:9000
```

### 2. Berichten delen en on-demand ophalen/uploaden (`share` / `pull` / `push`)

```bash
# Server: Publiceer de deelgroep met één opdrachtregel (voer het IP-vrije iroh-knooppuntadres uit na het opstarten)
sync0 share dataset:./data

# Client: Trek de volledige groep voor het delen van gegevenssets naar de lokale ./local_data
sync0 pull dataset:// ./local_data -p <knooppuntadres>

# Client: Haal op verzoek meerdere selecties op (synchroniseer alleen gespecificeerde afzonderlijke bestanden en submappen)
sync0 pull dataset://docs/readme.md,assets/icons,src/core ./local_data -p <knooppuntadres>

# Klant: Upload bestanden of mappen naar geautoriseerde deelgroepen
sync0 push dataset://releases/v1 ./build_output -p <knooppuntadres>
```

### 3. Realtime abonnementszaaiing en tweerichtingssynchronisatie tussen meerdere machines (`sub` / `group`)

Als er na het tot stand brengen van een lange verbinding een bestand **toevoegt, wijzigt, verwijdert, wijzigt van machtigingen of wijzigingstijd** plaatsvindt op de externe kant, wordt de lokale map synchroon bijgewerkt in milliseconden in realtime (nieuwe/gewijzigde grote bestanden halen alleen de verschilblokken op en werken ze op hun plaats bij, bij het verwijderen van bestanden worden automatisch lokale bestanden en lege mappen opgeschoond, en worden alleen machtigingen gewijzigd of tijdstempels van nul gegevensblokken rechtstreeks gewijzigd):

- **`sync0 sub`(Real-time mirroring in één richting + automatisch seeden)**: De lokale real-time volgt de toevoegingen, verwijderingen en wijzigingen van het deelgedeelte en dient automatisch als een P2P-seed-bron om gelijktijdige versnelling te bieden voor andere pull-knooppunten.
- **`sync0 group`(peer-to-peer tweerichtings real-time synchronisatie)**: als bestanden worden toegevoegd, verwijderd of gewijzigd op een computer in de groep, worden alle andere online knooppunten synchroon in realtime bijgewerkt en dienen ze als kiem voor elkaar.

```bash
# Abonneer u op de hele deelgroep: toevoegingen, verwijderingen en wijzigingen op afstand worden lokaal in realtime gesynchroniseerd en automatisch geplaatst
sync0 subdataset:// ./local_mirror -p <knooppuntadres>

# On-demand multi-select-abonnement: synchroniseer en see alleen gespecificeerde afzonderlijke bestanden en submappen in realtime
sync0 subdataset://models/llama.onnx,configs ./local_mirror -p <knooppuntadres>

# Tweeweg real-time synchronisatie met meerdere machines met peergroepen (ondersteunt ook volledige of komma meervoudige selectiepaden)
sync0 group team-docs:// ./docs -p <knooppuntadres>
```

### 4. Configuratie van team voor meerdere groepen en ledenrechten (`.nt` configuratie)

De standaardconfiguratiemap is `~/.config/sync0` (kan worden gewijzigd via `-c <directory>`), met behulp van het minimalistische [NestedText](https://nestedtext.org) (`.nt`) formaat:
- **Ondersteunt het beperken van het lezen van openbare netwerken (alleen geautoriseerde gebruikers kunnen lezen)**: Zolang de gedeelde groep is geconfigureerd met een `users:` ledenlijst (of de opdrachtregel `--allow-r` / `--allow-rw` / `--allow-w` / `--auth` specificeert), schakelt het systeem **automatisch privé-authenticatie in (`auth: true`)** en verbiedt het opsomming of lezen van anonieme knooppunten in het openbare netwerk, en P2P-abonnees op seeding-nodes zullen tegelijkertijd ook de witte lijst overnemen, waardoor alleen geautoriseerde gebruikers kunnen lezen; deelgroepen die niet zijn geconfigureerd met `users` zijn standaard openbaar en alleen-lezen (als u openbaar en alleen-lezen wilt zijn en alleen openstaat voor uploads naar specifieke leden, kunt u expliciet `auth: false` instellen).
- **Fijnmazige machtigingen voor lezen/schrijven en levering van submappen**: Leden zijn standaard alleen-lezen (`r` hoeft niet expliciet te worden geschreven) en ondersteunen `r` (alleen-lezen), `w` (alleen schrijven naar de dropbox, niet leesbaar), `rw` (lezen-schrijven) en `write_prefix` (beperk het voorvoegsel van de submap dat is toegestaan ​​voor uploaden).

**Voorbeeld van ledenconfiguratie (`~/.config/sync0/users/`):**

```yaml
# ~/.config/sync0/users/alice.nt (standaard alleen-lezen, snelheidslimiet 80 MB)
id: 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c
rate: 80MB
```

```yaml
# ~/.config/sync0/users/bob.nt (lees- en schrijfrechten, alleen toegestaan ​​om te uploaden naar de opgegeven submap)
id: 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f
perm: rw
write_prefix:
  - uploads/bob
  - shared/common
```

```yaml
# ~/.config/sync0/users/charlie.nt (alleen toestemming voor schrijfbezorging, lezen is verboden)
id: 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
perm: w
```

**Voorbeeld van configuratie van een deelgroep (`~/.config/sync0/config.nt`):**

```yaml
users_dir: ./users

groups:
  # Niet-geconfigureerde gebruikers: standaard openbaar netwerk alleen-lezen
  dataset:
    path: ./data
    rate: 50MB

  # Geconfigureerde gebruikers: automatisch het lezen van openbare netwerken beperken (alleen Alice en Bob kunnen lezen, Charlie kan alleen leveren en uploaden)
  team-vault:
    path: ./vault
    users:
      - alice
      - bob
      - charlie
```

Start de service (laad het configuratiebestand of gebruik direct opdrachtregelparameters om het lezen van openbare netwerken te beperken en leden te autoriseren):

```bash
# Laad ~/.config/sync0/config.nt om te starten
sync0 share --region SHA

# Of publiceer een privé-deelgroep via een pure opdrachtregel (specificeer --allow-* om het lezen van het openbare netwerk automatisch te beperken en alleen geautoriseerde gebruikers hebben er toegang toe)
sync0 share team-vault:./vault \
  --allow-r 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c \
  --allow-rw 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f:uploads/bob,shared/common \
  --allow-w 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
```

### 5. Knooppuntadres, visitekaartje en meertalig

```bash
# Bekijk het lokale IP-vrije knooppuntadres (sleutel wordt automatisch gegenereerd voor de eerste run)
sync0 id

# Genereer een Ed25519 zelfondertekend knooppunt-visitekaartje (bijnaam, stadscode, e-mail, website, introductie)
sync0 profile -u alice --region SHA --email alice@example.com -b "Shanghai node"

# Er zijn 75 ingebouwde talen, die standaard de systeemtaal volgen en kunnen worden geschakeld via omgevingsvariabelen.
SYNC0_LANG=en sync0 --help
```

## Prestaties en kernfuncties

### Werkelijke meting van cross-continentale WAN (vergelijking `rsync`)

Werkelijke meetomgeving: Verenigde Staten `c1` → Europa `n1` (RTT 65 ms), de dataset is Linux-kernel `linux-7.3.3`:

#### 1) A-la-cartemodus versus `rsync`

| Gegevensset | maat | `sync0` Eén punt<br>(Mbps) | `rsync`<br>(Mbps) | Versnellen (`sync0` / `rsync`) |
| :--- | ---: | ---: | ---: | ---: |
| `linux-7.3.3.tar.xz`(niet samendrukbaar) | 155 MB | **121.07** | 107.89 | **1,12×** |
| `linux-7.3.3.tar`(samendrukbaar) | 1.6 GB | **442.07** | 506.62 | **0,87×** |
| `linux-7.3.3.tar`(lokale wijzigingsverhoging) | 1.6 GB | **8,123.87** | 1,169.49 | **6,95×** |
| `linux-7.3.3/` (95.979 kleine bestanden) | 1.5 GB | **565.49** | 235.27 | **2,40×** |
| **geometrisch gemiddelde** | — | **704.17** | 350.19 | **2,01×** |

#### 2) P2P dual-source-modus versus `rsync`

| Gegevensset | maat | `sync0` P2P dubbele bron<br>(Mbps) | `rsync`<br>(Mbps) | Versnellen (`sync0` / `rsync`) |
| :--- | ---: | ---: | ---: | ---: |
| `linux-7.3.3.tar.xz`(niet samendrukbaar) | 155 MB | **466.85** | 107.89 | **4,33×** |
| `linux-7.3.3.tar`(samendrukbaar) | 1.6 GB | **992.17** | 506.62 | **1,96×** |
| `linux-7.3.3.tar`(lokale wijzigingsverhoging) | 1.6 GB | **5,666.84** | 1,169.49 | **4,85×** |
| `linux-7.3.3/` (95.979 kleine bestanden) | 1.5 GB | **703.30** | 235.27 | **2,99×** |
| **geometrisch gemiddelde** | — | **1,165.63** | 350.19 | **3,33×** |

> Opmerking: in de P2P dual-source-modus werken Europese knooppunten `s1` samen voor het seeden, en is de cross-continentale backbone RTT 65 ms.

### Kernproducteigenschappen

- **Public key adressering automatisch perforeren**: Geen behoefte aan een centrale server en een openbaar netwerkadres, automatisch NAT perforeren op basis van het gedistribueerde netwerk publieke sleuteladres.
- **Realtime synchronisatie op millisecondenniveau en bewustzijn van metadata**: Gebaseerd op native monitoring van bestandsgebeurtenissen (`FSEvents` / `inotify` / `ReadDirectoryChangesW`), real-time push van bestandstoevoegingen, verwijderingen, POSIX-machtigingen (`mode`) en wijzigingstijd op millisecondenniveau (`mtime`); verzending van nul datablokken wanneer alleen machtigingen of tijd worden gewijzigd.
- **BT-stijl gelijktijdig downloaden en uploaden en regionale affiniteitsplanning**: het downloadknooppunt hoeft niet te wachten tot het volledige bedrag is gedownload. Alle blokintervallen (`Op::Have`) die de 128-bits hash-controle hebben doorstaan, worden onmiddellijk vergrendeld in de geheugen/tijdelijke pool, zodat andere knooppunten deze gelijktijdig kunnen ophalen. Samen met de dynamische uitwisseling van PEX-knooppunten en de langzame voorrang op de halve knooppunten (Work-Stealing) wordt de clusterdoorvoer gemaximaliseerd.
- **Anti-kwaadaardige manipulatie van knooppunten en immuniteit tegen clustervergiftiging**: gezaghebbende bronlijst `root_hash` Zelfconsistente vergrendeling + verplichte hardwareversnelling `gxhash128` digest-verificatie voordat elk blok op de schijf wordt geplaatst en extern wordt gezaaid; Als er een blok wordt gevonden waarmee is geknoeid, zal de stroomonderbreker op millisecondenniveau het kwaadaardige knooppunt uitschakelen en automatisch herstellen van de eerlijke bron.
- **FastCDC in-situ verhoging en aggregatie van kleine bestanden**: alleen differentiële blokken worden overgedragen voor gedeeltelijke wijziging van grote bestanden en op schijf geplaatst; enorme geheugenverpakkingen voor kleine bestanden en differentiële compressie voor voorvoegsels elimineren overheadkosten per bestand.
- **Zelfherstel na stroomuitval en hervatting na herstart**: Redis-achtig RDB + AOF hybride formaat (`.s0_subs.bin`) wordt gebruikt om de seedingstatus en de bestandslijst te behouden, dagelijkse wijzigingen worden toegevoegd in microseconden en worden automatisch herschreven en regelmatig georganiseerd. Nadat de server opnieuw is opgestart, worden offline wijzigingen automatisch naar het Seeding-knooppunt gepusht.

## Technische architectuur en documentatie

- **Dual plane ontkoppelingsprotocol**:
  - **Controle en real-time wijzigingsoppervlak (`iroh` QUIC / TCP, eerste byte `PROTO_VERSION` versie handshake) **: verantwoordelijk voor Ed25519-visitekaartjeverificatie, toestemmingsverificatie, zwermzaaddetectie en ondersteuning voor massaveranderingsstreaming op `u64::MAX` niveau en `mzstd` gecomprimeerde `SubPush` real-time push (`FsOp::Upsert` / `FsOp::Remove` / `FsOp::Meta` Drie primitieven).
  - **Gegevensdistributieoppervlak (native UDP + `compio` asynchrone I/O) **: 20B compacte header, 18B TACK periodieke feedback, BBRv3/Copa hybride congestiecontrole, RACK-TLP herstel van pakketverlies, eerste-blok Shannon entropie-adaptieve Zstd-compressie-bypass en `gxhash128` blok end-to-end verificatie.
- **Meer informatie**:
  - [Prestatiemetingsrapport en reproductiescript](https://github.com/webc-site/sync0/blob/main/doc/zh/benchmark.md)
  - [Ontwerp van systeemarchitectuur](https://github.com/webc-site/sync0/blob/main/doc/zh/design_v4.md)
  - [Binair draadprotocol en persistentiespecificatie](https://github.com/webc-site/sync0/blob/main/doc/zh/proto.md)
  - [Identiteitsauthenticatie, autoriteitscontrole en integriteitsverificatie](https://github.com/webc-site/sync0/blob/main/doc/zh/crypto.md)