# sync0

`sync0` je decentralizovaný P2P nástroj pro sdílení souborů, synchronizaci v reálném čase a vícezdrojový distribuční nástroj používaný k nahrazení `rsync` - naměřená rychlost transkontinentální WAN je až `rsync` **5,89×** (jednobodová geometrická průměrná rychlost **2,67×**), P2P dual-source geometrická průměrná rychlost** a P2.nic-6 průměrná rychlost**3 zrychlení je až **7,68×** (Full-link 24-scénář komplexní geometrická střední rychlost **2,56×**).

Každý počítač je serverem i klientem: není potřeba centrální server ani veřejná síťová adresa, automatické děrování je založeno na adrese veřejného klíče distribuované sítě a podporuje přímý přenos mezi dvěma stroji, hostování oprávnění skupiny pro více sdílení, vícenásobný výběr podadresářů na vyžádání, přírůstkovou synchronizaci na úrovni milisekund v reálném čase a vícezdrojovou souběžnou akceleraci P2P.

## Instalovat

### Metoda 1: Instalace nákladu

```bash
cargo install sync0
```

### Metoda 2: Stáhněte si předkompilované binární soubory

Přejděte na [GitHub Releases](https://github.com/webc-site/sync0/releases) a stáhněte si `.tar.xz` komprimovaný balíček odpovídající systému (podporuje Linux, macOS, Windows, Android, iOS, FreeBSD), rozbalte jej a vložte do `PATH`:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Začněte rychle

### 1. Přímý přenos s nulovou konfigurací mezi dvěma počítači (`send` / `recv`)

Není třeba konfigurovat skupinu sdílení, přímo přenášet soubory nebo adresáře mezi dvěma počítači vysokou rychlostí:

```bash
# Přijímající konec naslouchá na portu a zapisuje do ./backup
sync0 recv 0.0.0.0:9000 ./backup

# Přímý přenos od odesílatele (volitelně --smazat zrcadlo pro vyčištění přebytečných souborů, --rychlostní limit 50 MB)
sync0 send ./project 192.168.1.10:9000
```

### 2. Sdílení příspěvků a stahování/nahrání na vyžádání (`share` / `pull` / `push`)

```bash
# Server: Zveřejněte skupinu sdílení jedním příkazovým řádkem (po spuštění uveďte adresu uzlu iroh bez IP adresy)
sync0 share dataset:./data

# Klient: Stáhněte celou skupinu sdílení datové sady do místního ./local_data
sync0 pull dataset:// ./local_data -p <adresa uzlu>

# Klient: Vytáhněte více výběrů na vyžádání (synchronizujte pouze zadané jednotlivé soubory a podadresáře)
sync0 pull dataset://docs/readme.md,assets/icons,src/core ./local_data -p <adresa uzlu>

# Klient: Nahrajte soubory nebo adresáře do autorizovaných skupin sdílení
sync0 push datová sada://releases/v1 ./build_output -p <adresa uzlu>
```

### 3. Seedování předplatného v reálném čase a obousměrná synchronizace mezi více počítači (`sub` / `group`)

Pokud po navázání dlouhého připojení dojde na vzdáleném konci k nějakému souboru **přidání, úprava, odstranění, změna oprávnění nebo čas úpravy**, bude místní adresář synchronně aktualizován v milisekundách v reálném čase (nové/upravené velké soubory pouze vytáhnou rozdílové bloky a aktualizují je na místě, smazání souborů automaticky vyčistí místní soubory a prázdné adresáře a pouze změní oprávnění nebo označí bloky s nulovými daty):

- **`sync0 sub`(Jednosměrné zrcadlení v reálném čase + automatické seedování)**: Lokální reálný čas následuje přidání, odstranění a úpravy konce sdílení a automaticky slouží jako zdroj P2P seed, který poskytuje souběžnou akceleraci pro ostatní tažné uzly.
- **`sync0 group`(obousměrná synchronizace mezi počítači peer-to-peer v reálném čase)**: Pokud jsou soubory přidány, odstraněny nebo upraveny na jakémkoli počítači ve skupině, všechny ostatní online uzly budou aktualizovány synchronně v reálném čase a budou si navzájem sloužit jako základ.

```bash
# Přihlaste se k odběru celé skupiny sdílení: vzdálené přidávání, mazání a úpravy se synchronizují lokálně v reálném čase a automaticky se nasazují
sync0 sub dataset:// ./local_mirror -p <adresa uzlu>

# Vícenásobné předplatné na vyžádání: synchronizujte a osídlujte pouze zadané jednotlivé soubory a podadresáře v reálném čase
sync0 sub dataset://models/llama.onnx,configs ./local_mirror -p <adresa uzlu>

# Obousměrná synchronizace v reálném čase u skupin s více počítači (podporuje také plné cesty nebo cesty s vícenásobným výběrem čárkami)
skupina sync0 team-docs:// ./docs -p <adresa uzlu>
```

### 4. Konfigurace skupinového multisdílení a konfigurace oprávnění členů (`.nt` konfigurace)

Výchozí konfigurační adresář je `~/.config/sync0` (lze přepínat pomocí `-c <adresář>`) v minimalistickém formátu [NestedText](https://nestedtext.org) (`.nt`):
- **Podporuje omezení čtení ve veřejné síti (číst mohou pouze oprávnění uživatelé)**: Dokud je skupina sdílení nakonfigurována se seznamem členů `users:` (nebo příkazový řádek specifikuje `--allow-r` / `--allow-rw` / `--allow-w` / `--auth`), systém **automaticky zapne soukromou autentizaci (veřejná autentizace = 6">)**, anonymní síť, anonymní čtení, ne a P2P přihlášení k seedovacím uzlům také zdědí seznam povolených současně, což umožní číst pouze oprávněným uživatelům; skupiny sdílení, které nejsou nakonfigurovány pomocí `users`, jsou ve výchozím nastavení veřejné a pouze pro čtení (chcete-li být veřejné a pouze pro čtení a otevřené pro nahrávání pouze určeným členům, můžete explicitně nastavit `auth: false`).
- **Jemně upravená oprávnění pro čtení, zápis a doručování podadresářů**: Členové jsou ve výchozím nastavení pouze pro čtení (`r` nemusí být explicitně zapsáni) a podporují `r` (pouze pro čtení), `w` (pouze zápis do schránky, nelze číst), `rw` (povoleno podřízené id="5") a <b> (předběžné omezení pro id="5") a <b).

**Příklad konfigurace člena (`~/.config/sync0/users/`):**

```yaml
# ~/.config/sync0/users/alice.nt (výchozí pouze pro čtení, rychlostní limit 80 MB)
id: 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c
rate: 80MB
```

```yaml
# ~/.config/sync0/users/bob.nt (oprávnění ke čtení a zápisu, povoleno pouze nahrávání do určeného podadresáře)
id: 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f
perm: rw
write_prefix:
  - uploads/bob
  - shared/common
```

```yaml
# ~/.config/sync0/users/charlie.nt (pouze oprávnění k doručování zápisu, čtení je zakázáno)
id: 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
perm: w
```

**Příklad konfigurace skupiny sdílení (`~/.config/sync0/config.nt`):**

```yaml
users_dir: ./users

groups:
  # Nekonfigurovaní uživatelé: výchozí veřejná síť pouze pro čtení
  dataset:
    path: ./data
    rate: 50MB

  # Konfigurovaní uživatelé: automaticky omezit čtení ve veřejné síti (číst mohou pouze Alice a Bob, charlie může pouze doručovat a nahrávat)
  team-vault:
    path: ./vault
    users:
      - alice
      - bob
      - charlie
```

Spusťte službu (načtěte konfigurační soubor nebo přímo použijte parametry příkazového řádku k omezení čtení ve veřejné síti a autorizaci členů):

```bash
# Pro spuštění načtěte ~/.config/sync0/config.nt
sync0 share --region SHA

# Nebo publikujte soukromou skupinu sdílení pomocí čistě příkazového řádku (zadejte --allow-*, aby se automaticky omezilo čtení ve veřejné síti a přístup k ní měli pouze oprávnění uživatelé)
sync0 share team-vault:./vault \
  --allow-r 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c \
  --allow-rw 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f:uploads/bob,shared/common \
  --allow-w 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
```

### 5. Adresa uzlu, vizitka a vícejazyčnost

```bash
# Zobrazení adresy místního uzlu bez IP (klíč se vygeneruje automaticky pro první spuštění)
sync0 id

# Vygenerujte vizitku uzlu s vlastním podpisem Ed25519 (přezdívka, kód města, e-mail, webové stránky, úvod)
sync0 profile -u alice --region SHA --email alice@example.com -b "Shanghai node"

# K dispozici je 75 vestavěných jazyků, které se ve výchozím nastavení řídí jazykem systému a lze je přepínat pomocí proměnných prostředí.
SYNC0_LANG=en sync0 --help
```

## Výkon a základní vlastnosti

### Skutečné měření mezi kontinentální sítí WAN (Porovnání `rsync`)

Aktuální prostředí měření: Spojené státy americké `c1` → Evropa `n1` (RTT 65 ms), datová sada je linuxové jádro `linux-7.3.3`:

#### 1) Režim A la carte vs `rsync`

| Soubor dat | velikost | `sync0` Jeden bod<br>(Mb/s) | `rsync`<br>(Mbps) | Zrychlení (`sync0` / `rsync`) |
| :--- | ---: | ---: | ---: | ---: |
| `linux-7.3.3.tar.xz`(není komprimovatelné) | 155 MB | **236.48** | 108.74 | **2,17×** |
| `linux-7.3.3.tar`(stlačitelné) | 1.6 GB | **843.83** | 385.06 | **2,19×** |
| `linux-7.3.3.tar`(přírůstek místní změny) | 1.6 GB | **8,014.15** | 1,361.72 | **5,89×** |
| `linux-7.3.3/` (95 979 malých souborů) | 1.5 GB | **627.58** | 347.80 | **1,80×** |
| **geometrický průměr** | — | **1,000.91** | 375.26 | **2,67×** |

#### 2) P2P dvouzdrojový režim vs `rsync`

| Soubor dat | velikost | `sync0` Duální zdroj P2P<br>(Mb/s) | `rsync`<br>(Mbps) | Zrychlení (`sync0` / `rsync`) |
| :--- | ---: | ---: | ---: | ---: |
| `linux-7.3.3.tar.xz`(není komprimovatelné) | 155 MB | **330.09** | 108.74 | **3,04×** |
| `linux-7.3.3.tar`(stlačitelné) | 1.6 GB | **1,551.86** | 385.06 | **4,03×** |
| `linux-7.3.3.tar`(přírůstek místní změny) | 1.6 GB | **5,474.17** | 1,361.72 | **4,02×** |
| `linux-7.3.3/` (95 979 malých souborů) | 1.5 GB | **706.56** | 347.80 | **2,03×** |
| **geometrický průměr** | — | **1,186.42** | 375.26 | **3,16×** |

> Poznámka: V režimu P2P dual-source spolupracují evropské uzly `s1` na seedování a mezikontinentální páteřní RTT je 65 ms.

### Základní vlastnosti produktu

- **Automatické děrování s adresováním veřejného klíče**: Není potřeba centrální server a adresa veřejné sítě, automatické děrování NAT na základě adresy veřejného klíče distribuované sítě.
- **Synchronizace na úrovni milisekund v reálném čase a povědomí o metadatech**: Založeno na nativním monitorování událostí souborů (`FSEvents` / `inotify` / `ReadDirectoryChangesW`), přidávání, mazání souborů v reálném čase, oprávnění POSIX (`mode`) a době úprav na úrovni milisekund (`mtime`); přenos nulového bloku dat, když se změní pouze oprávnění nebo čas.
- **Současné stahování a nahrávání ve stylu BT a plánování regionální afinity**: Stahovací uzel nemusí čekat na stažení celé částky. Všechny intervaly bloků (`Op::Have`), které prošly 128bitovou kontrolou hash, jsou okamžitě uzamčeny v paměti/dočasném fondu, aby je ostatní uzly mohly stahovat souběžně. Spolu s dynamickou výměnou uzlů PEX a pomalou preempcí poloviny uzlů (Work-Stealing) je maximalizována propustnost clusteru.
- **Ochrana proti neoprávněné manipulaci s uzly a otravě klastrů**: autoritativní seznam zdrojů `root_hash` Samokonzistentní zamykání + povinná hardwarová akcelerace `gxhash128` ověření digestu před umístěním každého bloku na disk a externím nasazením; pokud je nalezen zmanipulovaný blok, jistič na milisekundové úrovni vyřadí zákeřný uzel a automaticky se obnoví z poctivého zdroje.
- **FastCDC in-situ inkrement a agregace malých souborů**: Pouze rozdílové bloky jsou přenášeny pro částečnou úpravu velkých souborů a umístěny na disk; masivní balení paměti malých souborů a diferenciální komprese prefixů eliminují režii zpáteční cesty po jednotlivých souborech.
- **Samoopravování po výpadku napájení a obnovení po restartu**: Redis-like RDB + AOF hybridní formát (`.s0_subs.bin`) se používá k zachování stavu seedání a seznamu souborů, denní změny jsou připojeny v mikrosekundách a jsou automaticky pravidelně přepisovány a organizovány. Po restartování serveru se offline změny automaticky přenesou do seedovacího uzlu.

## Technická architektura a dokumentace

- **Protokol pro oddělování dvou rovin**:
  - **Kontrola a povrch změny v reálném čase (`iroh` QUIC / TCP, první bajt `PROTO_VERSION` handshake verze) **: Zodpovídá za ověření vizitky Ed25519, ověření oprávnění, zjišťování semene Swarm a podporu pro streamování hromadných změn na úrovni `u64::MAX` a `mzstd` 5 komprimované <b id v čase <b"> 6 `FsOp::Remove` / `FsOp::Meta` Tři primitiva).
  - **Plocha distribuce dat (nativní UDP + `compio` asynchronní I/O) **: 20B kompaktní hlavička, 18B TACK periodická zpětná vazba, hybridní řízení přetížení BBRv3/Copa, obnova ztráty paketů RACK-TLP, Shannon entropie adaptivní kompresní bypass Zstd prvního bloku a blokování <b-id="2"> end-to-end.
- **Více informací**:
  - [Zpráva o měření výkonu a scénář reprodukce](https://github.com/webc-site/sync0/blob/main/doc/zh/benchmark.md)
  - [Návrh architektury systému](https://github.com/webc-site/sync0/blob/main/doc/zh/design_v4.md)
  - [Binární drátový protokol a specifikace perzistence](https://github.com/webc-site/sync0/blob/main/doc/zh/proto.md)
  - [Autentizace identity, kontrola autority a ověřování integrity](https://github.com/webc-site/sync0/blob/main/doc/zh/crypto.md)