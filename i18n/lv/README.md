# sync0

`sync0` ir komandrindas rīks failu pārsūtīšanai pa tīkliem, pakāpeniskai direktoriju sinhronizēšanai un datu vienlaicīgai izplatīšanai vairākās iekārtās.

## Sākotnējā dizaina iecere

Mašīnu telpās, valstīs vai vidēs ar tīkla aizkavi un pakešu zudumiem tādi rīki kā `rsync` un `scp` ir balstīti uz vienas savienojuma secības apstiprinājumu, kas ierobežo joslas platuma izmantošanu; ja vairākas mašīnas vienlaikus izvelk datus no viena servera, avota servera tīkla karte var viegli kļūt par vājo vietu.

`sync0` mērķis ir atrisināt divas problēmas:

1. Ļaujiet failu un direktoriju sinhronizācijai starp abām iekārtām darboties ar pilnu fizisko joslas platumu un pārsūtīt mainīto daļu tikai tad, kad fails ir daļēji modificēts.
2. Vairākas mašīnas var pārsūtīt datu blokus viena otrai, sinhronizējot vienu un to pašu direktoriju, samazinot avota servera slodzi un saīsinot kopējo sinhronizācijas laiku.

## Ātruma salīdzinājums ar rsync

Testa vide: Eiropas mezgls $\leftrightarrow$ Ziemeļamerikas mezgls, turp un atpakaļ aizkave 86 ms, mēra visu laikietilpīgo, ieskaitot direktoriju skenēšanu, dublēšanas salīdzināšanu, tīkla pārraidi un diska izvietojuma pārbaudi:

| Pārraides scenārijs | Datu lielums | `sync0` | `rsync` | vairākas |
| :--- | :---: | :---: | :---: | :---: |
| 10 000 mazu failu | 77.6 MB | 0.90 s (688 Mbps) | 4.86 s (128 Mbps) | 5.4x |
| Kodu un arhīvu pakotnes (`.tar`) | 512 MB | 1.94 s (2111 Mbps) | 9.36 s (438 Mbps) | 4.8x |
| Video fails (`.mp4`) | 512 MB | 7.36 s (556 Mbps) | 12.11 s (338 Mbps) | 1.6x |
| Pakāpeniska sinhronizācija (mainīt 1% saturu) | 512 MB | 0.12 s | 1.85 s | 15.4x |

## Darbības uzlabošanas metodes

- **UDP vienlaicīga pārraide**: nosūtiet datu blokus, izmantojot UDP, un atkārtoti pārsūtiet tikai zaudētos fragmentus, lai izvairītos no vienas paketes zuduma, kas bloķē turpmāko failu pārsūtīšanu.
- **Satura segmentēšana un dublēšanas atcelšana**: sadaliet failu datu blokos atbilstoši saturam un aprēķiniet pārbaudes vērtību. Tas pats saturs, kas jau atrodas saņēmējā, tiek atkārtoti izmantots tieši lokāli, nedublējot tīklu.
- **Automātiski pielāgot saspiešanu atbilstoši tīkla ātrumam**: ieslēdziet saspiešanu saspiežamam saturam, piemēram, tekstam un kodam; automātiski izlaidiet saspiešanu videoklipiem, attēliem, saspiestām pakotnēm vai ātrdarbīgā LAN, lai samazinātu CPU patēriņu.
- **Vairāku mezglu izvilkšana blakus**: ja vairākas iekārtas izvelk vienu un to pašu direktoriju, datu bloki tiks izvilkti vienlaikus no lejupielādētajiem mezgliem tajā pašā pilsētā vai ar mazāku latentumu.

## Instalējiet

### 1. metode: instalējiet, izmantojot kravu

```bash
cargo install sync0
```

### 2. metode: lejupielādējiet iepriekš kompilētus bināros failus

