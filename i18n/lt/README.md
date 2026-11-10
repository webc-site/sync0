# sync0

`sync0` yra decentralizuotas P2P failų dalijimosi, sinchronizavimo realiuoju laiku ir kelių šaltinių paskirstymo įrankis, naudojamas pakeisti `rsync` – išmatuotas transkontinentinio plataus tinklo greitis yra iki **6,95×** iš `rsync` (vieno taško** geometrinis vidutinis greitis **2,01×3 ir 3 vidutinis greitis **2,01×**, P.2). didžiulių mažų failų sinchronizavimas visame vandenyne yra toks pat greitas kaip **7,19 ×**.

Kiekviena mašina yra ir serveris, ir klientas: nereikia centrinio serverio ar viešojo tinklo adreso, automatinis skylių vėrimas pagrįstas paskirstyto tinklo viešojo rakto adresu ir palaiko tiesioginį perdavimą tarp dviejų mašinų, kelių bendrinimo grupių leidimų prieglobą, kelis pakatalogių pasirinkimą pagal poreikį, milisekundžių lygio realiojo laiko laipsnišką sinchronizavimą ir P2P kelių šaltinių vienu metu.

## Įdiegti

### 1 būdas: krovinio montavimas

```bash
cargo install sync0
```

### 2 būdas: atsisiųskite iš anksto sukompiliuotus dvejetainius failus

Eikite į [GitHub Releases] (/0), kad atsisiųstumėte `.tar.xz` suglaudintą paketą, atitinkantį sistemą (palaikoma Linux, macOS, Windows, Android, iOS, FreeBSD), išpakuokite ir įdėkite į `PATH`:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Greitai pradėkite

### 1. Nulinės konfigūracijos tiesioginis perdavimas tarp dviejų mašinų (`send` / `recv`)

Nereikia konfigūruoti bendrinimo grupės, tiesiogiai perkelti failus ar katalogus tarp dviejų mašinų dideliu greičiu:

```bash
# Priėmimo galas klauso prievado ir rašo į ./backup
sync0 recv 0.0.0.0:9000 ./backup

# Tiesioginis perdavimas iš siuntėjo (pasirinktinai – ištrinkite veidrodį, kad išvalytumėte perteklinius failus, – 50 MB greičio apribojimas)
sync0 send ./project 192.168.1.10:9000
```

### 2. Įrašų bendrinimas ir pareikalavus ištraukimas / įkėlimas (`share` / `pull` / `push`)

```bash
# Serveris: paskelbkite bendrinimo grupę naudodami vieną komandų eilutę (po paleidimo išveskite iroh mazgo adresą be IP)
sync0 share dataset:./data

# Klientas: perkelkite visą duomenų rinkinio bendrinimo grupę į vietinį ./local_data
sync0 pull dataset:// ./local_data -p <mazgo adresas>

# Klientas: pareikalavus ištraukite kelis pasirinkimus (sinchronizuoti tik nurodytus atskirus failus ir pakatalogius)
sync0 pull dataset://docs/readme.md,assets/icons,src/core ./local_data -p <mazgo adresas>

# Klientas: įkelkite failus arba katalogus į įgaliotas bendrinimo grupes
sync0 push dataset://releases/v1 ./build_output -p <mazgo adresas>
```

### 3. Prenumeratos siuntimas realiuoju laiku ir kelių įrenginių dvipusis sinchronizavimas (`sub` / `group`)

Užmezgus ilgą ryšį, jei nuotoliniame gale atsiranda koks nors failas **pridedamas, keičiamas, ištrinamas, keičiamas leidimai ar modifikavimo laikas**, vietinis katalogas bus sinchroniškai atnaujinamas milisekundėmis realiuoju laiku (nauji / modifikuoti dideli failai tik ištrauks skirtumus ir atnaujins juos vietoje, ištrynus failus bus automatiškai išvalomi vietiniai failai ir tušti katalogai ir tik keičiami leidimai):

