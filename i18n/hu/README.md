# sync0

A `sync0` egy decentralizált P2P fájlmegosztó, valós idejű szinkronizálás és többforrású terjesztési eszköz, amely a `rsync` helyettesítésére szolgál - a transzkontinentális WAN mért sebessége eléri a `rsync` **5,89×** (egypontos geometriai átlagsebesség **2,67×**), P2P.1 ** geometriai átlagsebesség **2,67×**, P2P. Az óceánon túli, nagy késleltetésű link P2P-gyorsulása akár **7,68×** (A teljes kapcsolat 24 forgatókönyve átfogó geometriai átlagsebesség **2,56×**).

Minden gép egyben szerver és kliens is: nincs szükség központi szerverre vagy nyilvános hálózati címre, az automatikus lyukasztás az elosztott hálózati nyilvános kulcs címén alapul, és támogatja a közvetlen átvitelt két gép között, több megosztásos csoportengedély-tárhelyet, igény szerinti alkönyvtárak többszörös kiválasztását, ezredmásodperces szintű valós idejű növekményes szinkronizálást és P2P többforrásos egyidejűséget.

## Telepítés

### 1. módszer: Rakomány beszerelése

```bash
cargo install sync0
```

### 2. módszer: Töltse le az előre lefordított bináris fájlokat

