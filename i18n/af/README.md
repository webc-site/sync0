# sync0

`sync0` is 'n peer-to-peer (P2P) lêerdeling en multi-masjien sinchronisasie hulpmiddel wat meeding met Resilio Sync.

Elke masjien is beide 'n bediener en 'n kliënt: enige nodus kan verskeie lêerdeelgroepe publiseer. Nadat ander nodusse aangesluit het, kan hulle outomaties beskikbare saad nodusse van die eweknie-bediener verkry en dit gelyktydig uit verskeie bronne trek. Nadat sinchronisasie voltooi is, sal sade outomaties geskep word vir meer nodusse om aflaaie te versnel.

## Kernhoogtepunte

- **Almal is beide 'n bediener en 'n kliënt**: Vergelykbaar met Resilio Sync, geen sentrale bedienerafhanklikheid nie. Enige nodus kan veelvuldige lêerdeelgroepe skep, en kan ook by ander mense se deelgroepe aansluit; nodusse ruil outomaties en verkry beskikbare saadnodusse om gelyktydige trek en intydse sinchronisasie van verskeie masjiene te bewerkstellig.
- **IP-vrye nodusadres**: By verstek word die nodussleutel en `base64url` geënkodeerde `iroh` adres outomaties in `~/.config/sync0` gegenereer sonder om die publieke IP te onthou of bloot te lê.
- **Multi-keuse sinchronisasie op aanvraag**: Ondersteun sinchronisasie van die hele deelgroep, en ondersteun ook sinchronisasie op aanvraag deur verskeie lêers of subgidse op dieselfde tyd te kies.
- **Groeplid hiërargiese toestemmings en oplaaigidsbeheer**: Ondersteun leesalleen (`ro`), lees-skryf-oplaai (`rw`), en bestuurs- (`admin`) toestemmings, en kan individueel die padvoorvoegsel spesifiseer wat elke lid toegelaat word om op te laai (by verstek, oplaai na die wortelgids van die deel word toegelaat).
- **Volspoed en inkrementele sinchronisasie oor netwerke**: Gelyktydige oordrag van groot lêers, samevoeging van massiewe klein lêers in die geheue, en slegs gesinchroniseerde datablokke wanneer lêers gedeeltelik gewysig word.

## Spoedvergelyking met rsync

### Transoseaniese netwerk enkele bron werklike meting (China → Verenigde State, vertraging 260 ms)

| Transmissie scenario | Data grootte | model | `sync0` | `rsync -az` | Verhoog vermenigvuldiger |
| :--- | :---: | :---: | :---: | :---: | :---: |
| 500 klein lêers | 1.91 MB | Volledige sinchronisasie | 18.84 Mbps | 5.32 Mbps | 3.5x |
| 500 klein lêers | 1.91 MB | Inkrementele sinchronisasie | 27.15 Mbps | 3.83 Mbps | 7.1x |
| Kode en argief pakkette (`.tar`) | 10.0 MB | Volledige sinchronisasie | 139.70 Mbps | 24.97 Mbps | 5.6x |
| Kode en argief pakkette (`.tar`) | 10.0 MB | Inkrementele sinchronisasie | 83.38 Mbps | 26.02 Mbps | 3.2x |
| Videolêer (`.mp4`) | 15.0 MB | Volledige sinchronisasie | 54.06 Mbps | 27.04 Mbps | 2.0x |
| Videolêer (`.mp4`) | 15.0 MB | Inkrementele sinchronisasie | 216.83 Mbps | 21.51 Mbps | 10.1x |

### Transoseaniese netwerk P2P multi-node trek toets (China → Verenigde State dubbele nodusse, vertraging 260 ms)

Gemeet vergelyking van multi-bron saad node gestreepte gelyktydige trek:

| Transmissie scenario | Data grootte | Enkelnodus saai | Twee-node P2P gelyktydige saai | Verhoog vermenigvuldiger |
| :--- | :---: | :---: | :---: | :---: |
| 500 klein lêers | 1.97 MB | 5.28 Mbps | 7.87 Mbps | 1.5x |
| Kode en argief pakkette (`.tar`) | 30.0 MB | 56.35 Mbps | 83.17 Mbps | 1.5x |
| Videolêer (`.mp4`) | 30.0 MB | 27.00 Mbps | 39.46 Mbps | 1.5x |