- **`sync0 sub`(Vienpusis atspindėjimas realiuoju laiku + automatinis sėjimas)**: vietinis realusis laikas seka bendrinimo pabaigos papildymus, ištrynimus ir modifikacijas ir automatiškai veikia kaip P2P pradinis šaltinis, kad tuo pačiu metu paspartėtų kiti traukiantys mazgai.
- **`sync0 group`(Kelių mašinų lygiavertis dviejų krypčių sinchronizavimas realiuoju laiku)**: jei failai pridedami, ištrinami arba modifikuojami bet kuriame grupės kompiuteryje, visi kiti internetiniai mazgai bus atnaujinami sinchroniškai realiuoju laiku ir bus naudojami kaip vienas kito pradmenys.

```bash
# Prenumeruokite visą bendrinimo grupę: nuotoliniai papildymai, ištrynimai ir pakeitimai sinchronizuojami vietoje realiuoju laiku ir automatiškai įtraukiami
sync0 sub dataset:// ./local_mirror -p <mazgo adresas>

# Keleto pasirinkimų prenumerata pagal pareikalavimą: sinchronizuokite ir realiuoju laiku atsisiųskite tik nurodytus atskirus failus ir pakatalogius
sync0 antrinis duomenų rinkinys://models/llama.onnx,configs ./local_mirror -p <mazgo adresas>

# Kelių mašinų lygiaverčių grupių dvipusis sinchronizavimas realiuoju laiku (taip pat palaiko visą arba kablelio kelių pasirinkimų kelius)
sync0 group team-docs:// ./docs -p <mazgo adresas>
```

### 4. Komandos kelių bendrinimo grupės ir narių leidimų konfigūracija (`.nt` konfigūracija)

Numatytasis konfigūracijos katalogas yra `~/.config/sync0` (galima perjungti naudojant `-c <directory>`), naudojant minimalistinį [NestedText](https://nestedtext.org) (`.nt`) formatą:
- **Palaiko skaitymo viešajame tinkle ribojimą (skaityti gali tik įgalioti vartotojai)**: kol bendrinimo grupėje sukonfigūruotas `users:` narių sąrašas (arba komandų eilutėje nurodoma `--allow-r` / `--allow-rw` / `--allow-w` / `--auth`), sistema automatiškai įjungia ** **id =" ir ne** viešąjį autentifikavimą (">) surašymas arba skaitymas, o P2P prenumeruojant sėjimo mazgus taip pat vienu metu bus paveldimas baltasis sąrašas, todėl skaityti galės tik įgalioti vartotojai; bendrinimo grupės, kurios nesukonfigūruotos naudojant `users`, pagal numatytuosius nustatymus yra viešos ir tik skaitomos (jei norite būti viešos ir tik skaitomos ir įkelti tik nurodytiems nariams, galite aiškiai nustatyti `auth: false`).
- **Smulkūs skaitymo-rašymo ir pakatalogio pateikimo leidimai**: pagal numatytuosius nustatymus nariai yra tik skaitomi (`r` nebūtina aiškiai rašyti) ir palaiko `r` (tik skaitoma), `w` (rašoma tik į išskleidžiamąjį laukelį, neskaitoma), `rw` ir `rw` (tiesioginis fiksavimas) įkėlimui).

**Nario konfigūracijos pavyzdys (`~/.config/sync0/users/`):**

```yaml
# ~/.config/sync0/users/alice.nt (numatytasis tik skaitymas, greičio apribojimas 80 MB)
id: 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c
rate: 80MB
```

```yaml
# ~/.config/sync0/users/bob.nt (skaitymo ir rašymo leidimai, leidžiama tik įkelti į nurodytą pakatalogį)
id: 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f
perm: rw
write_prefix:
  - uploads/bob
  - shared/common
```

```yaml
# ~/.config/sync0/users/charlie.nt (tik rašymo pristatymo leidimas, skaityti draudžiama)
id: 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
perm: w
```

**Bendrinimo grupės konfigūracijos pavyzdys (`~/.config/sync0/config.nt`):**

```yaml
users_dir: ./users

groups:
  # Nesukonfigūruoti vartotojai: numatytasis viešasis tinklas yra tik skaitomas
  dataset:
    path: ./data
    rate: 50MB

  # Sukonfigūruoti vartotojai: automatiškai apribokite skaitymą viešajame tinkle (skaityti gali tik Alisa ir Bobas, Čarlis gali tik pristatyti ir įkelti)
  team-vault:
    path: ./vault
    users:
      - alice
      - bob
      - charlie
```

