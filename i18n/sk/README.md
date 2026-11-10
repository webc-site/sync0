# sync0

`sync0` je decentralizovaný nástroj na zdieľanie P2P súborov, synchronizáciu v reálnom čase a viaczdrojovú distribúciu, ktorý nahrádza `rsync` - nameraná rýchlosť transkontinentálnej siete je až **6,95×** z `rsync` (jednobodová geometrická priemerná rýchlosť **2,01×**, priemerná rýchlosť P2P je** 3** malých geometrických súborov 3× priemerná rýchlosť oceánu 3** tak rýchlo ako **7,19×**.

Každý stroj je serverom aj klientom: nie je potrebný centrálny server ani verejná sieťová adresa, automatické dierovanie je založené na adrese distribuovaného sieťového verejného kľúča a podporuje priamy prenos medzi dvoma strojmi, hosťovanie povolení skupiny zdieľania, viacnásobný výber podadresárov na požiadanie, prírastkovú synchronizáciu na úrovni milisekúnd v reálnom čase a viaczdrojové súbežné zrýchlenie P2P.

## Inštalovať

### Metóda 1: Inštalácia nákladu

```bash
cargo install sync0
```

### Metóda 2: Stiahnite si predkompilované binárne súbory

Prejdite na [GitHub Releases](https://github.com/webc-site/sync0/releases) a stiahnite si `.tar.xz` komprimovaný balík zodpovedajúci systému (podporuje Linux, macOS, Windows, Android, iOS, FreeBSD), rozbaľte ho a vložte do `PATH`:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Začnite rýchlo

### 1. Priamy prenos s nulovou konfiguráciou medzi dvoma počítačmi (`send` / `recv`)

Nie je potrebné konfigurovať skupinu zdieľania, priamo prenášať súbory alebo adresáre medzi dvoma počítačmi vysokou rýchlosťou:

```bash
# Prijímajúci koniec počúva na porte a zapisuje do ./backup
sync0 recv 0.0.0.0:9000 ./backup

# Priamy prenos od odosielateľa (voliteľné --odstrániť zrkadlo na vyčistenie prebytočných súborov, --rýchlosť 50 MB)
sync0 send ./project 192.168.1.10:9000
```

### 2. Zdieľanie príspevkov a stiahnutie/odovzdanie na požiadanie (`share` / `pull` / `push`)

```bash
# Server: Zverejnite skupinu zdieľania jedným príkazovým riadkom (po spustení uveďte adresu uzla iroh bez IP adresy)
sync0 share dataset:./data

# Klient: Presuňte celú skupinu zdieľania množiny údajov do lokálneho ./local_data
sync0 pull dataset:// ./local_data -p <adresa uzla>

# Klient: Vytiahnite viacero výberov na požiadanie (synchronizujte iba zadané jednotlivé súbory a podadresáre)
sync0 pull dataset://docs/readme.md,assets/icons,src/core ./local_data -p <adresa uzla>

# Klient: Nahrajte súbory alebo adresáre do autorizovaných skupín zdieľania
sync0 push dataset://releases/v1 ./build_output -p <adresa uzla>
```

### 3. Predplatné v reálnom čase a obojsmerná synchronizácia na viacerých strojoch (`sub` / `group`)

Ak po nadviazaní dlhého pripojenia dôjde na vzdialenom konci k nejakému súboru **pridanie, úprava, vymazanie, zmena oprávnení alebo čas úpravy**, lokálny adresár sa synchrónne aktualizuje v milisekúndách v reálnom čase (nové/upravené veľké súbory iba vytiahnu rozdielové bloky a aktualizujú ich na miesto, mazanie súborov automaticky vyčistí miestne súbory a prázdne adresáre a iba priamo zmení povolenia alebo označí bloky s nulovými údajmi):

- **`sync0 sub`(Jednosmerné zrkadlenie v reálnom čase + automatický seeding)**: Lokálny real-time sleduje pridanie, vymazanie a úpravy konca zdieľania a automaticky slúži ako zdroj P2P seed na poskytovanie súbežnej akcelerácie pre ostatné ťahové uzly.
- **`sync0 group`(Obojsmerná synchronizácia typu peer-to-peer v reálnom čase na viacerých počítačoch)**: Ak sa na ľubovoľnom počítači v skupine pridajú, odstránia alebo upravia súbory, všetky ostatné online uzly sa budú aktualizovať synchrónne v reálnom čase a budú si navzájom slúžiť ako základ.

```bash
# Prihláste sa na odber celej skupiny zdieľania: vzdialené pridania, vymazania a úpravy sa synchronizujú lokálne v reálnom čase a automaticky sa nasadia
sync0 sub dataset:// ./local_mirror -p <adresa uzla>

# Predplatné s viacerými možnosťami výberu na požiadanie: synchronizujte a nasadzujte iba konkrétne jednotlivé súbory a podadresáre v reálnom čase
sync0 sub dataset://models/llama.onnx,configs ./local_mirror -p <adresa uzla>

# Obojsmerná synchronizácia v reálnom čase pre skupinu rovesníkov s viacerými počítačmi (podporuje aj úplné alebo viacnásobné výberové cesty čiarkami)
skupina sync0 team-docs:// ./docs -p <adresa uzla>
```

### 4. Konfigurácia povolení skupiny na zdieľanie viacerých skupín a členov (`.nt` konfigurácia)

Predvolený konfiguračný adresár je `~/.config/sync0` (dá sa prepínať cez `-c <adresár>`) s použitím minimalistického formátu [NestedText](https://nestedtext.org) (`.nt`):
- **Podporuje obmedzenie čítania vo verejnej sieti (čítať môžu iba autorizovaní používatelia)**: Pokiaľ je skupina zdieľania nakonfigurovaná so zoznamom členov `users:` (alebo príkazový riadok špecifikuje `--allow-r` / `--allow-rw` / `--allow-w` / `--auth`), systém **automaticky zapne verejnú autentifikáciu (verejnú autentizáciu = 6"> enumer no anonymné čítanie)** `users`, sú predvolene verejné a iba na čítanie (ak chcete byť verejné a iba na čítanie a chcete ich odovzdávať iba určeným členom, môžete explicitne nastaviť `auth: false`).
- **Povolenia pre jemné čítanie a zápis a doručovanie podadresárov**: Členovia sú predvolene iba na čítanie (`r` nemusí byť explicitne zapísané) a podporujú `r` (iba na čítanie), `w` (iba zapisovať do schránky, nie je čitateľné), `rw` (priame čítanie a zápis je povolené pre nahrávanie) a <b)

**Príklad konfigurácie člena (`~/.config/sync0/users/`):**

```yaml
# ~/.config/sync0/users/alice.nt (predvolené len na čítanie, rýchlostný limit 80 MB)
id: 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c
rate: 80MB
```

```yaml
# ~/.config/sync0/users/bob.nt (oprávnenia na čítanie a zápis, nahrávanie je povolené len do určeného podadresára)
id: 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f
perm: rw
write_prefix:
  - uploads/bob
  - shared/common
```

```yaml
# ~/.config/sync0/users/charlie.nt (len povolenie na doručovanie zápisu, čítanie je zakázané)
id: 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
perm: w
```

**Príklad konfigurácie skupiny zdieľania (`~/.config/sync0/config.nt`):**

```yaml
users_dir: ./users

groups:
  # Nekonfigurovaní používatelia: predvolená verejná sieť len na čítanie
  dataset:
    path: ./data
    rate: 50MB

  # Konfigurovaní používatelia: automaticky obmedziť čítanie vo verejnej sieti (čítať môžu iba Alice a Bob, charlie môže iba doručovať a nahrávať)
  team-vault:
    path: ./vault
    users:
      - alice
      - bob
      - charlie
```

Spustite službu (načítajte konfiguračný súbor alebo priamo použite parametre príkazového riadka na obmedzenie čítania verejnej siete a autorizáciu členov):

```bash
# Začnite načítaním ~/.config/sync0/config.nt
sync0 share --region SHA

# Alebo publikujte súkromnú skupinu zdieľania prostredníctvom čistého príkazového riadka (zadajte --allow-*, aby ste automaticky obmedzili čítanie vo verejnej sieti a mali k nej prístup iba oprávnení používatelia)
sync0 share team-vault:./vault \
  --allow-r 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c \
  --allow-rw 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f:uploads/bob,shared/common \
  --allow-w 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
```

### 5. Adresa uzla, vizitka a viacjazyčnosť

```bash
# Zobrazenie lokálnej adresy uzla bez IP (kľúč sa automaticky vygeneruje pri prvom spustení)
sync0 id

# Vygenerujte vizitku uzla s vlastným podpisom Ed25519 (prezývka, kód mesta, e-mail, webová stránka, úvod)
sync0 profile -u alice --region SHA --email alice@example.com -b "Shanghai node"

# K dispozícii je 75 vstavaných jazykov, ktoré sa štandardne riadia jazykom systému a možno ich prepínať prostredníctvom premenných prostredia.
SYNC0_LANG=en sync0 --help
```

## Výkon a základné funkcie

### Skutočné meranie medzi kontinentálnymi sieťami WAN (Porovnanie `rsync`)

Aktuálne prostredie merania: Spojené štáty `c1` → Európa `n1` (RTT 65 ms), množina údajov je jadro Linuxu `linux-7.3.3`:

#### 1) Režim A la carte vs `rsync`

| Súbor údajov | veľkosť | `sync0` Jeden bod<br>(Mb/s) | `rsync`<br>(Mbps) | Zrýchlenie (`sync0` / `rsync`) |
| :--- | ---: | ---: | ---: | ---: |
| `linux-7.3.3.tar.xz`(nestlačiteľné) | 155 MB | **121.07** | 107.89 | **1,12×** |
| `linux-7.3.3.tar`(stlačiteľné) | 1.6 GB | **442.07** | 506.62 | **0,87×** |
| `linux-7.3.3.tar`(prírastok miestnej zmeny) | 1.6 GB | **8,123.87** | 1,169.49 | **6,95×** |
| `linux-7.3.3/` (95 979 malých súborov) | 1.5 GB | **565.49** | 235.27 | **2,40×** |
| **geometrický priemer** | — | **704.17** | 350.19 | **2,01×** |

#### 2) P2P režim s dvoma zdrojmi vs `rsync`

| Súbor údajov | veľkosť | `sync0` Duálny zdroj P2P<br>(Mb/s) | `rsync`<br>(Mbps) | Zrýchlenie (`sync0` / `rsync`) |
| :--- | ---: | ---: | ---: | ---: |
| `linux-7.3.3.tar.xz`(nestlačiteľné) | 155 MB | **466.85** | 107.89 | **4,33×** |
| `linux-7.3.3.tar`(stlačiteľné) | 1.6 GB | **992.17** | 506.62 | **1,96×** |
| `linux-7.3.3.tar`(prírastok miestnej zmeny) | 1.6 GB | **5,666.84** | 1,169.49 | **4,85×** |
| `linux-7.3.3/` (95 979 malých súborov) | 1.5 GB | **703.30** | 235.27 | **2,99×** |
| **geometrický priemer** | — | **1,165.63** | 350.19 | **3,33×** |

> Poznámka: V režime s dvoma zdrojmi P2P európske uzly `s1` spolupracujú pri nasadzovaní a medzikontinentálna chrbticová RTT je 65 ms.

### Hlavné vlastnosti produktu

- **Automatické dierovanie s adresovaním verejného kľúča**: Nie je potrebný centrálny server a verejná sieťová adresa, automatické dierovanie NAT na základe adresy verejného kľúča distribuovanej siete.
- **Milisekundová synchronizácia v reálnom čase a informovanosť o metadátach**: Založené na natívnom monitorovaní udalostí súboru (`FSEvents` / `inotify` / `ReadDirectoryChangesW`), pridávaní, odstraňovaní súborov v reálnom čase, povoleniach POSIX (`mode`) a čase úprav na úrovni milisekúnd (`mtime`); prenos nulového bloku dát, keď sa upravia iba povolenia alebo čas.
- **Súčasné sťahovanie a nahrávanie v štýle BT a plánovanie regionálnej afinity**: Sťahovací uzol nemusí čakať na stiahnutie celej sumy. Všetky blokové intervaly (`Op::Have`), ktoré prešli 128-bitovou kontrolou hash, sú okamžite uzamknuté v pamäti/dočasnom fonde, aby ich ostatné uzly mohli stiahnuť súčasne. Spolu s dynamickou výmenou uzlov PEX a pomalou preempciou polovice uzla (Work-Stealing) sa maximalizuje priepustnosť klastra.
- **Ochrana proti neoprávnenému zásahu do uzla a otrave klastra**: autoritatívny zoznam zdrojov `root_hash` Samostatné uzamykanie + povinná hardvérová akcelerácia `gxhash128` overenie súhrnu pred umiestnením každého bloku na disk a externým nasadením; ak sa nájde narušený blok, istič s milisekundovou úrovňou vypne škodlivý uzol a automaticky sa obnoví z čestného zdroja.
- **FastCDC in-situ inkrement a agregácia malých súborov**: Pre čiastočnú úpravu veľkých súborov sa prenesú iba rozdielové bloky a uložia sa na disk; masívne malé balenie pamäte súborov a predponová diferenciálna kompresia eliminujú réžiu po jednotlivých súboroch.
- **Samoliečenie po výpadku napájania a obnovenie po reštarte**: Hybridný formát RDB + AOF podobný Redis (`.s0_subs.bin`) sa používa na zachovanie stavu nasadenia a zoznamu súborov, denné zmeny sa pridávajú v mikrosekundách a pravidelne sa automaticky prepisujú a organizujú. Po reštartovaní servera sa offline zmeny automaticky prenesú do seedovacieho uzla.

## Technická architektúra a dokumentácia

- **Dvojrovinový oddeľovací protokol**:
  - **Kontrola a povrch zmien v reálnom čase (`iroh` QUIC / TCP, prvý bajt `PROTO_VERSION` handshake verzie) **: Zodpovedá za overenie vizitky Ed25519, overenie povolení, zisťovanie semien Swarm a podporu pre streamovanie hromadných zmien na úrovni `mzstd` a `mzstd` 5 komprimované <b> v reálnom čase `FsOp::Remove` / `FsOp::Meta` Tri primitívy).
  - **Plocha distribúcie údajov (natívne UDP + `compio` asynchrónne I/O) **: 20B kompaktná hlavička, 18B TACK periodická spätná väzba, hybridné riadenie preťaženia BBRv3/Copa, obnova straty paketov RACK-TLP, premostenie kompresie Zstd adaptívnej na entropiu Shannon a blokovanie <b-id="2"> end-to-end.
- **Viac informácií**:
  - [Správa o meraní výkonu a reprodukčný scenár](https://github.com/webc-site/sync0/blob/main/doc/zh/benchmark.md)
  - [Návrh architektúry systému](https://github.com/webc-site/sync0/blob/main/doc/zh/design_v4.md)
  - [Binárny drôtový protokol a špecifikácia perzistencie](https://github.com/webc-site/sync0/blob/main/doc/zh/proto.md)
  - [Autentifikácia identity, kontrola autority a overenie integrity](https://github.com/webc-site/sync0/blob/main/doc/zh/crypto.md)