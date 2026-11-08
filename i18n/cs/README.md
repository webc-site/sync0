# sync0

`sync0` je peer-to-peer (P2P) nástroj pro sdílení souborů a synchronizaci mezi více počítači, který konkuruje Resilio Sync.

Každý počítač je server i klient: každý uzel může publikovat více skupin sdílení souborů. Poté, co se ostatní uzly připojí, mohou automaticky získat dostupné počáteční uzly z peer serveru a stáhnout je současně z více zdrojů. Po dokončení synchronizace se automaticky vytvoří semena pro více uzlů, aby se urychlilo stahování.

## Hlavní přednosti

- **Každý je server i klient**: Srovnatelné s Resilio Sync, žádná závislost na centrálním serveru. Jakýkoli uzel může vytvořit více skupin sdílení souborů a může se také připojit ke skupinám sdílení jiných lidí; uzly si automaticky vyměňují a získávají dostupné počáteční uzly, aby bylo dosaženo souběžného stahování a synchronizace více strojů v reálném čase.
- **Adresa uzlu bez IP**: Ve výchozím nastavení se klíč uzlu a `base64url` kódovaná adresa `iroh` automaticky generují v `~/.config/sync0`, aniž byste si museli pamatovat nebo odhalovat veřejnou IP.
- **Synchronizace vícenásobného výběru na vyžádání**: Podporuje synchronizaci celé skupiny sdílení a také podporuje synchronizaci na vyžádání výběrem více souborů nebo podadresářů současně.
- **Herarchická oprávnění členů skupiny a řízení adresáře pro nahrávání**: Podporuje oprávnění pouze pro čtení (`ro`), pro čtení a zápis (`rw`) a pro správu (`admin`) a může individuálně určit předponu cesty, kterou může každý člen nahrávat (ve výchozím nastavení je nahrávání do kořenového adresáře sdílené složky povoleno).
- **Plnorychlostní a přírůstková synchronizace napříč sítěmi**: Současný přenos velkých souborů, agregace velkých malých souborů v paměti a synchronizované datové bloky pouze při částečné úpravě souborů.

## Porovnání rychlosti s rsync

### Skutečné měření transoceánské sítě z jednoho zdroje (Čína → Spojené státy, zpoždění 260 ms)

| Scénář přenosu | Velikost dat | model | `sync0` | `rsync -az` | Zvyšte multiplikátor |
| :--- | :---: | :---: | :---: | :---: | :---: |
| 500 malých souborů | 1.91 MB | Plná synchronizace | 18.84 Mbps | 5.32 Mbps | 3.5x |
| 500 malých souborů | 1.91 MB | Přírůstková synchronizace | 27.15 Mbps | 3.83 Mbps | 7.1x |
| Balíčky kódu a archivace (`.tar`) | 10.0 MB | Plná synchronizace | 139.70 Mbps | 24.97 Mbps | 5.6x |
| Balíčky kódu a archivace (`.tar`) | 10.0 MB | Přírůstková synchronizace | 83.38 Mbps | 26.02 Mbps | 3.2x |
| Video soubor (`.mp4`) | 15.0 MB | Plná synchronizace | 54.06 Mbps | 27.04 Mbps | 2.0x |
| Video soubor (`.mp4`) | 15.0 MB | Přírůstková synchronizace | 216.83 Mbps | 21.51 Mbps | 10.1x |

### Zaoceánský síť P2P multi-node pull test (Čína → Spojené státy duální uzly, zpoždění 260 ms)

Naměřené srovnání vícezdrojového seedového uzlu pruhovaného souběžného stahování:

| Scénář přenosu | Velikost dat | Seedování jednoho uzlu | Souběžné seedování P2P se dvěma uzly | Zvyšte multiplikátor |
| :--- | :---: | :---: | :---: | :---: |
| 500 malých souborů | 1.97 MB | 5.28 Mbps | 7.87 Mbps | 1.5x |
| Balíčky kódu a archivace (`.tar`) | 30.0 MB | 56.35 Mbps | 83.17 Mbps | 1.5x |
| Video soubor (`.mp4`) | 30.0 MB | 27.00 Mbps | 39.46 Mbps | 1.5x |