Paleiskite paslaugą (įkelkite konfigūracijos failą arba tiesiogiai naudokite komandinės eilutės parametrus, kad apribotumėte skaitymą viešajame tinkle ir įgaliotumėte narius):

```bash
# Norėdami pradėti, įkelkite ~/.config/sync0/config.nt
sync0 share --region SHA

# Arba paskelbkite privačią bendrinimo grupę naudodami komandinę eilutę (nurodyti --allow-*, kad automatiškai apribotumėte skaitymą viešajame tinkle ir tik įgalioti vartotojai galėtų ją pasiekti)
sync0 share team-vault:./vault \
  --allow-r 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c \
  --allow-rw 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f:uploads/bob,shared/common \
  --allow-w 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
```

### 5. Mazgo adresas, vizitinė kortelė ir kelios kalbos

```bash
# Peržiūrėkite vietinį mazgo adresą be IP (raktas automatiškai sugeneruojamas pirmą kartą)
sync0 id

# Sukurkite Ed25519 savarankiškai pasirašytą mazgo vizitinę kortelę (slapyvardį, miesto kodą, el. paštą, svetainę, įžangą)
sync0 profile -u alice --region SHA --email alice@example.com -b "Shanghai node"

# Yra 75 integruotos kalbos, kurios pagal numatytuosius nustatymus atitinka sistemos kalbą ir jas galima perjungti naudojant aplinkos kintamuosius.
SYNC0_LANG=en sync0 --help
```

## Našumas ir pagrindinės savybės

### Faktinis tarpžemyninis WAN matavimas (palyginimas `rsync`)

Faktinė matavimo aplinka: Jungtinės Valstijos `c1` → Europa `n1` (RTT 65 ms), duomenų rinkinys yra Linux branduolys `linux-7.3.3`:

#### 1) A la carte režimas prieš `rsync`

| Duomenų rinkinys | dydis | `sync0` Vienas taškas<br>(Mbps) | `rsync`<br>(Mbps) | Paspartinimas (`sync0` / `rsync`) |
| :--- | ---: | ---: | ---: | ---: |
| `linux-7.3.3.tar.xz`(nesuglaudinamas) | 155 MB | **121.07** | 107.89 | **1,12×** |
| `linux-7.3.3.tar`(suglaudinamas) | 1.6 GB | **442.07** | 506.62 | **0,87×** |
| `linux-7.3.3.tar`(vietinis pakeitimo prieaugis) | 1.6 GB | **8,123.87** | 1,169.49 | **6,95×** |
| `linux-7.3.3/` (95 979 maži failai) | 1.5 GB | **565.49** | 235.27 | **2,40×** |
| **geometrinis vidurkis** | — | **704.17** | 350.19 | **2.01×** |

#### 2) P2P dviejų šaltinių režimas ir `rsync`

| Duomenų rinkinys | dydis | `sync0` P2P dvigubas šaltinis<br>(Mbps) | `rsync`<br>(Mbps) | Paspartinimas (`sync0` / `rsync`) |
| :--- | ---: | ---: | ---: | ---: |
| `linux-7.3.3.tar.xz`(nesuglaudinamas) | 155 MB | **466.85** | 107.89 | **4,33×** |
| `linux-7.3.3.tar`(suglaudinamas) | 1.6 GB | **992.17** | 506.62 | **1,96×** |
| `linux-7.3.3.tar`(vietinis pakeitimo prieaugis) | 1.6 GB | **5,666.84** | 1,169.49 | **4,85×** |
| `linux-7.3.3/` (95 979 maži failai) | 1.5 GB | **703.30** | 235.27 | **2,99×** |
| **geometrinis vidurkis** | — | **1,165.63** | 350.19 | **3,33×** |

> Pastaba: P2P dviejų šaltinių režimu Europos mazgai `s1` bendradarbiauja sėjant, o tarpžemyninio pagrindo RTT yra 65 ms.

### Pagrindinės produkto savybės