### Werklike meting van transkontinentale hoëspoed toegewyde lyn (Europa → Noord-Amerika, vertraging 86 ms)

| Transmissie scenario | Data grootte | `sync0` | `rsync` | Verhoog vermenigvuldiger |
| :--- | :---: | :---: | :---: | :---: |
| 10 000 klein lêers | 77.6 MB | 688 Mbps | 128 Mbps | 5.4x |
| Kode en argief pakkette (`.tar`) | 512 MB | 2111 Mbps | 438 Mbps | 4.8x |
| Videolêer (`.mp4`) | 512 MB | 556 Mbps | 338 Mbps | 1.6x |
| Inkrementele sinchronisasie (verander 1% inhoud) | 512 MB | 34133 Mbps | 2214 Mbps | 15.4x |

## Installeer

### Metode 1: Installeer via Cargo

```bash
cargo install sync0
```

### Metode 2: Laai vooraf saamgestelde binaries af

Gaan na [GitHub Releases](https://github.com/webc-site/sync0/releases) om die `.tar.xz` saamgeperste pakket vir die ooreenstemmende stelsel af te laai (ondersteun Linux, macOS, Windows, Android, FreeBSD, NetBSD), pak dit uit en plaas dit in die `PATH` gids:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Begin vinnig

`sync0` gebruik by verstek `~/.config/sync0` as die konfigurasiegids (as jy na ander gidse moet oorskakel, kan jy `-c <gids>` byvoeg).

### 1. Bekyk die plaaslike nodusadres en instellingbesigheidskaartjie

```bash
# Bekyk die plaaslike nodusadres (die sleutel word vir die eerste keer outomaties in ~/.config/sync0 gegenereer)
sync0 id

# Genereer 'n persoonlike nodusbesigheidskaartjie (ondersteun gebruikernaam, stad, e-pos, webwerf en handtekening binne 256 woorde)
sync0 profile -u alice --region SHA --email alice@example.com --url https://example.com -b "Shanghai node"
```

### 2. Publiseer veelvuldige lêerdeelgroepe en stel groeplidtoestemmings in (`.nt` Konfigurasie)

`sync0` is opgestel in die [NestedText](https://nestedtext.org) (`.nt`) formaat. Elke gebruiker het 'n aparte `.nt` konfigurasielêer. Jy kan die gebruikersprofielgids spesifiseer deur `users_dir` in `config.nt` (wat outomaties alle `.nt` lêers in hierdie gids insluit). Beide die gebruiker en die pad-voorvoegsel wat toegelaat word om opgelaai te word, gebruik 'n ongeordende lys (`-`). **Wanneer die oplaaipad-voorvoegsel nie ingevul is nie, word oplaai na die wortelgids van die deel by verstek toegelaat**.

1. Skep aparte `.nt` konfigurasielêers vir elke lid in die `~/.config/sync0/users/` gids:

`~/.config/sync0/users/alice.nt` (leesalleen-lid, tarief beperk `80MB`, `rate` ondersteun `GB`, `MB`, `KB` eenhede):

```yaml
id: 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c
perm: ro
rate: 80MB
```

`~/.config/sync0/users/bob.nt` (beperkte oplaailede: slegs toegelaat om op te laai na `uploads/bob` en `shared/common`-gidse):

```yaml
id: 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f
perm: rw
write_prefix:
  - uploads/bob
  - shared/common
```

`~/.config/sync0/users/charlie.nt` (Verstek lidoplaai: Wanneer `write_prefix` weggelaat word, word oplaai na die wortelgids van die deel by verstek toegelaat):

```yaml
id: 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
perm: rw
```

2. Stel die gebruikersgids en veelvuldige lêerdeelgroepe op in `~/.config/sync0/config.nt`:

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

Begin deeldiens:

```bash
# Direkte opstart (laai outomaties alle .nt-gebruikerkonfigurasies in ~/.config/sync0/config.nt en users_dir-gidse)
sync0 share --region SHA

# Of publiseer vinnig verskeie deelgroepe en magtig groeplede direk deur die opdragreël
sync0 share dataset:./data team-vault:./vault \
  --auth \
  --allow-ro 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c \
  --allow-rw 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f:uploads/bob,shared/common \
  --allow-rw 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
```

### 3. Trek alle inhoud of kies veelvuldige lêers en gidse (`sync0 pull`)

Koppel aan enige eweknie nodusse om outomaties alle beskikbare saad nodusse in die deelgroep te verkry, en trek gelyktydig van verskeie saad nodusse af:

```bash
# Trek die hele datasteldeelgroep na die plaaslike ./local_data
sync0 trek datastel:// ./local_data -p <node-adres>

# Multi-kies sinchronisasie: trek gelyktydig die enkele lêer docs/readme.md en subgidse bates/ikone, src/core
sync0 trek datastel://docs/readme.md,assets/icons,src/core ./local_data -p <node address>
```

### 4. Laai 'n lêer of gids op na 'n deelgroep (`sync0 push`)

Lede met oplaaitoestemmings (`rw` of `admin`) kan plaaslike lêers of gidse na afgeleë deelgroepe oplaai:

```bash
# Lede met wortelgids-oplaaitoestemming: Laai direk op na die gedeelde wortelgids
sync0 druk span-kluis:// ./new_release -p <node-adres>

# Slegs lede wat gemagtig is tot 'n spesifieke voorvoegsel (soos oplaaie/bob): laai op na die toegelate padvoorvoegsel
sync0 push team-vault://uploads/bob/report.pdf ./report.pdf -p <node address>
```

### 5. Teken in op outomatiese sinkronisering en saai (`sync0 sub`)

Nadat u op die deelgroep van 'n sekere nodus ingeteken het, is die plaaslike bediener beide 'n ontvangende kliënt en 'n nuwe saaibediener; wanneer die bronnoduslêer opgedateer word, word dit outomaties gesinchroniseer en verskaf stukkies aflaaie vir ander nodusse:

```bash
# Teken in op die hele deelgroep en maak outomaties saad
sync0 sub datastel:// ./local_mirror -p <node adres>

# Slegs multi-selekteer om in te teken op gespesifiseerde lêers en subgidse en dit te saai
sync0 subdatastel://models/llama.onnx,configs ./local_mirror -p <node-adres>
```

### 6. Multi-masjien groep eweknie-sinchronisasie (`sync0 group`)

Veelvuldige masjiene sluit by dieselfde deelgroep aan, mekaar ontdek outomaties beskikbare saadnodusse en handhaaf tweerigtingsinchronisasie (ondersteun volledige sinchronisasie of veelvuldige keuse van lêers en gidse):

```bash
# Sluit ten volle aan by die groep en tweerigtingsinchronisasie
sync0 groep span-docs:// ./docs -p <node-adres>

# Slegs die wal-gids en meta.json-lêer in die multi-select sinchronisasie groep
sync0 group team-docs://wal,meta.json ./docs -p <node address>
```

### 7. Punt-tot-punt direkte transmissie tussen twee masjiene (`send` / `recv`)

Dit is nie nodig om 'n groep vooraf te konfigureer nie, en jy kan lêers of gidse tydelik direk tussen twee masjiene oordra:

```bash
# Ontvanger monitering
sync0 recv 0.0.0.0:9000 ./backup

# Die stuur einde dra lêers of dopgehou direk oor (opsioneel --vee spieël uit om oortollige lêers skoon te maak, --snelheid spoedlimiet)
sync0 send ./project 192.168.1.10:9000
```

### 8. Multi-taal skakeling

`sync0` ondersteun 75 tale, en skakel outomaties volgens die stelseltaalomgewing by verstek; dit kan ook met die hand gespesifiseer word deur `SYNC0_LANG`:

```bash
SYNC0_LANG=en sync0 --help
SYNC0_LANG=ja sync0 --help
SYNC0_LANG=de sync0 --help
```