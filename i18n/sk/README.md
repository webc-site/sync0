# sync0

`sync0` je nástroj príkazového riadka na prenos súborov cez siete, prírastkovú synchronizáciu adresárov a súčasnú distribúciu údajov do viacerých počítačov.

## Pôvodný dizajnový zámer

V miestnostiach s viacerými strojmi, v krajinách alebo prostrediach so sieťovým oneskorením a stratou paketov sú nástroje ako `rsync` a `scp` založené na potvrdení sekvencie jedného pripojenia, čo obmedzuje využitie šírky pásma; keď viaceré stroje sťahujú údaje z rovnakého servera súčasne, sieťová karta zdrojového servera sa môže ľahko stať prekážkou.

Cieľom `sync0` je vyriešiť dva problémy:

1. Nechajte synchronizáciu súborov a adresárov medzi týmito dvoma počítačmi bežať s plnou fyzickou šírkou pásma a preneste zmenenú časť iba vtedy, keď je súbor čiastočne upravený.
2. Viaceré stroje si môžu navzájom prenášať dátové bloky pri synchronizácii rovnakého adresára, čím sa znižuje zaťaženie zdrojového servera a skracuje sa celkový čas synchronizácie.

## Porovnanie rýchlosti s rsync

Testovacie prostredie: Európsky uzol $\leftrightarrow$ Severoamerický uzol, spiatočné oneskorenie 86 ms, meranie celkovej časovej náročnosti vrátane skenovania adresárov, porovnania deduplikácie, sieťového prenosu a overenia umiestnenia disku:

| Scenár prenosu | Veľkosť údajov | `sync0` | `rsync` | viacnásobný |
| :--- | :---: | :---: | :---: | :---: |
| 10 000 malých súborov | 77.6 MB | 0.90 s (688 Mbps) | 4.86 s (128 Mbps) | 5.4x |
| Kódové a archivačné balíčky (`.tar`) | 512 MB | 1.94 s (2111 Mbps) | 9.36 s (438 Mbps) | 4.8x |
| Video súbor (`.mp4`) | 512 MB | 7.36 s (556 Mbps) | 12.11 s (338 Mbps) | 1.6x |
| Prírastková synchronizácia (upraviť 1% obsah) | 512 MB | 0.12 s | 1.85 s | 15.4x |

## Metódy zvyšovania výkonu

- **Súbežný prenos UDP**: Odosielajte bloky údajov prostredníctvom protokolu UDP a opätovne prenášajte iba stratené fragmenty, aby ste predišli strate jedného paketu blokujúcej následné prenosy súborov.
- **Segmentácia obsahu a deduplikácia**: Rozdeľte súbor na dátové bloky podľa obsahu a vypočítajte kontrolnú hodnotu. Rovnaký obsah, ktorý sa už nachádza na prijímacej strane, sa opätovne používa priamo lokálne bez duplikácie siete.
- **Automaticky upraviť kompresiu podľa rýchlosti siete**: Zapnite kompresiu pre komprimovateľný obsah, ako je text a kód; automaticky preskočí kompresiu pre videá, obrázky, komprimované balíky alebo pri vysokorýchlostnej sieti LAN, aby sa znížila spotreba procesora.
- **Sťahovanie viacerých uzlov v okolí**: Keď viacero počítačov stiahne rovnaký adresár, bloky údajov sa budú sťahovať súčasne zo stiahnutých uzlov v rovnakom meste alebo s nižšou latenciou.

## Inštalovať

### Metóda 1: Inštalácia cez Cargo

```bash
cargo install sync0
```

### Metóda 2: Stiahnite si predkompilované binárne súbory

