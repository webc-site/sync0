# sync0

A `sync0` egy peer-to-peer (P2P) fájlmegosztó és többgépes szinkronizálási eszköz, amely versenyez a Resilio Sync-el.

Minden gép egyben kiszolgáló és kliens is: bármelyik csomópont több fájlmegosztó csoportot is közzé tud tenni. Más csomópontok csatlakozása után automatikusan lekérhetik az elérhető kezdőcsomópontokat a peer szervertől, és egyszerre több forrásból is lekérhetik azokat. A szinkronizálás befejezése után a letöltések felgyorsítása érdekében automatikusan létrejön a magok további csomópontokhoz.

## Core Highlights

- **Mindenki egyszerre kiszolgáló és kliens**: A Resilio Sync-hez hasonlítható, nincs központi szerverfüggőség. Bármely csomópont több fájlmegosztó csoportot hozhat létre, és csatlakozhat mások megosztási csoportjaihoz is; A csomópontok automatikusan kicserélik és beszerzik az elérhető kezdőcsomópontokat, hogy több gép egyidejű lehúzását és valós idejű szinkronizálását érjék el.
- **IP-mentes csomópontcím**: Alapértelmezés szerint a csomópontkulcs és a `base64url` kódolt `iroh` cím automatikusan létrejön az `~/.config/sync0` helyen, anélkül, hogy meg kellene emlékezni vagy fel kellene tárni a nyilvános IP-címet.
- **Többszörös szinkronizálás igény szerint**: Támogatja a teljes megosztási csoport szinkronizálását, és támogatja az igény szerinti szinkronizálást is több fájl vagy alkönyvtár egyidejű kiválasztásával.
- **Csoporttagok hierarchikus engedélyei és feltöltési könyvtár vezérlése**: Támogatja a csak olvasási (`ro`), olvasási-írási feltöltés (`rw`) és kezelési (`admin`) engedélyeket, és egyénileg megadhatja az elérési út előtagját, amelyet minden tag feltölthet (alapértelmezés szerint a megosztás gyökérkönyvtárába történő feltöltés engedélyezett).
- **Teljes sebességű és növekményes szinkronizálás a hálózatokon**: Nagy fájlok egyidejű átvitele, nagy méretű kis fájlok összesítése a memóriában, és csak szinkronizált adatblokkok, ha a fájlok részben módosítva vannak.

## Sebesség-összehasonlítás rsync-cel

### Transoceanic network egyetlen forrásból származó tényleges mérés (Kína → Egyesült Államok, késleltetés 260 ms)

| Átviteli forgatókönyv | Adatméret | modell | `sync0` | `rsync -az` | Növelje a szorzót |
| :--- | :---: | :---: | :---: | :---: | :---: |
| 500 kis fájl | 1.91 MB | Teljes szinkronizálás | 18.84 Mbps | 5.32 Mbps | 3.5x |
| 500 kis fájl | 1.91 MB | Növekményes szinkronizálás | 27.15 Mbps | 3.83 Mbps | 7.1x |
| Kód- és archiválási csomagok (`.tar`) | 10.0 MB | Teljes szinkronizálás | 139.70 Mbps | 24.97 Mbps | 5.6x |
| Kód- és archiválási csomagok (`.tar`) | 10.0 MB | Növekményes szinkronizálás | 83.38 Mbps | 26.02 Mbps | 3.2x |
| Videofájl (`.mp4`) | 15.0 MB | Teljes szinkronizálás | 54.06 Mbps | 27.04 Mbps | 2.0x |
| Videofájl (`.mp4`) | 15.0 MB | Növekményes szinkronizálás | 216.83 Mbps | 21.51 Mbps | 10.1x |

### Transoceanic network P2P többcsomópontos lehúzási teszt (Kína → Egyesült Államok kettős csomópontok, késleltetés 260 ms)

A többforrású magcsomópont csíkos egyidejű húzásának mért összehasonlítása:

| Átviteli forgatókönyv | Adatméret | Egycsomópontos vetés | Két csomópontos P2P párhuzamos vetés | Növelje a szorzót |
| :--- | :---: | :---: | :---: | :---: |
| 500 kis fájl | 1.97 MB | 5.28 Mbps | 7.87 Mbps | 1.5x |
| Kód- és archiválási csomagok (`.tar`) | 30.0 MB | 56.35 Mbps | 83.17 Mbps | 1.5x |
| Videofájl (`.mp4`) | 30.0 MB | 27.00 Mbps | 39.46 Mbps | 1.5x |

