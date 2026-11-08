# sync0

`sync0` är ett peer-to-peer (P2P) fildelnings- och synkroniseringsverktyg för flera maskiner som konkurrerar med Resilio Sync.

Varje maskin är både en server och en klient: vilken nod som helst kan publicera flera fildelningsgrupper. Efter att andra noder anslutit sig kan de automatiskt erhålla tillgängliga frönoder från peer-servern och hämta dem samtidigt från flera källor. När synkroniseringen är klar skapas frön automatiskt för fler noder för att påskynda nedladdningar.

## Kärnhöjdpunkter

- **Alla är både en server och en klient**: Jämförbar med Resilio Sync, inget centralt serverberoende. Vilken nod som helst kan skapa flera fildelningsgrupper och kan även gå med i andras delningsgrupper; noder utbyter automatiskt och erhåller tillgängliga frönoder för att uppnå samtidig dragning och realtidssynkronisering av flera maskiner.
- **IP-fri nodadress**: Som standard genereras nodnyckeln och `base64url` kodad `iroh` adress automatiskt i `~/.config/sync0` utan att behöva komma ihåg eller exponera den offentliga IP-adressen.
- **Multi-select synkronisering på begäran**: Stöder synkronisering av hela delningsgruppen, och stöder även synkronisering på begäran genom att välja flera filer eller underkataloger samtidigt.
- **Hierarkiska behörigheter för gruppmedlemmar och kontroll för uppladdningskatalog**: Stöder skrivskyddad (`ro`), läs-skrivuppladdning (`rw`) och hanteringsbehörigheter (`admin`), och kan individuellt ange sökvägsprefixet som varje medlem får ladda upp (som standard är uppladdning till andelens rotkatalog tillåten).
- **Fullhastighets- och inkrementell synkronisering över nätverk**: Samtidig överföring av stora filer, aggregering av stora små filer i minnet och endast synkroniserade datablock när filer är delvis modifierade.

## Hastighetsjämförelse med rsync

### Transoceanic nätverk en källa faktisk mätning (Kina → USA, fördröjning 260 ms)

| Överföringsscenario | Datastorlek | modell | `sync0` | `rsync -az` | Öka multiplikatorn |
| :--- | :---: | :---: | :---: | :---: | :---: |
| 500 små filer | 1.91 MB | Fullständig synkronisering | 18.84 Mbps | 5.32 Mbps | 3.5x |
| 500 små filer | 1.91 MB | Inkrementell synkronisering | 27.15 Mbps | 3.83 Mbps | 7.1x |
| Kod- och arkivpaket (`.tar`) | 10.0 MB | Fullständig synkronisering | 139.70 Mbps | 24.97 Mbps | 5.6x |
| Kod- och arkivpaket (`.tar`) | 10.0 MB | Inkrementell synkronisering | 83.38 Mbps | 26.02 Mbps | 3.2x |
| Videofil (`.mp4`) | 15.0 MB | Fullständig synkronisering | 54.06 Mbps | 27.04 Mbps | 2.0x |
| Videofil (`.mp4`) | 15.0 MB | Inkrementell synkronisering | 216.83 Mbps | 21.51 Mbps | 10.1x |

### Transoceaniskt nätverk P2P multi-nod pull test (Kina → USA dubbla noder, fördröjning 260 ms)

Uppmätt jämförelse av multi-source seed nod randigt samtidig drag:

| Överföringsscenario | Datastorlek | Enkel nodsådd | Två-nod P2P samtidig sådd | Öka multiplikatorn |
| :--- | :---: | :---: | :---: | :---: |
| 500 små filer | 1.97 MB | 5.28 Mbps | 7.87 Mbps | 1.5x |
| Kod- och arkivpaket (`.tar`) | 30.0 MB | 56.35 Mbps | 83.17 Mbps | 1.5x |
| Videofil (`.mp4`) | 30.0 MB | 27.00 Mbps | 39.46 Mbps | 1.5x |

### Faktisk mätning av transkontinental dedikerad höghastighetslinje (Europa → Nordamerika, fördröjning 86 ms)

| Överföringsscenario | Datastorlek | `sync0` | `rsync` | Öka multiplikatorn |
| :--- | :---: | :---: | :---: | :---: |
| 10 000 små filer | 77.6 MB | 688 Mbps | 128 Mbps | 5.4x |
| Kod- och arkivpaket (`.tar`) | 512 MB | 2111 Mbps | 438 Mbps | 4.8x |
| Videofil (`.mp4`) | 512 MB | 556 Mbps | 338 Mbps | 1.6x |
| Inkrementell synkronisering (ändra 1 % innehåll) | 512 MB | 34133 Mbps | 2214 Mbps | 15.4x |

## Installera

### Metod 1: Installera via Cargo

```bash
cargo install sync0
```

### Metod 2: Ladda ner förkompilerade binärer

