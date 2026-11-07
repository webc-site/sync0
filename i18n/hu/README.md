# sync0

Az `sync0` egy parancssori eszköz a fájlok hálózaton keresztüli átvitelére, a könyvtárak fokozatos szinkronizálására és az adatok egyidejű elosztására több gépen.

## Eredeti tervezési szándék

Gépek közötti helyiségekben, országokban vagy hálózati késleltetésekkel és csomagvesztésekkel járó környezetekben az olyan eszközök, mint az `rsync` és az `scp` egyetlen kapcsolódási sorrend megerősítésén alapulnak, ami korlátozza a sávszélesség kihasználását; amikor több gép ugyanarról a szerverről húz adatokat egyszerre, a forrásszerver hálózati kártyája könnyen szűk keresztmetszetgé válhat.

Az `sync0` célja két probléma megoldása:

1. Hagyja, hogy a két gép közötti fájl- és könyvtárszinkronizálás teljes fizikai sávszélességen működjön, és csak akkor vigye át a megváltozott részt, ha a fájl részben módosul.
2. Több gép is átvihet adatblokkokat egymásnak, amikor ugyanazt a címtárat szinkronizálja, csökkentve ezzel a forrásszerver terhelését és lerövidítve a teljes szinkronizálási időt.

## Sebesség-összehasonlítás rsync-cel

Tesztkörnyezet: európai csomópont $\leftrightarrow$ észak-amerikai csomópont, oda-vissza út késleltetése 86 ms, a teljes időigényes mérés, beleértve a címtárvizsgálatot, a deduplikáció összehasonlítását, a hálózati átvitelt és a lemezelhelyezés ellenőrzését:

| Átviteli forgatókönyv | Adatméret | `sync0` | `rsync` | több- |
| :--- | :---: | :---: | :---: | :---: |
| 10 000 kis fájl | 77.6 MB | 0.90 s (688 Mbps) | 4.86 s (128 Mbps) | 5.4x |
| Kód- és archiválási csomagok (`.tar`) | 512 MB | 1.94 s (2111 Mbps) | 9.36 s (438 Mbps) | 4.8x |
| Videofájl (`.mp4`) | 512 MB | 7.36 s (556 Mbps) | 12.11 s (338 Mbps) | 1.6x |
| Növekményes szinkronizálás (1%-os tartalom módosítása) | 512 MB | 0.12 s | 1.85 s | 15.4x |

## Teljesítményjavító módszerek

- **UDP egyidejű átvitel**: Adatblokkok küldése UDP-n keresztül, és csak az elveszett töredékek újraküldése, így elkerülhető, hogy egyetlen csomagvesztés blokkolja a további fájlátviteleket.
- **Tartalom szegmentálása és deduplikációja**: A fájl felosztása adatblokkokra a tartalom szerint, és kiszámítja az ellenőrző értéket. Ugyanaz a tartalom, amely már a fogadó oldalon van, újra felhasználható közvetlenül, a hálózat megkettőzése nélkül.
- **A tömörítés automatikus beállítása a hálózat sebességének megfelelően**: Tömörítés bekapcsolása a tömöríthető tartalmakhoz, például szöveghez és kódhoz; automatikusan kihagyja a tömörítést videók, képek, tömörített csomagok vagy nagy sebességű LAN esetén a CPU-fogyasztás csökkentése érdekében.
- **Több csomópont közeli lehívása**: Ha több gép húzza le ugyanazt a könyvtárat, az adatblokkok egyidejűleg lesznek levonva az ugyanabban a városban található letöltött csomópontokból, vagy alacsonyabb késleltetéssel.

## Telepítés

### 1. módszer: Telepítés a Cargo-n keresztül

```bash
cargo install sync0
```

### 2. módszer: Töltse le az előre lefordított bináris fájlokat