### A transzkontinentális nagysebességű dedikált vonal tényleges mérése (Európa → Észak-Amerika, késés 86 ms)

| Átviteli forgatókönyv | Adatméret | `sync0` | `rsync` | Növelje a szorzót |
| :--- | :---: | :---: | :---: | :---: |
| 10 000 kis fájl | 77.6 MB | 688 Mbps | 128 Mbps | 5.4x |
| Kód- és archiválási csomagok (`.tar`) | 512 MB | 2111 Mbps | 438 Mbps | 4.8x |
| Videofájl (`.mp4`) | 512 MB | 556 Mbps | 338 Mbps | 1.6x |
| Növekményes szinkronizálás (1%-os tartalom módosítása) | 512 MB | 34133 Mbps | 2214 Mbps | 15.4x |

## Telepítés

### 1. módszer: Telepítés a Cargo-n keresztül

```bash
cargo install sync0
```

### 2. módszer: Töltse le az előre lefordított bináris fájlokat

Nyissa meg a [GitHub Releases](https://github.com/webc-site/sync0/releases) oldalt a `.tar.xz` tömörített csomag letöltéséhez a megfelelő rendszerhez (támogatja a Linuxot, a macOS-t, a Windowst, az Androidot, a FreeBSD-t, a NetBSD-t), csomagolja ki és helyezze el a `PATH` könyvtárba:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Kezdje el gyorsan

A `sync0` alapértelmezés szerint a `~/.config/sync0`-t használja konfigurációs könyvtárként (ha más könyvtárakra kell váltania, hozzáadhatja a `-c <könyvtár>`-t).

### 1. Tekintse meg a helyi csomópont címét és beállítási névjegykártyáját

```bash
# Tekintse meg a helyi csomópont címét (a kulcs először automatikusan generálódik a ~/.config/sync0 fájlban)
sync0 id

# Személyes csomópont névjegykártya létrehozása (256 szón belül támogatja a felhasználónevet, a várost, az e-mail címet, a webhelyet és az aláírást)
sync0 profile -u alice --region SHA --email alice@example.com --url https://example.com -b "Shanghai node"
```

### 2. Több fájlmegosztó csoport közzététele és a csoporttagok engedélyeinek beállítása (`.nt` Konfiguráció)

A `sync0` [NestedText](https://nestedtext.org) (`.nt`) formátumban van konfigurálva. Minden felhasználónak külön `.nt` konfigurációs fájlja van. A felhasználói profil könyvtárát a `config.nt` `users_dir` elemén keresztül adhatja meg (amely automatikusan tartalmazza a könyvtárban található összes `.nt` fájlt). Mind a felhasználó, mind a feltölthető elérési út előtagja rendezetlen listát (`-`) használ. **Ha a feltöltési útvonal előtagja nincs kitöltve, a megosztás gyökérkönyvtárába történő feltöltés alapértelmezés szerint engedélyezett**.

1. Hozzon létre külön `.nt` konfigurációs fájlokat a `~/.config/sync0/users/` könyvtár minden tagjához:

`~/.config/sync0/users/alice.nt` (csak olvasható tag, korlátozott arányú `80MB`, `rate` támogatja a `GB`, `MB`, `KB` egységeket):

```yaml
id: 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c
perm: ro
rate: 80MB
```

`~/.config/sync0/users/bob.nt` (korlátozott feltöltési tagok: csak a `uploads/bob` és `shared/common` könyvtárba tölthetnek fel):

```yaml
id: 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f
perm: rw
write_prefix:
  - uploads/bob
  - shared/common
```

`~/.config/sync0/users/charlie.nt` (Alapértelmezett tagfeltöltés: Ha a `write_prefix` ki van hagyva, a megosztás gyökérkönyvtárába való feltöltés alapértelmezés szerint engedélyezett):

```yaml
id: 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
perm: rw
```

2. Konfigurálja a felhasználói könyvtárat és több fájlmegosztó csoportot itt: `~/.config/sync0/config.nt`:

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

Indítsa el a megosztási szolgáltatást:

```bash
# Közvetlen indítás (automatikusan betölti az összes .nt felhasználói konfigurációt a ~/.config/sync0/config.nt és a users_dir könyvtárakba)
sync0 share --region SHA

# Vagy tegyen közzé gyorsan több megosztási csoportot, és engedélyezze a csoporttagokat közvetlenül a parancssoron keresztül
sync0 share dataset:./data team-vault:./vault \
  --auth \
  --allow-ro 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c \
  --allow-rw 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f:uploads/bob,shared/common \
  --allow-rw 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
```

### 3. Húzza ki az összes tartalmat, vagy válasszon ki több fájlt és könyvtárat (`sync0 pull`)

Csatlakozzon bármely társcsomóponthoz, hogy automatikusan megszerezze a megosztási csoport összes elérhető magcsomópontját, és egyszerre több magcsomópontból is lekérjen:

```bash
# Húzza át a teljes adatkészlet-megosztási csoportot a helyi ./local_data fájlba
sync0 pull dataset:// ./local_data -p <csomópont címe>

# Többszörös szinkronizálás: egyszerre húzza le a docs/readme.md fájlt és az eszközök/icons, src/core alkönyvtárakat
sync0 pull dataset://docs/readme.md,assets/icons,src/core ./local_data -p <csomópont címe>
```

### 4. Fájl vagy könyvtár feltöltése megosztási csoportba (`sync0 push`)

A feltöltési engedéllyel (`rw` vagy `admin`) rendelkező tagok helyi fájlokat vagy könyvtárakat tölthetnek fel távoli megosztási csoportokba:

```bash
# Gyökérkönyvtár-feltöltési engedéllyel rendelkező tagok: Feltöltés közvetlenül a megosztott gyökérkönyvtárba
sync0 push team-vault:// ./new_release -p <csomópont címe>

# Csak azok a tagok, akik jogosultak egy adott előtaghoz (például feltöltések/bob): töltsön fel az engedélyezett elérési út előtagjára
sync0 push team-vault://uploads/bob/report.pdf ./report.pdf -p <csomópont címe>
```

### 5. Feliratkozás az automatikus szinkronizálásra és küldésre (`sync0 sub`)

Miután előfizetett egy bizonyos csomópont megosztási csoportjára, a helyi szerver egyszerre fogadó kliens és új kezdőkiszolgáló is; a forráscsomópont-fájl frissítésekor automatikusan szinkronizálódik, és csonkolt letöltéseket biztosít a többi csomópont számára:

```bash
# Iratkozzon fel a teljes megosztási csoportra, és automatikusan induljon el
sync0 sub dataset:// ./local_mirror -p <csomópont címe>

# Csak a többszörös kijelölést választva előfizethet meghatározott fájlok és alkönyvtárak számára, és elküldheti azokat
sync0 sub dataset://models/llama.onnx,configs ./local_mirror -p <csomópont címe>
```

### 6. Többgépes csoportos társszinkronizálás (`sync0 group`)

Több gép csatlakozik ugyanahhoz a megosztási csoporthoz, egymás automatikusan felfedezik az elérhető kezdőcsomópontokat, és fenntartják a kétirányú szinkronizálást (támogatja a teljes szinkronizálást vagy a fájlok és könyvtárak többszörös kiválasztását):

```bash
# Teljesen csatlakozzon a csoporthoz és a kétirányú szinkronizáláshoz
sync0 csoport team-docs:// ./docs -p <csomópont címe>

# Csak a wal könyvtár és a meta.json fájl a többszörös kijelölésű szinkronizálási csoportban
sync0 csoport team-docs://wal,meta.json ./docs -p <csomópont címe>
```

### 7. Pont-pont közvetlen átvitel két gép között (`send` / `recv`)

Nincs szükség csoportok előzetes konfigurálására, és ideiglenesen közvetlenül átvihet fájlokat vagy könyvtárakat két gép között:

```bash
# Vevő felügyelet
sync0 recv 0.0.0.0:9000 ./backup

# A küldő oldal közvetlenül továbbítja a fájlokat vagy könyvtárakat (opcionális --delete tükör a felesleges fájlok eltávolításához, --rate sebességkorlátozás)
sync0 send ./project 192.168.1.10:9000
```

### 8. Többnyelvű váltás

Az `sync0` 75 nyelvet támogat, és alapértelmezés szerint automatikusan a rendszer nyelvi környezetének megfelelően vált; manuálisan is megadható a `SYNC0_LANG` segítségével:

```bash
SYNC0_LANG=en sync0 --help
SYNC0_LANG=ja sync0 --help
SYNC0_LANG=de sync0 --help
```