Gå till [GitHub Releases](https://github.com/webc-site/sync0/releases) för att ladda ner det `.tar.xz` komprimerade paketet för motsvarande system (stöder Linux, macOS, Windows, Android, FreeBSD, NetBSD), packa upp och lägg det i `PATH`-katalogen:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Kom igång snabbt

`sync0` använder `~/.config/sync0` som konfigurationskatalog som standard (om du behöver byta till andra kataloger kan du lägga till `-c <katalog>`).

### 1. Visa den lokala nodens adress och inställningsvisitkortet

```bash
# Visa den lokala nodadressen (nyckeln genereras automatiskt i ~/.config/sync0 för första gången)
sync0 id

# Skapa ett personligt nodvisitkort (stöder användarnamn, stad, e-post, webbplats och signatur inom 256 ord)
sync0 profile -u alice --region SHA --email alice@example.com --url https://example.com -b "Shanghai node"
```

### 2. Publicera flera fildelningsgrupper och ange behörigheter för gruppmedlemmar (`.nt` Konfiguration)

`sync0` är konfigurerad i formatet [NestedText](https://nestedtext.org) (`.nt`). Varje användare har en separat `.nt` konfigurationsfil. Du kan ange användarprofilkatalogen genom `users_dir` i `config.nt` (som automatiskt inkluderar alla `.nt` filer i denna katalog). Både användaren och sökvägsprefixet som får laddas upp använder en oordnad lista (`-`). **När uppladdningssökvägsprefixet inte är ifyllt är uppladdning till andelens rotkatalog tillåten som standard**.

1. Skapa separata `.nt` konfigurationsfiler för varje medlem i katalogen `~/.config/sync0/users/`:

`~/.config/sync0/users/alice.nt` (skrivskyddad medlem, prisbegränsad `80MB`, `rate` stöder `GB`, `MB`, `KB` enheter):

```yaml
id: 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c
perm: ro
rate: 80MB
```

`~/.config/sync0/users/bob.nt` (begränsade uppladdningsmedlemmar: endast tillåtet att ladda upp till `uploads/bob` och `shared/common` kataloger):

```yaml
id: 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f
perm: rw
write_prefix:
  - uploads/bob
  - shared/common
```

`~/.config/sync0/users/charlie.nt` (Standardmedlemsuppladdning: När `write_prefix` utelämnas är uppladdning till delingens rotkatalog tillåten som standard):

```yaml
id: 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
perm: rw
```

2. Konfigurera användarkatalogen och flera fildelningsgrupper i `~/.config/sync0/config.nt`:

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

Börja dela tjänst:

```bash
# Direktstart (läser automatiskt in alla .nt-användarkonfigurationer i katalogerna ~/.config/sync0/config.nt och users_dir)
sync0 share --region SHA

# Eller publicera snabbt flera delningsgrupper och auktorisera gruppmedlemmar direkt via kommandoraden
sync0 share dataset:./data team-vault:./vault \
  --auth \
  --allow-ro 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c \
  --allow-rw 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f:uploads/bob,shared/common \
  --allow-rw 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
```

### 3. Dra allt innehåll eller välj flera filer och kataloger (`sync0 pull`)

Anslut till valfri peer-nod för att automatiskt få alla tillgängliga frönoder i delningsgruppen, och dra från flera frönoder samtidigt:

```bash
# Dra hela datasetdelningsgruppen till den lokala ./local_data
sync0 pull dataset:// ./local_data -p <nodadress>

# Flervalssynkronisering: dra samtidigt enstaka filen docs/readme.md och underkataloger assets/ikoner, src/core
sync0 pull dataset://docs/readme.md,assets/icons,src/core ./local_data -p <nodadress>
```

### 4. Ladda upp en fil eller katalog till en delningsgrupp (`sync0 push`)

Medlemmar med uppladdningsbehörigheter (`rw` eller `admin`) kan ladda upp lokala filer eller kataloger till fjärrdelningsgrupper:

```bash
# Medlemmar med rotkataloguppladdningsbehörighet: Ladda upp direkt till den delade rotkatalogen
sync0 push team-vault:// ./new_release -p <nodadress>

# Endast medlemmar som är auktoriserade till ett specifikt prefix (som uppladdningar/bob): ladda upp till det tillåtna sökvägsprefixet
sync0 push team-vault://uploads/bob/report.pdf ./report.pdf -p <nodadress>
```

### 5. Prenumerera på automatisk synkronisering och seedning (`sync0 sub`)

Efter att ha prenumererat på delningsgruppen för en viss nod är den lokala servern både en mottagande klient och en ny såddserver; när källnodsfilen uppdateras synkroniseras den automatiskt och tillhandahåller nedladdningar i bitar för andra noder:

```bash
# Prenumerera på hela delningsgruppen och seed automatiskt
sync0 underdataset:// ./local_mirror -p <nodadress>

# Välj bara flera för att prenumerera på specificerade filer och underkataloger och se dem
sync0 subdataset://models/llama.onnx,configs ./local_mirror -p <nodadress>
```

### 6. Gruppsynkronisering för flera datorer (`sync0 group`)

Flera maskiner går med i samma delningsgrupp, varandra upptäcker automatiskt tillgängliga frönoder och upprätthåller tvåvägssynkronisering (stöder full synkronisering eller flera urval av filer och kataloger):

```bash
# Gå med helt i gruppen och tvåvägssynkronisering
sync0 group team-docs:// ./docs -p <nodadress>

# Endast wal-katalogen och meta.json-filen i multi-select synkroniseringsgruppen
sync0 group team-docs://wal,meta.json ./docs -p <nodadress>
```

### 7. Direktöverföring från punkt till punkt mellan två maskiner (`send` / `recv`)

Det finns inget behov av att förkonfigurera en grupp, och du kan tillfälligt överföra filer eller kataloger direkt mellan två maskiner:

```bash
# Mottagare övervakning
sync0 recv 0.0.0.0:9000 ./backup

# Sändningsänden överför filer eller kataloger direkt (valfritt --radera spegel för att rensa upp överflödiga filer, --hastighetsgräns)
sync0 send ./project 192.168.1.10:9000
```

### 8. Flerspråksväxling

`sync0` stöder 75 språk och växlar automatiskt efter systemspråkmiljön som standard; det kan också anges manuellt genom `SYNC0_LANG`:

```bash
SYNC0_LANG=en sync0 --help
SYNC0_LANG=ja sync0 --help
SYNC0_LANG=de sync0 --help
```