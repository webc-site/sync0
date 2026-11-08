# sync0

`sync0` je peer-to-peer (P2P) nástroj na zdieľanie súborov a synchronizáciu medzi viacerými počítačmi, ktorý konkuruje Resilio Sync.

Každý počítač je server aj klient: každý uzol môže publikovať viacero skupín zdieľania súborov. Po pripojení ďalších uzlov môžu automaticky získať dostupné počiatočné uzly z partnerského servera a stiahnuť ich súčasne z viacerých zdrojov. Po dokončení synchronizácie sa automaticky vytvoria semená pre viac uzlov, aby sa urýchlilo sťahovanie.

## Hlavné prvky

- **Každý je server aj klient**: Porovnateľné s Resilio Sync, žiadna závislosť od centrálneho servera. Každý uzol môže vytvoriť viacero skupín zdieľania súborov a môže sa tiež pripojiť k skupinám zdieľania iných ľudí; uzly si automaticky vymieňajú a získavajú dostupné počiatočné uzly, aby sa dosiahlo súčasné ťahanie a synchronizácia viacerých strojov v reálnom čase.
- **Adresa uzla bez IP**: V predvolenom nastavení sa kľúč uzla a `base64url` zakódovaná adresa `iroh` automaticky generujú v `~/.config/sync0` bez toho, aby ste si museli pamätať alebo odhaľovať verejnú IP.
- **Synchronizácia s viacerými výbermi na požiadanie**: Podporuje synchronizáciu celej skupiny zdieľania a tiež podporuje synchronizáciu na požiadanie výberom viacerých súborov alebo podadresárov súčasne.
- **Hierarchické oprávnenia členov skupiny a ovládanie adresára na odovzdávanie**: Podporuje oprávnenia len na čítanie (`ro`), čítanie a zápis (`rw`) a správu (`admin`) a môže individuálne určiť predponu cesty, ktorú môže každý člen nahrať (v predvolenom nastavení je nahrávanie do koreňového adresára zdieľanej zložky povolené).
- **Plnorýchlostná a prírastková synchronizácia naprieč sieťami**: Súčasný prenos veľkých súborov, agregácia veľkých malých súborov v pamäti a synchronizované dátové bloky iba pri čiastočnej úprave súborov.

## Porovnanie rýchlosti s rsync

### Skutočné meranie z jedného zdroja v transoceánskej sieti (Čína → Spojené štáty americké, oneskorenie 260 ms)

| Scenár prenosu | Veľkosť údajov | model | `sync0` | `rsync -az` | Zvýšte multiplikátor |
| :--- | :---: | :---: | :---: | :---: | :---: |
| 500 malých súborov | 1.91 MB | Úplná synchronizácia | 18.84 Mbps | 5.32 Mbps | 3.5x |
| 500 malých súborov | 1.91 MB | Prírastková synchronizácia | 27.15 Mbps | 3.83 Mbps | 7.1x |
| Kódové a archivačné balíčky (`.tar`) | 10.0 MB | Úplná synchronizácia | 139.70 Mbps | 24.97 Mbps | 5.6x |
| Kódové a archivačné balíčky (`.tar`) | 10.0 MB | Prírastková synchronizácia | 83.38 Mbps | 26.02 Mbps | 3.2x |
| Video súbor (`.mp4`) | 15.0 MB | Úplná synchronizácia | 54.06 Mbps | 27.04 Mbps | 2.0x |
| Video súbor (`.mp4`) | 15.0 MB | Prírastková synchronizácia | 216.83 Mbps | 21.51 Mbps | 10.1x |

### P2P multiuzlový ťahový test transoceánskej siete (Čína → Spojené štáty duálne uzly, oneskorenie 260 ms)

Namerané porovnanie viaczdrojového seedného uzla pruhovaného súbežného ťahania:

| Scenár prenosu | Veľkosť údajov | Výsadba jedného uzla | Súbežné seedovanie P2P s dvoma uzlami | Zvýšte multiplikátor |
| :--- | :---: | :---: | :---: | :---: |
| 500 malých súborov | 1.97 MB | 5.28 Mbps | 7.87 Mbps | 1.5x |
| Kódové a archivačné balíčky (`.tar`) | 30.0 MB | 56.35 Mbps | 83.17 Mbps | 1.5x |
| Video súbor (`.mp4`) | 30.0 MB | 27.00 Mbps | 39.46 Mbps | 1.5x |