Prejdite na [GitHub Releases](https://github.com/webc-site/sync0/releases) a stiahnite si komprimovaný balík `.tar.xz` pre príslušný systém (podporuje Linux x86_64/ARM64, macOS Apple Silicon/Intel, Windows x64/ARM64, Android ARM64/x86_64, FreeBSD, NetBSD). Po rozbalení umiestnite `sync0` do adresára `PATH`:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Používateľská príručka

### 1. Priamy prenos súborov alebo adresárov medzi dvoma počítačmi

Spustite monitorovanie na prijímajúcom počítači a zadajte cestu prijímania a ukladania:

```bash
sync0 recv 0.0.0.0:9000 ./backup
```

Odošlite jeden súbor alebo celý adresár z odosielajúceho zariadenia:

```bash
sync0 send ./project 192.168.1.10:9000
```

Bežne používané parametre:

```bash
# Odstráňte nadbytočné súbory, ktoré existujú na prijímacej strane, ale nie na odosielajúcej strane (zrkadlová synchronizácia)
sync0 send ./project 192.168.1.10:9000 --delete

# Obmedzte rýchlosť odosielania (jednotka: bajty/sekundu, napríklad 50 MB/s)
sync0 send ./project 192.168.1.10:9000 --rate 50000000

# Tichý režim (nezobrazuje sa žiadny indikátor priebehu)
sync0 send ./project 192.168.1.10:9000 -q
```

### 2. Publikovanie zdieľaných adresárov a sťahovanie z viacerých uzlov

Zverejnite zdieľaný adresár (s názvom `dataset`) na pôvodnom serveri:

```bash
sync0 share dataset ./data --region SHA
```

Vytiahnite zdieľaný adresár na iných počítačoch (môžete vyplniť viacero adries uzlov naraz, oddelených čiarkami, aby ste ich stiahli súčasne):

```bash
sync0 pull 10.0.0.1:9000,10.0.0.2:9000 dataset ./local_data
```

Vytiahnite iba určitý podadresár v zdieľanom adresári:

```bash
sync0 pull 10.0.0.1:9000 dataset ./local_data --subpath 2026/logs
```

Preneste lokálne zmeny späť do vzdialeného zdieľaného adresára:

```bash
sync0 push 10.0.0.1:9000 dataset ./local_data
```

### 3. Prihláste sa na odber automatických aktualizácií katalógu

Prihláste sa na odber zdieľaného adresára zdrojového uzla. Keď sa súbor pridá alebo upraví v zdrojovom adresári, automaticky sa zosynchronizuje s lokálnym počítačom. Súčasne bude lokálny počítač automaticky slúžiť ako uzol pre ostatné stroje, ktoré budú ťahať:

```bash
sync0 sub 10.0.0.1:9000 dataset ./local_mirror
```

### 4. Obojsmerná synchronizácia skupín viacerých strojov

Viaceré počítače sa pripájajú k rovnakému názvu skupiny a zmeny v ľubovoľnom adresári uzlov sa automaticky synchronizujú s ostatnými uzlami v skupine:

```bash
sync0 group team-docs ./docs 10.0.0.1:9000,10.0.0.2:9000
```

### 5. Identita uzla a osobná vizitka

Každý uzol podporuje nastavenie používateľského mena, mesta, e-mailu, webovej stránky a personalizovaného podpisu (maximálne 256 znakov) a prechádza lokálnym overením podpisu súkromného kľúča:

```bash
# Zobraziť ID lokálneho verejného kľúča (uveďte konfiguračný adresár, aby sa kľúč zachoval)
sync0 id -c ~/.config/sync0

# Generujte a zobrazujte vizitky uzlov s personalizovanými podpismi
sync0 profile -c ~/.config/sync0 \
  -u alice \
  --region SHA \
  --email alice@example.com \
  --url https://example.com \
  -b "Shanghai backup node"
```

`share`, `pull`, `sub`, `group` je možné spustiť aj s týmito parametrami profilu:

```bash
sync0 share dataset ./data -c ~/.config/sync0 -u alice --region SHA -b "Shanghai backup node"
```

### 6. Prepínanie viacerých jazykov

`sync0` Podporuje 75 jazykov. V predvolenom nastavení sa pomocník príkazového riadka a výzvy v príslušnom jazyku automaticky zobrazujú podľa jazykového prostredia operačného systému; ak systémový jazyk nie je v zozname podporovaných, štandardne sa použije angličtina.

Ak chcete jazyk zadať manuálne, nastavte premennú prostredia `SYNC0_LANG`:

```bash
SYNC0_LANG=en sync0 --help
SYNC0_LANG=ja sync0 --help
SYNC0_LANG=de sync0 --help
```