# sync0

`sync0` yra lygiavertis (P2P) failų bendrinimo ir kelių įrenginių sinchronizavimo įrankis, konkuruojantis su Resilio Sync.

Kiekvienas įrenginys yra ir serveris, ir klientas: bet kuris mazgas gali paskelbti kelias dalijimosi failais grupes. Prisijungę kiti mazgai, jie gali automatiškai gauti galimus pradinius mazgus iš lygiaverčio serverio ir vienu metu paimti juos iš kelių šaltinių. Kai sinchronizavimas bus baigtas, bus automatiškai sukurtos pradinės dalys daugiau mazgų, kad būtų pagreitintas atsisiuntimas.

## Pagrindiniai akcentai

- **Kiekvienas yra ir serveris, ir klientas**: panašus į Resilio Sync, nėra centrinio serverio priklausomybės. Bet kuris mazgas gali sukurti kelias dalijimosi failais grupes, taip pat gali prisijungti prie kitų žmonių bendrinimo grupių; mazgai automatiškai keičiasi ir gauna galimus pradinius mazgus, kad būtų galima vienu metu traukti ir sinchronizuoti kelias mašinas realiuoju laiku.
- **Ne IP mazgo adresas**: pagal numatytuosius nustatymus mazgo raktas ir `base64url` užkoduotas `iroh` adresas automatiškai generuojami `~/.config/sync0`, nereikia atsiminti ar atskleisti viešo IP.
- **Kelių pasirinkimų sinchronizavimas pagal poreikį**: palaiko visos bendrinimo grupės sinchronizavimą, taip pat palaiko sinchronizavimą pagal poreikį vienu metu pasirenkant kelis failus ar pakatalogius.
- **Grupės narių hierarchiniai leidimai ir įkėlimo katalogo valdymas**: palaiko tik skaitymo (`ro`), skaitymo ir rašymo įkėlimą (`rw`) ir valdymo (`admin`) leidimus ir gali individualiai nurodyti kelio priešdėlį, kurį kiekvienam nariui leidžiama įkelti (pagal numatytuosius nustatymus leidžiama įkelti į bendrinimo šakninį katalogą).
- **Viso greičio ir laipsniškas sinchronizavimas tinkluose**: didelių failų perdavimas vienu metu, didelių mažų failų kaupimas atmintyje ir tik sinchronizuoti duomenų blokai, kai failai yra iš dalies modifikuoti.

## Greičio palyginimas su rsync

### Transokeaninio tinklo vieno šaltinio faktinis matavimas (Kinija → Jungtinės Valstijos, delsa 260 ms)

| Perdavimo scenarijus | Duomenų dydis | modelis | `sync0` | `rsync -az` | Padidinti daugiklį |
| :--- | :---: | :---: | :---: | :---: | :---: |
| 500 mažų failų | 1.91 MB | Pilnas sinchronizavimas | 18.84 Mbps | 5.32 Mbps | 3.5x |
| 500 mažų failų | 1.91 MB | Laipsniškas sinchronizavimas | 27.15 Mbps | 3.83 Mbps | 7.1x |
| Kodo ir archyvo paketai (`.tar`) | 10.0 MB | Pilnas sinchronizavimas | 139.70 Mbps | 24.97 Mbps | 5.6x |
| Kodo ir archyvo paketai (`.tar`) | 10.0 MB | Laipsniškas sinchronizavimas | 83.38 Mbps | 26.02 Mbps | 3.2x |
| Vaizdo įrašo failas (`.mp4`) | 15.0 MB | Pilnas sinchronizavimas | 54.06 Mbps | 27.04 Mbps | 2.0x |
| Vaizdo įrašo failas (`.mp4`) | 15.0 MB | Laipsniškas sinchronizavimas | 216.83 Mbps | 21.51 Mbps | 10.1x |

### Transokeaninio tinklo P2P kelių mazgų traukos testas (Kinija → JAV du mazgai, delsa 260 ms)

Išmatuotas kelių šaltinių sėklinio mazgo dryžuoto lygiagrečiojo traukimo palyginimas:

| Perdavimo scenarijus | Duomenų dydis | Vieno mazgo sėjimas | Dviejų mazgų P2P sėjimas vienu metu | Padidinti daugiklį |
| :--- | :---: | :---: | :---: | :---: |
| 500 mažų failų | 1.97 MB | 5.28 Mbps | 7.87 Mbps | 1.5x |
| Kodo ir archyvo paketai (`.tar`) | 30.0 MB | 56.35 Mbps | 83.17 Mbps | 1.5x |
| Vaizdo įrašo failas (`.mp4`) | 30.0 MB | 27.00 Mbps | 39.46 Mbps | 1.5x |