Nyissa meg a [GitHub Releases](https://github.com/webc-site/sync0/releases) oldalt a `.tar.xz` tömörített csomag letöltéséhez a megfelelő rendszerhez (támogatja a Linux x86_64/ARM64, macOS Apple Silicon/Intel, Windows x64/ARM64, Android ARM64/x86_64, FreeBSD, NetBSD rendszereket). A kicsomagolás után helyezze a `sync0` elemet a `PATH` könyvtárba:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Felhasználói kézikönyv

### 1. Fájlok vagy könyvtárak közvetlen átvitele két gép között

Indítsa el a figyelést a fogadó gépen, és adja meg a fogadási és mentési útvonalat:

```bash
sync0 recv 0.0.0.0:9000 ./backup
```

Egyetlen fájl vagy egy teljes könyvtár küldése a küldő gépről:

```bash
sync0 send ./project 192.168.1.10:9000
```

Gyakran használt paraméterek:

```bash
# Törölje a redundáns fájlokat, amelyek a fogadó oldalon léteznek, de a küldő oldalon nem (tükrös szinkronizálás)
sync0 send ./project 192.168.1.10:9000 --delete

# Korlátozott küldési sebesség (mértékegység: bájt/másodperc, például 50 MB/s)
sync0 send ./project 192.168.1.10:9000 --rate 50000000

# Csendes mód (nincs látható folyamatjelző)
sync0 send ./project 192.168.1.10:9000 -q
```

### 2. Megosztott könyvtárak közzététele és lehívás több csomópontból

Tegyen közzé egy megosztott könyvtárat (névvel `dataset`) az eredeti kiszolgálón:

```bash
sync0 share dataset ./data --region SHA
```

Húzza ki a megosztott könyvtárat más gépeken (egyszerre több csomópontcímet is kitölthet, vesszővel elválasztva, hogy egyidejűleg lehessen lehívni):

```bash
sync0 pull 10.0.0.1:9000,10.0.0.2:9000 dataset ./local_data
```

Csak egy bizonyos alkönyvtárat húzzon ki a megosztott könyvtárból:

```bash
sync0 pull 10.0.0.1:9000 dataset ./local_data --subpath 2026/logs
```

A helyi módosítások visszaküldése a távoli megosztott könyvtárba:

```bash
sync0 push 10.0.0.1:9000 dataset ./local_data
```

### 3. Feliratkozás a katalógus automatikus frissítésére

Iratkozzon fel a forráscsomópont megosztott könyvtárára. Amikor egy fájlt hozzáadunk vagy módosítunk a forráskönyvtárban, a rendszer automatikusan szinkronizálja a helyi számítógéppel. Ugyanakkor a helyi gép automatikusan csomópontként fog szolgálni más gépek számára, hogy húzzák:

```bash
sync0 sub 10.0.0.1:9000 dataset ./local_mirror
```

### 4. Többgépes csoportok kétirányú szinkronizálása

Több gép csatlakozik ugyanahhoz a csoportnévhez, és a csomópontok bármely könyvtárában bekövetkezett változások automatikusan szinkronizálódnak a csoport többi csomópontjával:

```bash
sync0 group team-docs ./docs 10.0.0.1:9000,10.0.0.2:9000
```

### 5. Csomópont-azonosító és személyes névjegykártya

Mindegyik csomópont támogatja a felhasználónév, a város, az e-mail cím, a webhely és a személyre szabott aláírás (maximum 256 karakter) beállítását, és átadja a helyi privát kulcs aláírás hitelesítését:

```bash
# A helyi nyilvános kulcs azonosítójának megtekintése (adja meg a konfigurációs könyvtárat a kulcs megőrzéséhez)
sync0 id -c ~/.config/sync0

# Személyre szabott aláírásokkal ellátott csomóponti névjegykártyák létrehozása és megjelenítése
sync0 profile -c ~/.config/sync0 \
  -u alice \
  --region SHA \
  --email alice@example.com \
  --url https://example.com \
  -b "Shanghai backup node"
```

A `share`, `pull`, `sub`, `group` a következő profilparaméterekkel is elindítható:

```bash
sync0 share dataset ./data -c ~/.config/sync0 -u alice --region SHA -b "Shanghai backup node"
```

### 6. Többnyelvű váltás

`sync0` 75 nyelvet támogat. Alapértelmezés szerint a parancssori súgó és a megfelelő nyelvű promptok automatikusan megjelennek az operációs rendszer nyelvi környezetének megfelelően; ha a rendszer nyelve nem szerepel a támogatott listában, akkor alapértelmezés szerint az angol használatos.

A nyelv manuális megadásához állítsa be a `SYNC0_LANG` környezeti változót:

```bash
SYNC0_LANG=en sync0 --help
SYNC0_LANG=ja sync0 --help
SYNC0_LANG=de sync0 --help
```