### Aktuální měření transkontinentální vysokorychlostní vyhrazené linky (Evropa → Severní Amerika, zpoždění 86 ms)

| Scénář přenosu | Velikost dat | `sync0` | `rsync` | Zvyšte multiplikátor |
| :--- | :---: | :---: | :---: | :---: |
| 10 000 malých souborů | 77.6 MB | 688 Mbps | 128 Mbps | 5.4x |
| Balíčky kódu a archivace (`.tar`) | 512 MB | 2111 Mbps | 438 Mbps | 4.8x |
| Video soubor (`.mp4`) | 512 MB | 556 Mbps | 338 Mbps | 1.6x |
| Přírůstková synchronizace (upravit 1% obsah) | 512 MB | 34133 Mbps | 2214 Mbps | 15.4x |

## Instalovat

### Metoda 1: Instalace přes Cargo

```bash
cargo install sync0
```

### Metoda 2: Stáhněte si předkompilované binární soubory

Přejděte na [GitHub Releases](https://github.com/webc-site/sync0/releases) a stáhněte si `.tar.xz` komprimovaný balíček pro odpovídající systém (podporuje Linux, macOS, Windows, Android, FreeBSD, NetBSD), rozbalte jej a vložte do adresáře `PATH`:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Začněte rychle

`sync0` standardně používá jako konfigurační adresář `~/.config/sync0` (pokud se potřebujete přepnout do jiných adresářů, můžete přidat `-c <adresář>`).

### 1. Zobrazte adresu místního uzlu a vizitku nastavení

```bash
# Zobrazení adresy místního uzlu (klíč je poprvé vygenerován automaticky v ~/.config/sync0)
sync0 id

# Vygenerujte si vizitku osobního uzlu (podporuje uživatelské jméno, město, e-mail, webovou stránku a podpis v rozsahu 256 slov)
sync0 profile -u alice --region SHA --email alice@example.com --url https://example.com -b "Shanghai node"
```

### 2. Publikování více skupin sdílení souborů a nastavení oprávnění členů skupiny (`.nt` Konfigurace)

`sync0` je nakonfigurován ve formátu [NestedText](https://nestedtext.org) (`.nt`). Každý uživatel má samostatný konfigurační soubor `.nt`. Adresář uživatelského profilu můžete zadat pomocí `users_dir` v `config.nt` (což automaticky zahrnuje všechny `.nt` soubory v tomto adresáři). Uživatel i předpona cesty, které lze nahrát, používají neuspořádaný seznam (`-`). **Pokud není vyplněna předpona cesty pro nahrávání, nahrávání do kořenového adresáře sdílené složky je ve výchozím nastavení povoleno**.

1. Vytvořte samostatné `.nt` konfigurační soubory pro každého člena v adresáři `~/.config/sync0/users/`:

`~/.config/sync0/users/alice.nt` (člen pouze pro čtení, omezená sazba `80MB`, `rate` podporuje jednotky `GB`, `MB`, `KB`):

```yaml
id: 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c
perm: ro
rate: 80MB
```

`~/.config/sync0/users/bob.nt` (členové s omezeným nahráváním: mohou nahrávat pouze do adresářů `uploads/bob` a `shared/common`):

```yaml
id: 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f
perm: rw
write_prefix:
  - uploads/bob
  - shared/common
```

`~/.config/sync0/users/charlie.nt` (Výchozí nahrání členů: Když je vynecháno `write_prefix`, nahrávání do kořenového adresáře sdílené složky je ve výchozím nastavení povoleno):

```yaml
id: 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
perm: rw
```

2. Nakonfigurujte uživatelský adresář a více skupin sdílení souborů v `~/.config/sync0/config.nt`:

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

Spustit službu sdílení:

```bash
# Přímé spuštění (automaticky načte všechny uživatelské konfigurace .nt v adresářích ~/.config/sync0/config.nt a users_dir)
sync0 share --region SHA

# Nebo rychle publikujte více skupin sdílení a autorizujte členy skupiny přímo pomocí příkazového řádku
sync0 share dataset:./data team-vault:./vault \
  --auth \
  --allow-ro 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c \
  --allow-rw 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f:uploads/bob,shared/common \
  --allow-rw 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
```

### 3. Vytáhněte veškerý obsah nebo vyberte více souborů a adresářů (`sync0 pull`)

Připojte se k libovolnému uzlu rovnocenného partnera, abyste automaticky získali všechny dostupné počáteční uzly ve skupině sdílení a současně vytahovali z více výchozích uzlů:

```bash
# Přetáhněte celou skupinu sdílení datové sady do místního ./local_data
sync0 pull dataset:// ./local_data -p <adresa uzlu>

# Vícenásobná synchronizace: současně vytáhněte jeden soubor docs/readme.md a podadresáře aktiva/ikony, src/core
sync0 pull dataset://docs/readme.md,assets/icons,src/core ./local_data -p <adresa uzlu>
```

### 4. Nahrajte soubor nebo adresář do skupiny sdílení (`sync0 push`)

Členové s oprávněním k nahrávání (`rw` nebo `admin`) mohou nahrávat místní soubory nebo adresáře do skupin vzdáleného sdílení:

```bash
# Členové s oprávněním k nahrávání do kořenového adresáře: Nahrajte přímo do sdíleného kořenového adresáře
sync0 push team-vault:// ./new_release -p <adresa uzlu>

# Pouze členové s oprávněním k určitému prefixu (jako je uploads/bob): upload na povolený prefix cesty
sync0 push team-vault://uploads/bob/report.pdf ./report.pdf -p <adresa uzlu>
```

### 5. Přihlaste se k odběru automatické synchronizace a seedování (`sync0 sub`)

Po přihlášení do skupiny sdílení určitého uzlu je místní server přijímajícím klientem i novým seedovacím serverem; když je soubor zdrojového uzlu aktualizován, je automaticky synchronizován a poskytuje části ke stažení pro další uzly:

```bash
# Přihlaste se k odběru celé skupiny sdílení a automaticky se zasévá
sync0 sub dataset:// ./local_mirror -p <adresa uzlu>

# Chcete-li se přihlásit k odběru konkrétních souborů a podadresářů a osadit je, vyberte pouze vícenásobný výběr
sync0 sub dataset://models/llama.onnx,configs ./local_mirror -p <adresa uzlu>
```

### 6. Synchronizace skupin s více počítači (`sync0 group`)

Více počítačů se připojí ke stejné skupině sdílení, každý druhý automaticky zjistí dostupné seed uzly a udržuje obousměrnou synchronizaci (podporuje plnou synchronizaci nebo vícenásobný výběr souborů a adresářů):

```bash
# Plně se připojte ke skupině a obousměrná synchronizace
skupina sync0 team-docs:// ./docs -p <adresa uzlu>

# Pouze adresář wal a soubor meta.json ve skupině synchronizace s vícenásobným výběrem
skupina sync0 team-docs://wal,meta.json ./docs -p <adresa uzlu>
```

### 7. Přímý přenos z bodu do bodu mezi dvěma stroji (`send` / `recv`)

Není třeba předem konfigurovat skupinu a můžete dočasně přenášet soubory nebo adresáře přímo mezi dvěma počítači:

```bash
# Monitorování přijímače
sync0 recv 0.0.0.0:9000 ./backup

# Odesílající konec přímo přenáší soubory nebo adresáře (volitelně --delete mirror pro vyčištění přebytečných souborů, --rate speed limit)
sync0 send ./project 192.168.1.10:9000
```

### 8. Vícejazyčné přepínání

`sync0` podporuje 75 jazyků a ve výchozím nastavení se automaticky přepíná podle jazykového prostředí systému; lze ji také zadat ručně pomocí `SYNC0_LANG`:

```bash
SYNC0_LANG=en sync0 --help
SYNC0_LANG=ja sync0 --help
SYNC0_LANG=de sync0 --help
```