### Faktinis tarpžemyninės didelės spartos specialiosios linijos matavimas (Europa → Šiaurės Amerika, vėlavimas 86 ms)

| Perdavimo scenarijus | Duomenų dydis | `sync0` | `rsync` | Padidinti daugiklį |
| :--- | :---: | :---: | :---: | :---: |
| 10 000 mažų failų | 77.6 MB | 688 Mbps | 128 Mbps | 5.4x |
| Kodo ir archyvo paketai (`.tar`) | 512 MB | 2111 Mbps | 438 Mbps | 4.8x |
| Vaizdo įrašo failas (`.mp4`) | 512 MB | 556 Mbps | 338 Mbps | 1.6x |
| Laipsniškas sinchronizavimas (keisti 1 % turinį) | 512 MB | 34133 Mbps | 2214 Mbps | 15.4x |

## Įdiegti

### 1 būdas: įdiekite per „Cargo“.

```bash
cargo install sync0
```

### 2 būdas: atsisiųskite iš anksto sukompiliuotus dvejetainius failus

Eikite į [GitHub Releases] (/0), kad atsisiųstumėte `.tar.xz` suglaudintą atitinkamos sistemos paketą (palaiko Linux, macOS, Windows, Android, FreeBSD, NetBSD), išpakuokite ir įdėkite į `PATH` katalogą:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Greitai pradėkite

`sync0` pagal numatytuosius nustatymus naudoja `~/.config/sync0` kaip konfigūracijos katalogą (jei reikia perjungti į kitus katalogus, galite pridėti `-c <katalogas>`).

### 1. Peržiūrėkite vietinio mazgo adresą ir nustatymo vizitinę kortelę

```bash
# Peržiūrėkite vietinio mazgo adresą (raktas pirmą kartą automatiškai sugeneruojamas ~/.config/sync0)
sync0 id

# Sukurkite asmeninę mazgo vizitinę kortelę (palaiko vartotojo vardą, miestą, el. paštą, svetainę ir parašą per 256 žodžius)
sync0 profile -u alice --region SHA --email alice@example.com --url https://example.com -b "Shanghai node"
```

### 2. Paskelbkite kelias failų bendrinimo grupes ir nustatykite grupės narių leidimus (`.nt` konfigūracija)