- **Viešojo rakto adresų automatinis skylių pramušimas**: Nereikia centrinio serverio ir viešojo tinklo adreso, automatinis NAT skylės perforavimas pagal paskirstyto tinklo viešojo rakto adresą.
- **Milisekundžių lygio sinchronizavimas realiuoju laiku ir metaduomenų žinomumas**: pagrįsta savųjų failų įvykių stebėjimu (`FSEvents` / `inotify` / `ReadDirectoryChangesW`), failų papildymu, trynimu realiuoju laiku, POSIX leidimais (`mode`) ir milisekundžių keitimo laiku (5"> milisekundės = 5); nulinis duomenų bloko perdavimas, kai keičiami tik leidimai arba laikas.
- **BT stiliaus vienalaikis atsisiuntimas ir įkėlimas bei regioninio giminingumo planavimas**: atsisiuntimo mazgui nereikia laukti, kol bus atsiųsta visa suma. Visi blokų intervalai (`Op::Have`), kurie praėjo 128 bitų maišos patikrinimą, nedelsiant užrakinami atmintyje / laikinajame telkinyje, kad kiti mazgai galėtų vienu metu traukti. Kartu su dinamišku PEX mazgų keitimu ir lėtu mazgo pusės prevencija (darbo pavogimas), klasterio pralaidumas yra maksimalus.
- **Apsauga nuo kenkėjiškų mazgų klastojimo ir klasterio apsinuodijimo**: autoritetingų šaltinių sąrašas `root_hash` Savaime nuoseklus užrakinimas + privalomas aparatinės įrangos pagreitis `gxhash128` santraukos patikrinimas prieš įdedant kiekvieną bloką į diską ir pašalinant iš išorės; jei randamas sugadintas blokas, milisekundės lygio grandinės pertraukiklis užtemdys kenkėjišką mazgą ir automatiškai atsigaus iš sąžiningo šaltinio.
- **FastCDC in situ prieaugis ir mažas failų agregavimas**: tik diferencialiniai blokai perduodami daliniam didelių failų modifikavimui ir patalpinami į diską; masinis mažų failų atminties paketas ir priešdėlių diferencinis glaudinimas pašalina kiekvieną failą ir atgal.
- **Savaiminis išgydymas nutrūkus maitinimui ir atnaujinimas po paleidimo iš naujo**: „Redis“ panašus RDB + AOF hibridinis formatas (`.s0_subs.bin`) naudojamas sėjimo būsenai ir failų sąrašui išsaugoti, kasdieniai pakeitimai pridedami mikrosekundėmis ir automatiškai perrašomi bei reguliariai tvarkomi. Iš naujo paleidus serverį, neprisijungus atlikti pakeitimai automatiškai perkeliami į sėjimo mazgą.

## Techninė architektūra ir dokumentacija

- **Dviejų plokštumų atsiejimo protokolas**:
  - **Valdymo ir keitimo realiuoju laiku paviršius (`iroh` QUIC / TCP, pirmojo baito `PROTO_VERSION` versijos rankos paspaudimas) **: atsakingas už Ed25519 vizitinės kortelės patvirtinimą, leidimo patvirtinimą, spiečiaus sėklų aptikimą ir `u64::MAX` lygio masinio pakeitimo srautinio siuntimo paketų palaikymą (realiuoju laiku push 4 id. <b">b = 4 id.) id="6"> / `FsOp::Remove` / `FsOp::Meta` Trys primityvai).
  - **Duomenų paskirstymo paviršius (vietinis UDP + `compio` asinchroninis įvestis/išvestis) **: 20B kompaktiška antraštė, 18B TACK periodinis grįžtamasis ryšys, BBRv3/Copa hibridinis perkrovos valdymas, RACK-TLP paketų praradimo atkūrimas, pirmojo bloko Šenono entropijos adaptyvus Zstd suspaudimo apėjimas="2"> bloko pabaiga-to-end-to.
- **Sužinokite daugiau**:
  - [Našumo matavimo ataskaita ir atkūrimo scenarijus](https://github.com/webc-site/sync0/blob/main/doc/zh/benchmark.md)
  - [Sistemos architektūros projektavimas](https://github.com/webc-site/sync0/blob/main/doc/zh/design_v4.md)
  - [Dvejetainių laidų protokolas ir patvarumo specifikacija](https://github.com/webc-site/sync0/blob/main/doc/zh/proto.md)
  - [Tapatybės autentifikavimas, autoritetų kontrolė ir vientisumo tikrinimas](https://github.com/webc-site/sync0/blob/main/doc/zh/crypto.md)