Nyissa meg a [GitHub Releases](https://github.com/webc-site/sync0/releases) oldalt a rendszernek megfelelő `.tar.xz` tömörített csomag letöltéséhez (Linux, macOS, Windows, Android, iOS, FreeBSD támogatás), csomagolja ki és helyezze be a `PATH` mappába:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Kezdje el gyorsan

### 1. Nulla konfigurációjú közvetlen átvitel két gép között (`send` / `recv`)

Nincs szükség megosztási csoport konfigurálására, közvetlenül, nagy sebességgel viheti át a fájlokat vagy könyvtárakat két gép között:

```bash
# A fogadó oldal figyel a porton, és ír a ./backup fájlba
sync0 recv 0.0.0.0:9000 ./backup

# Közvetlen átvitel a feladótól (opcionális -- tükör törlése a felesleges fájlok törléséhez, --sebesség 50 MB sebességkorlátozás)
sync0 send ./project 192.168.1.10:9000
```

### 2. Bejegyzések megosztása és igény szerinti lehívás/feltöltés (`share` / `pull` / `push`)

```bash
# Szerver: A megosztási csoport közzététele egy parancssorral (az IP-mentes iroh csomópont címének kiadása indítás után)
sync0 share dataset:./data

# Ügyfél: Húzza át a teljes adatkészlet-megosztási csoportot a helyi ./local_data fájlba
sync0 pull dataset:// ./local_data -p <csomópont címe>

# Kliens: Igény szerint húzzon ki több kijelölést (csak meghatározott egyedi fájlok és alkönyvtárak szinkronizálása)
sync0 pull dataset://docs/readme.md,assets/icons,src/core ./local_data -p <csomópont címe>

# Kliens: Fájlok vagy könyvtárak feltöltése engedélyezett megosztási csoportokba
sync0 push dataset://releases/v1 ./build_output -p <csomópont címe>
```

### 3. Valós idejű előfizetés vetés és többgépes kétirányú szinkronizálás (`sub` / `group`)

Hosszú kapcsolat létesítése után, ha a távoli oldalon bármilyen fájl **Hozzáadás, módosítás, törlés, engedélyek módosítása vagy módosítási idő** történik, a helyi könyvtár ezredmásodperc alatt, valós időben szinkronban frissül (az új/módosított nagy fájlok csak a különbségblokkokat veszik ki és frissítik a helyükön, a fájlok törlése automatikusan megtisztítja a helyi fájlokat és üres könyvtárakat, és csak az engedélyeket módosítja):

- **`sync0 sub`(Egyirányú valós idejű tükrözés + automatikus vetés)**: A helyi valós idejű követi a megosztási vég kiegészítéseit, törléseit és módosításait, és automatikusan P2P-magforrásként szolgál, hogy párhuzamos gyorsítást biztosítson a többi lehúzó csomópont számára.
- **`sync0 group`(Többgépes peer-to-peer kétirányú, valós idejű szinkronizálás)**: Ha a csoport bármely gépén fájlokat adnak hozzá, törölnek vagy módosítanak, az összes többi online csomópont valós időben szinkronban frissül, és magokként szolgál egymás számára.

```bash
# Iratkozzon fel a teljes megosztási csoportra: a távoli hozzáadások, törlések és módosítások helyileg, valós időben szinkronizálódnak, és automatikusan feltöltésre kerülnek
sync0 sub dataset:// ./local_mirror -p <csomópont címe>

# Igény szerinti többszörös kijelölésű előfizetés: csak a megadott egyedi fájlok és alkönyvtárak szinkronizálása és küldése valós időben
sync0 sub dataset://models/llama.onnx,configs ./local_mirror -p <csomópont címe>

# Többgépes peer csoport kétirányú valós idejű szinkronizálás (támogatja a teljes vagy vesszős többszörös kijelölési útvonalakat is)
sync0 csoport team-docs:// ./docs -p <csomópont címe>
```

### 4. Csoportos többmegosztási csoport és tagengedély-konfiguráció (`.nt` konfiguráció)

Az alapértelmezett konfigurációs könyvtár a `~/.config/sync0` (a `-c <directory>`-en keresztül váltható), a minimalista [NestedText](https://nestedtext.org) (`.nt`) formátum használatával:
- **Támogatja a nyilvános hálózati olvasás korlátozását (csak a jogosult felhasználók olvashatják)**: Amíg a megosztási csoport `users:` taglistával van konfigurálva (vagy a parancssor a `--allow-r` / `--allow-rw` / `--allow-w` / `--auth` értéket adja meg), a rendszer automatikusan bekapcsolja a **id = ** és a nyilvános hitelesítést ("> de nem engedélyezi a hálózatot) felsorolás vagy olvasás, valamint a kezdő csomópontokra való P2P előfizetés egyidejűleg örökli az engedélyezőlistát is, így csak a jogosult felhasználók olvashatnak; azok a megosztási csoportok, amelyek nincsenek beállítva a `users` beállítással, alapértelmezés szerint nyilvánosak és csak olvashatók (ha nyilvánosak és csak olvashatók szeretnének lenni, és csak meghatározott tagok feltölthetik, akkor kifejezetten beállíthatja a `auth: false` beállítást).
- **Finomszemcsés olvasási-írási és alkönyvtár-kézbesítési engedélyek**: A tagok alapértelmezés szerint csak olvashatóak (`r` nem kell kifejezetten írni), és támogatják a `r` (csak olvasható), `w` (csak a drop boxba írható, nem olvasható), `rw` és `rw` (read-bit the idrite)"> (read-bit idrite) feltöltéshez).

**Példa tagkonfigurációra (`~/.config/sync0/users/`):**

```yaml
# ~/.config/sync0/users/alice.nt (alapértelmezett csak olvasható, sebességkorlátozás 80 MB)
id: 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c
rate: 80MB
```

```yaml
# ~/.config/sync0/users/bob.nt (olvasási és írási jogosultság, csak a megadott alkönyvtárba tölthető fel)
id: 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f
perm: rw
write_prefix:
  - uploads/bob
  - shared/common
```

```yaml
# ~/.config/sync0/users/charlie.nt (csak írási kézbesítési engedély, olvasás tilos)
id: 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
perm: w
```

**Példa a megosztási csoport konfigurációjára (`~/.config/sync0/config.nt`):**

```yaml
users_dir: ./users

groups:
  # Nem konfigurált felhasználók: alapértelmezett nyilvános hálózat csak olvasható
  dataset:
    path: ./data
    rate: 50MB

  # Konfigurált felhasználók: automatikusan korlátozza a nyilvános hálózati olvasást (csak Alice és Bob tud olvasni, Charlie csak kézbesíteni és feltölteni)
  team-vault:
    path: ./vault
    users:
      - alice
      - bob
      - charlie
```

Indítsa el a szolgáltatást (töltse be a konfigurációs fájlt vagy használja közvetlenül a parancssori paramétereket a nyilvános hálózat olvasásának korlátozásához és a tagok engedélyezéséhez):

```bash
# Az indításhoz töltse be a ~/.config/sync0/config.nt fájlt
sync0 share --region SHA

# Vagy tegyen közzé egy privát megosztási csoportot tiszta parancssoron keresztül (adja meg az --allow-* paramétert, hogy automatikusan korlátozza a nyilvános hálózati olvasást, és csak a jogosult felhasználók férhessenek hozzá)
sync0 share team-vault:./vault \
  --allow-r 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c \
  --allow-rw 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f:uploads/bob,shared/common \
  --allow-w 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
```

### 5. Csomópont címe, névjegykártya és többnyelvű

```bash
# A helyi IP-mentes csomópont címének megtekintése (a kulcs automatikusan generálódik az első futtatáshoz)
sync0 id

# Ed25519 önaláírt csomópont névjegykártya létrehozása (becenév, városkód, e-mail, webhely, bemutatkozás)
sync0 profile -u alice --region SHA --email alice@example.com -b "Shanghai node"

# 75 beépített nyelv van, amelyek alapértelmezés szerint követik a rendszer nyelvét, és környezeti változókon keresztül válthatók.
SYNC0_LANG=en sync0 --help
```

## Teljesítmény és alapvető jellemzők

### Kontinentális WAN tényleges mérés (összehasonlítás: `rsync`)

Tényleges mérési környezet: Egyesült Államok `c1` → Európa `n1` (RTT 65 ms), az adatkészlet Linux kernel `linux-7.3.3`:

#### 1) A la carte mód vs `rsync`

| Adatkészlet | méret | `sync0` Egypontos<br>(Mbps) | `rsync`<br>(Mbps) | Gyorsítás (`sync0` / `rsync`) |
| :--- | ---: | ---: | ---: | ---: |
| `linux-7.3.3.tar.xz`(nem tömöríthető) | 155 MB | **236.48** | 108.74 | **2,17×** |
| `linux-7.3.3.tar`(tömöríthető) | 1.6 GB | **843.83** | 385.06 | **2,19×** |
| `linux-7.3.3.tar`(helyi változás növekménye) | 1.6 GB | **8,014.15** | 1,361.72 | **5,89×** |
| `linux-7.3.3/` (95 979 kis fájl) | 1.5 GB | **627.58** | 347.80 | **1,80×** |
| **geometriai átlag** | — | **1,000.91** | 375.26 | **2,67×** |

#### 2) P2P kétforrású mód vs `rsync`

| Adatkészlet | méret | `sync0` P2P kettős forrás<br>(Mbps) | `rsync`<br>(Mbps) | Gyorsítás (`sync0` / `rsync`) |
| :--- | ---: | ---: | ---: | ---: |
| `linux-7.3.3.tar.xz`(nem tömöríthető) | 155 MB | **330.09** | 108.74 | **3,04×** |
| `linux-7.3.3.tar`(tömöríthető) | 1.6 GB | **1,551.86** | 385.06 | **4,03×** |
| `linux-7.3.3.tar`(helyi változás növekménye) | 1.6 GB | **5,474.17** | 1,361.72 | **4,02×** |
| `linux-7.3.3/` (95 979 kis fájl) | 1.5 GB | **706.56** | 347.80 | **2,03×** |
| **geometriai átlag** | — | **1,186.42** | 375.26 | **3,16×** |

> Megjegyzés: P2P kettős forrású módban az európai csomópontok `s1` együttműködnek a vetésben, és a kontinenseket átívelő gerinc RTT 65 ms.

### A termék fő jellemzői

- **Nyilvános kulcs címzés automatikus lyukasztás**: Nincs szükség központi szerverre és nyilvános hálózati címre, automatikus NAT lyukasztás az elosztott hálózati nyilvános kulcs címe alapján.
- **Ezredmásodperces szintű valós idejű szinkronizálás és metaadat-tudatosság**: A natív fájlesemény-felügyeleten (`FSEvents` / `inotify` / `ReadDirectoryChangesW`), a fájlok hozzáadásának, törlésének, POSIX-engedélyeinek (`mode`) és ezredmásodperc-szintű módosítási időnek (<b">5) valós idejű leküldésén alapul; nulla adatblokk átvitel, ha csak az engedélyek vagy az idő módosul.
- **BT-stílusú egyidejű letöltés és feltöltés, valamint regionális affinitás ütemezése**: A letöltési csomópontnak nem kell megvárnia a teljes mennyiség letöltését. Az összes blokkintervallum (`Op::Have`), amely átment a 128 bites kivonatellenőrzésen, azonnal zárolva van a memóriában/ideiglenes készletben, hogy más csomópontok párhuzamosan lehívhassák. A dinamikus PEX csomópontcserével és a lassú csomópont-elővétellel (Work-Stealing) együtt a fürt átviteli sebessége maximalizálható.
- **A rosszindulatú csomópontok manipulálása és fürtmérgezés elleni védelem**: mérvadó forráslista `root_hash` Önkonzisztens zárolás + kötelező hardveres gyorsítás `gxhash128` kivonatellenőrzés, mielőtt minden blokk a lemezre kerül, és külsőleg bekerülne; ha manipulált blokkot talál, az ezredmásodperces szintű megszakító kikapcsolja a rosszindulatú csomópontot, és automatikusan helyreáll a becsületes forrásból.
- **FastCDC in situ növekmény és kis fájlok aggregálása**: Csak a differenciális blokkok kerülnek átvitelre a nagy fájlok részleges módosításához, és helyezik el őket a lemezen; masszív kis fájlmemória-csomagolás és az előtagok differenciális tömörítése kiküszöböli a fájlról fájlra történő oda-vissza utazást.
- **Öngyógyulás áramkimaradás után és újraindítás után**: A Redis-szerű RDB + AOF hibrid formátum (`.s0_subs.bin`) a vetés állapotának és a fájllistának a megőrzésére szolgál, a napi változásokat mikroszekundumokban hozzáfűzik, és automatikusan átírják és rendszeresen rendszerezik. A kiszolgáló újraindítása után az offline módosítások automatikusan a vetési csomópontba kerülnek.

## Műszaki architektúra és dokumentáció

- **Kétsíkú szétválasztási protokoll**:
  - **Vezérlő és valós idejű változási felület (`iroh` QUIC / TCP, első bájt `PROTO_VERSION` verzió kézfogása) **: Felelős az Ed25519 névjegykártya ellenőrzéséért, az engedélyek ellenőrzéséért, a Swarm seed felfedezéséért, valamint a `u64::MAX` szintű tömeges változás streaming (valós idejű push = 4> kötegelés és <b">b-4 id <b">b-4 id) támogatásáért id="6"> / `FsOp::Remove` / `FsOp::Meta` Három primitív).
  - **Adatelosztási felület (natív UDP + `compio` aszinkron I/O) **: 20B kompakt fejléc, 18B TACK periodikus visszacsatolás, BBRv3/Copa hibrid torlódásvezérlés, RACK-TLP csomagvesztés-helyreállítás, első blokk Shannon-entrópia adaptív Zstd tömörítési bypass="2">blokk end-to-end-beállítása és <b.
- **További információ**:
  - [Teljesítménymérési jelentés és reprodukciós szkript](https://github.com/webc-site/sync0/blob/main/doc/zh/benchmark.md)
  - [Rendszer architektúra tervezés](https://github.com/webc-site/sync0/blob/main/doc/zh/design_v4.md)
  - [Bináris vezetékes protokoll és perzisztencia-specifikáció](https://github.com/webc-site/sync0/blob/main/doc/zh/proto.md)
  - [Személyazonosság-hitelesítés, jogosultság-ellenőrzés és integritás-ellenőrzés](https://github.com/webc-site/sync0/blob/main/doc/zh/crypto.md)