`sync0` sukonfigūruotas [NestedText](https://nestedtext.org) (`.nt`) formatu. Kiekvienas vartotojas turi atskirą `.nt` konfigūracijos failą. Vartotojo profilio katalogą galite nurodyti per `users_dir` `config.nt` (kuris automatiškai apima visus `.nt` šiame kataloge esančius failus). Tiek naudotojas, tiek kelio priešdėlis, kurį leidžiama įkelti, naudoja netvarkingą sąrašą (`-`). **Kai įkėlimo kelio priešdėlis neužpildytas, pagal numatytuosius nustatymus leidžiama įkelti į pagrindinį bendrinimo katalogą**.

1. Sukurkite atskirus `.nt` konfigūracijos failus kiekvienam nariui `~/.config/sync0/users/` kataloge:

`~/.config/sync0/users/alice.nt` (tik skaitomas narys, skaičius ribotas `80MB`, `rate` palaiko `GB`, `MB`, `KB` vienetus):

```yaml
id: 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c
perm: ro
rate: 80MB
```

`~/.config/sync0/users/bob.nt` (riboto įkėlimo nariai: leidžiama įkelti tik į `uploads/bob` ir `shared/common` katalogus):

```yaml
id: 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f
perm: rw
write_prefix:
  - uploads/bob
  - shared/common
```

`~/.config/sync0/users/charlie.nt` (numatytasis nario įkėlimas: kai `write_prefix` praleista, įkelti į pagrindinį bendrinimo katalogą leidžiama pagal numatytuosius nustatymus):

```yaml
id: 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
perm: rw
```

2. Sukonfigūruokite naudotojo katalogą ir kelias dalijimosi failais grupes `~/.config/sync0/config.nt`:

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

Pradėti bendrinimo paslaugą:

```bash
# Tiesioginis paleidimas (automatiškai įkelia visas .nt vartotojo konfigūracijas ~/.config/sync0/config.nt ir user_dir kataloguose)
sync0 share --region SHA

# Arba greitai paskelbkite kelias bendrinimo grupes ir įgalioti grupės narius tiesiogiai naudodami komandinę eilutę
sync0 share dataset:./data team-vault:./vault \
  --auth \
  --allow-ro 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c \
  --allow-rw 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f:uploads/bob,shared/common \
  --allow-rw 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
```

### 3. Ištraukite visą turinį arba pasirinkite kelis failus ir katalogus (`sync0 pull`)

Prisijunkite prie bet kurio lygiaverčio mazgo, kad automatiškai gautumėte visus galimus pradinius mazgus bendrinimo grupėje ir vienu metu gautumėte iš kelių pradinių mazgų:

```bash
# Ištraukite visą duomenų rinkinio bendrinimo grupę į vietinį ./local_data
sync0 pull dataset:// ./local_data -p <mazgo adresas>

# Kelių pasirinkimų sinchronizavimas: vienu metu ištraukite vieną failą docs/readme.md ir pakatalogius aktyvai/icons, src/core
sync0 pull dataset://docs/readme.md,assets/icons,src/core ./local_data -p <mazgo adresas>
```

### 4. Įkelti failą arba katalogą į bendrinimo grupę (`sync0 push`)

Nariai, turintys įkėlimo leidimus (`rw` arba `admin`), gali įkelti vietinius failus arba katalogus į nuotolinio bendrinimo grupes:

```bash
# Nariai, turintys šakninio katalogo įkėlimo leidimą: įkelkite tiesiai į bendrinamą šakninį katalogą
sync0 push team-vault:// ./new_release -p <mazgo adresas>

# Tik nariai, įgalioti naudoti konkretų priešdėlį (pvz., įkėlimai / bob): įkelkite į leidžiamą kelio priešdėlį
sync0 push team-vault://uploads/bob/report.pdf ./report.pdf -p <mazgo adresas>
```

### 5. Prenumeruoti automatinį sinchronizavimą ir sėjimą (`sync0 sub`)

Užsiprenumeravus tam tikro mazgo bendrinimo grupę, vietinis serveris yra ir priimantis klientas, ir naujas siuntimo serveris; kai šaltinio mazgo failas atnaujinamas, jis automatiškai sinchronizuojamas ir pateikia gabalų atsisiuntimus kitiems mazgams:

```bash
# Prenumeruokite visą bendrinimo grupę ir automatiškai atsisiųskite
sync0 sub dataset:// ./local_mirror -p <mazgo adresas>

# Jei norite užsiprenumeruoti nurodytus failus ir pakatalogius ir juos pradėti, pasirinkite tik kelis kartus
sync0 antrinis duomenų rinkinys://models/llama.onnx,configs ./local_mirror -p <mazgo adresas>
```

### 6. Kelių įrenginių grupės lygiaverčių programų sinchronizavimas (`sync0 group`)

Keli įrenginiai prisijungia prie tos pačios bendrinimo grupės, viena kita automatiškai atranda galimus pradinius mazgus ir palaiko dvipusį sinchronizavimą (palaiko visišką sinchronizavimą arba daugkartinį failų ir katalogų pasirinkimą):

```bash
# Visiškai prisijungti prie grupės ir dvipusis sinchronizavimas
sync0 group team-docs:// ./docs -p <mazgo adresas>

# Tik wal katalogas ir meta.json failas kelių pasirinkimų sinchronizavimo grupėje
sync0 group team-docs://wal,meta.json ./docs -p <mazgo adresas>
```

### 7. Tiesioginis perdavimas iš taško į tašką tarp dviejų mašinų (`send` / `recv`)

Nereikia iš anksto konfigūruoti grupės ir galite laikinai perkelti failus arba katalogus tiesiai tarp dviejų įrenginių:

```bash
# Imtuvo stebėjimas
sync0 recv 0.0.0.0:9000 ./backup

# Siuntimo galas tiesiogiai perkelia failus arba katalogus (neprivaloma --delete veidrodis, kad išvalytumėte perteklinius failus, --rate greičio apribojimas)
sync0 send ./project 192.168.1.10:9000
```

### 8. Kelių kalbų perjungimas

`sync0` palaiko 75 kalbas ir automatiškai persijungia pagal sistemos kalbos aplinką pagal numatytuosius nustatymus; jį taip pat galima nurodyti rankiniu būdu per `SYNC0_LANG`:

```bash
SYNC0_LANG=en sync0 --help
SYNC0_LANG=ja sync0 --help
SYNC0_LANG=de sync0 --help
```