### Aktuálne meranie transkontinentálnej vysokorýchlostnej vyhradenej linky (Európa → Severná Amerika, meškanie 86 ms)

| Scenár prenosu | Veľkosť údajov | `sync0` | `rsync` | Zvýšte multiplikátor |
| :--- | :---: | :---: | :---: | :---: |
| 10 000 malých súborov | 77.6 MB | 688 Mbps | 128 Mbps | 5.4x |
| Kódové a archivačné balíčky (`.tar`) | 512 MB | 2111 Mbps | 438 Mbps | 4.8x |
| Video súbor (`.mp4`) | 512 MB | 556 Mbps | 338 Mbps | 1.6x |
| Prírastková synchronizácia (upraviť 1% obsah) | 512 MB | 34133 Mbps | 2214 Mbps | 15.4x |

## Inštalovať

### Metóda 1: Inštalácia cez Cargo

```bash
cargo install sync0
```

### Metóda 2: Stiahnite si predkompilované binárne súbory

Prejdite na [GitHub Releases](https://github.com/webc-site/sync0/releases) a stiahnite si `.tar.xz` komprimovaný balík pre príslušný systém (podporuje Linux, macOS, Windows, Android, FreeBSD, NetBSD), rozbaľte ho a vložte do adresára `PATH`:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Začnite rýchlo

`sync0` štandardne používa `~/.config/sync0` ako konfiguračný adresár (ak sa potrebujete prepnúť do iných adresárov, môžete pridať `-c <adresár>`).

### 1. Zobrazte adresu lokálneho uzla a vizitku nastavenia

```bash
# Zobrazenie adresy lokálneho uzla (kľúč sa prvýkrát automaticky vygeneruje v ~/.config/sync0)
sync0 id

# Vygenerujte si vizitku osobného uzla (podporuje meno používateľa, mesto, e-mail, webovú stránku a podpis v rozsahu 256 slov)
sync0 profile -u alice --region SHA --email alice@example.com --url https://example.com -b "Shanghai node"
```

### 2. Publikovanie viacerých skupín zdieľania súborov a nastavenie oprávnení členov skupiny (`.nt` Konfigurácia)

`sync0` je nakonfigurovaný vo formáte [NestedText](https://nestedtext.org) (`.nt`). Každý používateľ má samostatný konfiguračný súbor `.nt`. Adresár užívateľského profilu môžete zadať pomocou `users_dir` v `config.nt` (ktorý automaticky zahŕňa všetky `.nt` súbory v tomto adresári). Používateľ aj predpona cesty, ktorú je možné nahrať, používajú neusporiadaný zoznam (`-`). **Keď nie je vyplnená predpona cesty nahrávania, nahrávanie do koreňového adresára zdieľania je predvolene povolené**.

1. Vytvorte samostatné konfiguračné súbory `.nt` pre každého člena v adresári `~/.config/sync0/users/`:

`~/.config/sync0/users/alice.nt` (člen len na čítanie, obmedzená sadzba `80MB`, `rate` podporuje jednotky `GB`, `MB`, `KB`):

```yaml
id: 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c
perm: ro
rate: 80MB
```

`~/.config/sync0/users/bob.nt` (členovia s obmedzeným odovzdávaním: môžu odovzdávať iba do `uploads/bob` a `shared/common` adresárov):

```yaml
id: 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f
perm: rw
write_prefix:
  - uploads/bob
  - shared/common
```

`~/.config/sync0/users/charlie.nt` (Predvolené odovzdanie člena: Keď je vynechané `write_prefix`, odovzdávanie do koreňového adresára zdieľania je predvolene povolené):

```yaml
id: 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
perm: rw
```

2. Nakonfigurujte používateľský adresár a viacero skupín zdieľania súborov v `~/.config/sync0/config.nt`:

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

Spustiť službu zdieľania:

```bash
# Priame spustenie (automaticky načíta všetky používateľské konfigurácie .nt v adresároch ~/.config/sync0/config.nt a users_dir)
sync0 share --region SHA

# Alebo rýchlo publikujte viacero skupín zdieľania a autorizujte členov skupiny priamo cez príkazový riadok
sync0 share dataset:./data team-vault:./vault \
  --auth \
  --allow-ro 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c \
  --allow-rw 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f:uploads/bob,shared/common \
  --allow-rw 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
```

### 3. Vytiahnite celý obsah alebo vyberte viacero súborov a adresárov (`sync0 pull`)

Pripojte sa k akémukoľvek partnerskému uzlu, aby ste automaticky získali všetky dostupné počiatočné uzly v skupine zdieľania a vytiahli z viacerých počiatočných uzlov súčasne:

```bash
# Presuňte celú skupinu zdieľania množiny údajov do lokálneho ./local_data
sync0 pull dataset:// ./local_data -p <adresa uzla>

# Synchronizácia s viacerými možnosťami: súčasne vytiahnite jeden súbor docs/readme.md a podadresáre aktíva/ikony, src/core
sync0 pull dataset://docs/readme.md,assets/icons,src/core ./local_data -p <adresa uzla>
```

### 4. Nahrajte súbor alebo adresár do skupiny zdieľania (`sync0 push`)

Členovia s oprávneniami na odovzdávanie (`rw` alebo `admin`) môžu odovzdávať miestne súbory alebo adresáre do skupín vzdialeného zdieľania:

```bash
# Členovia s povolením na nahrávanie do koreňového adresára: Nahrávajte priamo do zdieľaného koreňového adresára
sync0 push team-vault:// ./new_release -p <adresa uzla>

# Iba členovia, ktorí majú oprávnenie na konkrétnu predponu (ako napr. uploads/bob): upload na povolenú predponu cesty
sync0 push team-vault://uploads/bob/report.pdf ./report.pdf -p <adresa uzla>
```

### 5. Prihlásiť sa na odber automatickej synchronizácie a seedovania (`sync0 sub`)

Po prihlásení do skupiny zdieľania určitého uzla je lokálny server prijímajúcim klientom aj novým seedovacím serverom; keď je súbor zdrojového uzla aktualizovaný, automaticky sa synchronizuje a poskytuje časti sťahovania pre iné uzly:

```bash
# Prihláste sa na odber celej skupiny zdieľania a automaticky sa naskladajte
sync0 sub dataset:// ./local_mirror -p <adresa uzla>

# Iba viacnásobným výberom sa prihlásite na odber konkrétnych súborov a podadresárov a nasadíte ich
sync0 sub dataset://models/llama.onnx,configs ./local_mirror -p <adresa uzla>
```

### 6. Synchronizácia skupiny medzi viacerými počítačmi (`sync0 group`)

Viaceré počítače sa pripájajú k rovnakej skupine zdieľania, navzájom si automaticky zisťujú dostupné počiatočné uzly a udržiavajú obojsmernú synchronizáciu (podporuje úplnú synchronizáciu alebo viacnásobný výber súborov a adresárov):

```bash
# Plné pripojenie ku skupine a obojsmerná synchronizácia
skupina sync0 team-docs:// ./docs -p <adresa uzla>

# Iba adresár wal a súbor meta.json v skupine synchronizácie s viacerými výbermi
skupina sync0 team-docs://wal,meta.json ./docs -p <adresa uzla>
```

### 7. Priamy prenos z bodu do bodu medzi dvoma strojmi (`send` / `recv`)

Nie je potrebné vopred konfigurovať skupinu a môžete dočasne prenášať súbory alebo adresáre priamo medzi dvoma počítačmi:

```bash
# Monitorovanie prijímača
sync0 recv 0.0.0.0:9000 ./backup

# Odosielajúci koniec priamo prenáša súbory alebo adresáre (voliteľné --delete mirror na vyčistenie prebytočných súborov, --rate speed limit)
sync0 send ./project 192.168.1.10:9000
```

### 8. Prepínanie viacerých jazykov

`sync0` podporuje 75 jazykov a predvolene sa automaticky prepína podľa jazykového prostredia systému; dá sa zadať aj manuálne pomocou `SYNC0_LANG`:

```bash
SYNC0_LANG=en sync0 --help
SYNC0_LANG=ja sync0 --help
SYNC0_LANG=de sync0 --help
```