Dodieties uz [GitHub Releases](https://github.com/webc-site/sync0/releases), lai lejupielādētu `.tar.xz` saspiesto pakotni attiecīgajai sistēmai (atbalsta Linux x86_64/ARM64, macOS Apple Silicon/Intel, Windows x64/ARM64, Android ARM64/x86_64, FreeBSD, NetBSD). Pēc izsaiņošanas ievietojiet `sync0` `PATH` direktorijā:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Lietotāja rokasgrāmata

### 1. Tieši pārsūtiet failus vai direktorijus starp divām iekārtām

Sāciet uzraudzību saņēmējā mašīnā un norādiet saņemšanas un saglabāšanas ceļu:

```bash
sync0 recv 0.0.0.0:9000 ./backup
```

Nosūtiet vienu failu vai visu direktoriju no sūtīšanas iekārtas:

```bash
sync0 send ./project 192.168.1.10:9000
```

Parasti izmantotie parametri:

```bash
# Dzēst liekos failus, kas pastāv saņēmējā, bet ne sūtīšanas pusē (spoguļa sinhronizācija)
sync0 send ./project 192.168.1.10:9000 --delete

# Sūtīšanas ātruma ierobežojums (vienība: baiti/sekundē, piemēram, 50 MB/s)
sync0 send ./project 192.168.1.10:9000 --rate 50000000

# Klusais režīms (nav rādīta progresa josla)
sync0 send ./project 192.168.1.10:9000 -q
```

### 2. Kopīgotu direktoriju publicēšana un izvilkšana no vairākiem mezgliem

Publicējiet koplietotu direktoriju (ar nosaukumu `dataset`) sākotnējā serverī:

```bash
sync0 share dataset ./data --region SHA
```

Izvelciet koplietoto direktoriju citās iekārtās (varat vienlaikus ievadīt vairākas mezglu adreses, atdalot tās ar komatiem, lai vilktu vienlaicīgi):

```bash
sync0 pull 10.0.0.1:9000,10.0.0.2:9000 dataset ./local_data
```

Koplietotajā direktorijā izvelciet tikai noteiktu apakšdirektoriju:

```bash
sync0 pull 10.0.0.1:9000 dataset ./local_data --subpath 2026/logs
```

Nosūtiet vietējās izmaiņas atpakaļ uz attālo koplietoto direktoriju:

```bash
sync0 push 10.0.0.1:9000 dataset ./local_data
```

### 3. Abonējiet kataloga automātiskos atjauninājumus

Abonējiet avota mezgla koplietoto direktoriju. Kad fails tiek pievienots vai pārveidots avota direktorijā, tas tiks automātiski sinhronizēts ar vietējo datoru. Tajā pašā laikā vietējā iekārta automātiski kalpos kā mezgls citām iekārtām, kas var vilkt:

```bash
sync0 sub 10.0.0.1:9000 dataset ./local_mirror
```

### 4. Divvirzienu vairāku mašīnu grupu sinhronizācija

Vairākas mašīnas pievienojas vienam un tam pašam grupas nosaukumam, un izmaiņas jebkurā mezgla direktorijā tiks automātiski sinhronizētas ar citiem grupas mezgliem:

```bash
sync0 group team-docs ./docs 10.0.0.1:9000,10.0.0.2:9000
```

### 5. Mezgla identitāte un personīgā vizītkarte

Katrs mezgls atbalsta lietotājvārda, pilsētas, e-pasta, vietnes un personalizēta paraksta iestatīšanu (maksimums 256 rakstzīmes) un nodod vietējās privātās atslēgas paraksta autentifikāciju:

```bash
# Skatiet vietējās publiskās atslēgas ID (norādiet konfigurācijas direktoriju, lai atslēga tiktu saglabāta)
sync0 id -c ~/.config/sync0

# Ģenerējiet un parādiet mezglu vizītkartes ar personalizētiem parakstiem
sync0 profile -c ~/.config/sync0 \
  -u alice \
  --region SHA \
  --email alice@example.com \
  --url https://example.com \
  -b "Shanghai backup node"
```

`share`, `pull`, `sub`, `group` var palaist arī ar šiem profila parametriem:

```bash
sync0 share dataset ./data -c ~/.config/sync0 -u alice --region SHA -b "Shanghai backup node"
```

### 6. Vairāku valodu pārslēgšana

`sync0` Atbalsta 75 valodas. Pēc noklusējuma komandrindas palīdzība un uzvednes attiecīgajā valodā tiek parādītas automātiski atbilstoši operētājsistēmas valodas videi; ja sistēmas valoda nav atbalstītajā sarakstā, pēc noklusējuma tiek izmantota angļu valoda.

Lai manuāli norādītu valodu, iestatiet vides mainīgo `SYNC0_LANG`:

```bash
SYNC0_LANG=en sync0 --help
SYNC0_LANG=ja sync0 --help
SYNC0_LANG=de sync0 --help
```