# sync0

`sync0` ir decentralizēts P2P failu koplietošanas, reāllaika sinhronizācijas un vairāku avotu izplatīšanas rīks, ko izmanto, lai aizstātu `rsync` — izmērītais transkontinentālā WAN ātrums ir līdz `rsync` **5,89 ×** (viena punkta ģeometriskais vidējais ātrums **2,67 × **), ģeometriskais ** vidējais ātrums **2,67 × **, P2P.1. Transokeāna augstas latentuma saites P2P paātrinājums ir līdz **7,68 ×** (Pilnas saites 24 scenārija visaptverošs ģeometriskais vidējais ātrums **2,56 ×**).

Katra iekārta ir gan serveris, gan klients: nav nepieciešams centrālais serveris vai publiska tīkla adrese, automātiskā caurumu noteikšana ir balstīta uz izplatītā tīkla publiskās atslēgas adresi, un tā atbalsta tiešu pārraidi starp divām iekārtām, vairāku koplietošanas grupu atļauju mitināšanu, vairāku apakšdirektoriju atlasi pēc pieprasījuma, milisekundes līmeņa reāllaika pakāpenisku sinhronizāciju un P2P vairāku avotu vienlaicību.

## Instalējiet

### 1. metode: kravas uzstādīšana

```bash
cargo install sync0
```

### 2. metode: lejupielādējiet iepriekš kompilētus bināros failus

Dodieties uz [GitHub Releases](https://github.com/webc-site/sync0/releases), lai lejupielādētu `.tar.xz` saspiesto pakotni, kas atbilst sistēmai (atbalsta Linux, macOS, Windows, Android, iOS, FreeBSD), izpakojiet un ievietojiet `PATH`:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Sāciet ātri

### 1. Nulles konfigurācijas tiešā pārraide starp divām iekārtām (`send` / `recv`)

Nav nepieciešams konfigurēt koplietošanas grupu, tieši pārsūtiet failus vai direktorijus starp divām iekārtām lielā ātrumā:

```bash
# Saņēmējs klausās portā un raksta uz ./backup
sync0 recv 0.0.0.0:9000 ./backup

# Tieša pārraide no sūtītāja (pēc izvēles — dzēst spoguli, lai notīrītu liekos failus, — ātrums 50 MB ātruma ierobežojums)
sync0 send ./project 192.168.1.10:9000
```

### 2. Ziņu kopīgošana un vilkšana/augšupielāde pēc pieprasījuma (`share` / `pull` / `push`)

```bash
# Serveris: publicējiet koplietošanas grupu ar vienu komandrindu (pēc startēšanas izvadiet iroh mezgla adresi bez IP)
sync0 share dataset:./data

# Klients: pārvelciet visu datu kopas koplietošanas grupu uz vietējo ./local_data
sync0 pull dataset:// ./local_data -p <mezgla adrese>

# Klients: pēc pieprasījuma izvelciet vairākas atlases (sinhronizējiet tikai noteiktus atsevišķus failus un apakšdirektorijus)
sync0 pull dataset://docs/readme.md,assets/icons,src/core ./local_data -p <mezgla adrese>

# Klients: augšupielādējiet failus vai direktorijus pilnvarotās koplietošanas grupās
sync0 push dataset://releases/v1 ./build_output -p <mezgla adrese>
```

### 3. Reāllaika abonementu sēšana un vairāku iekārtu divvirzienu sinhronizācija (`sub` / `group`)

Ja pēc ilgstoša savienojuma izveides attālajā galā tiek veikts fails **pievienojot, pārveidojot, dzēšot, mainot atļaujas vai modificēšanas laiku**, lokālais direktorijs tiks sinhroni atjaunināts milisekundēs reāllaikā (jauni/modificēti lielie faili tikai izvilks atšķirību blokus un atjauninās tos vietā, failu dzēšana automātiski notīrīs lokālos failus un tukšos direktorijus un mainīs tikai atļaujas):

- **`sync0 sub`(Vienvirziena reāllaika spoguļošana + automātiska sēšana)**: lokālais reāllaika režīms seko koplietošanas gala papildinājumiem, dzēšanas un modifikācijām un automātiski kalpo kā P2P sākuma avots, lai nodrošinātu vienlaicīgu paātrinājumu citiem vilkšanas mezgliem.
- **`sync0 group`(Vairāku mašīnu vienādranga divvirzienu reāllaika sinhronizācija)**: ja faili tiek pievienoti, dzēsti vai pārveidoti jebkurā grupas iekārtā, visi pārējie tiešsaistes mezgli tiks sinhroni atjaunināti reāllaikā un kalpos viens otram kā sākumi.

```bash
# Abonējiet visu koplietošanas grupu: attālās pievienošanas, dzēšanas un modifikācijas tiek sinhronizētas lokāli reāllaikā un tiek automātiski iekļautas
sync0 sub dataset:// ./local_mirror -p <mezgla adrese>

# Vairāku atlases abonements pēc pieprasījuma: sinhronizējiet un ievadiet tikai noteiktus atsevišķus failus un apakšdirektorijus reāllaikā
sync0 apakšdatu kopa://models/llama.onnx,configs ./local_mirror -p <mezgla adrese>

# Vairāku mašīnu vienādranga grupas divvirzienu reāllaika sinhronizācija (atbalsta arī pilnu vai komatu vairāku atlases ceļus)
sync0 grupa team-docs:// ./docs -p <mezgla adrese>
```

### 4. Komandas vairāku koplietošanas grupu un dalībnieku atļauju konfigurācija (`.nt` konfigurācija)

Noklusējuma konfigurācijas direktorijs ir `~/.config/sync0` (var pārslēgt, izmantojot `-c <direktoriju>`), izmantojot minimālisma [NestedText](https://nestedtext.org) (`.nt`) formātu:
- **Atbalsta lasīšanas ierobežošanu publiskajā tīklā (lasīt var tikai autorizēti lietotāji)**: kamēr koplietošanas grupa ir konfigurēta ar `users:` dalībnieku sarakstu (vai komandrindā ir norādīts `--allow-r` / `--allow-rw` / `--allow-w` / `--auth`), sistēma automātiski ieslēdz privāto autentifikāciju ** un** =" un nē" uzskaitīšana vai lasīšana, un P2P abonēšana sēšanas mezgliem vienlaikus pārmantos arī balto sarakstu, ļaujot lasīt tikai autorizētiem lietotājiem; koplietošanas grupas, kas nav konfigurētas ar `users`, pēc noklusējuma ir publiskas un tikai lasāmas (ja vēlaties būt publiskas un tikai lasāmas un atvērtas augšupielādei tikai noteiktiem dalībniekiem, varat skaidri iestatīt `auth: false`).
- **Smalkas lasīšanas-rakstīšanas un apakšdirektoriju piegādes atļaujas**: dalībnieki pēc noklusējuma ir tikai lasāmi (`r` nav skaidri jāraksta), un tie atbalsta `r` (tikai lasāms), `w` (rakstīt tikai nolaižamajā lodziņā, nav lasāms), `rw` un `rw` (apakšnoteikums = prefix = id) augšupielādei).

**Dalībnieka konfigurācijas piemērs (`~/.config/sync0/users/`):**

```yaml
# ~/.config/sync0/users/alice.nt (noklusējuma tikai lasāms, ātruma ierobežojums 80 MB)
id: 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c
rate: 80MB
```

```yaml
# ~/.config/sync0/users/bob.nt (lasīšanas un rakstīšanas atļaujas, atļauts tikai augšupielādēt norādītajā apakšdirektorijā)
id: 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f
perm: rw
write_prefix:
  - uploads/bob
  - shared/common
```

```yaml
# ~/.config/sync0/users/charlie.nt (tikai rakstīšanas piegādes atļauja, lasīšana ir aizliegta)
id: 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
perm: w
```

**Koplietošanas grupas konfigurācijas piemērs (`~/.config/sync0/config.nt`):**

```yaml
users_dir: ./users

groups:
  # Nav konfigurēti lietotāji: noklusējuma publiskais tīkls ir tikai lasāms
  dataset:
    path: ./data
    rate: 50MB

  # Konfigurēti lietotāji: automātiski ierobežo publiskā tīkla lasīšanu (lasīt var tikai Alise un Bobs, Čārlijs var tikai piegādāt un augšupielādēt)
  team-vault:
    path: ./vault
    users:
      - alice
      - bob
      - charlie
```

Sāciet pakalpojumu (ielādējiet konfigurācijas failu vai tieši izmantojiet komandrindas parametrus, lai ierobežotu publiskā tīkla lasīšanu un autorizētu dalībniekus):

```bash
# Lai sāktu, ielādējiet ~/.config/sync0/config.nt
sync0 share --region SHA

# Vai arī publicējiet privātu koplietošanas grupu, izmantojot tīru komandrindu (norādiet --allow-*, lai automātiski ierobežotu publiskā tīkla lasīšanu un tai var piekļūt tikai pilnvaroti lietotāji)
sync0 share team-vault:./vault \
  --allow-r 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c \
  --allow-rw 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f:uploads/bob,shared/common \
  --allow-w 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
```

### 5. Mezgla adrese, vizītkarte un vairākas valodas

```bash
# Skatiet vietējo mezgla adresi bez IP (atslēga tiek automātiski ģenerēta pirmajai palaišanai)
sync0 id

# Ģenerēt Ed25519 pašparakstītu mezgla vizītkarti (segvārds, pilsētas kods, e-pasts, vietne, ievads)
sync0 profile -u alice --region SHA --email alice@example.com -b "Shanghai node"

# Ir 75 iebūvētās valodas, kas pēc noklusējuma seko sistēmas valodai, un tās var pārslēgt, izmantojot vides mainīgos.
SYNC0_LANG=en sync0 --help
```

## Veiktspēja un galvenās funkcijas

### Starpkontinentālais WAN faktiskais mērījums (salīdzinājums `rsync`)

Faktiskā mērījumu vide: Amerikas Savienotās Valstis `c1` → Eiropa `n1` (RTT 65 ms), datu kopa ir Linux kodols `linux-7.3.3`:

#### 1) A la carte režīms pret `rsync`

| Datu kopa | izmērs | `sync0` Viens punkts<br>(Mb/s) | `rsync`<br>(Mbps) | Paātrinājums (`sync0` / `rsync`) |
| :--- | ---: | ---: | ---: | ---: |
| `linux-7.3.3.tar.xz`(nav saspiežams) | 155 MB | **236.48** | 108.74 | **2,17×** |
| `linux-7.3.3.tar`(saspiežams) | 1.6 GB | **843.83** | 385.06 | **2,19×** |
| `linux-7.3.3.tar`(lokālo izmaiņu pieaugums) | 1.6 GB | **8,014.15** | 1,361.72 | **5,89 ×** |
| `linux-7.3.3/` (95 979 mazi faili) | 1.5 GB | **627.58** | 347.80 | **1,80 ×** |
| **ģeometriskais vidējais** | — | **1,000.91** | 375.26 | **2,67 ×** |

#### 2) P2P divu avotu režīms pret `rsync`

| Datu kopa | izmērs | `sync0` P2P Dual Source<br>(Mb/s) | `rsync`<br>(Mbps) | Paātrinājums (`sync0` / `rsync`) |
| :--- | ---: | ---: | ---: | ---: |
| `linux-7.3.3.tar.xz`(nav saspiežams) | 155 MB | **330.09** | 108.74 | **3,04×** |
| `linux-7.3.3.tar`(saspiežams) | 1.6 GB | **1,551.86** | 385.06 | **4,03×** |
| `linux-7.3.3.tar`(lokālo izmaiņu pieaugums) | 1.6 GB | **5,474.17** | 1,361.72 | **4,02×** |
| `linux-7.3.3/` (95 979 mazi faili) | 1.5 GB | **706.56** | 347.80 | **2,03×** |
| **ģeometriskais vidējais** | — | **1,186.42** | 375.26 | **3,16×** |

> Piezīme. P2P divu avotu režīmā Eiropas mezgli `s1` sadarbojas sēšanai, un starpkontinentālais mugurkauls RTT ir 65 ms.

### Produkta galvenās īpašības

- **Publiskās atslēgas adresācijas automātiskā caurumošana**: nav nepieciešams centrālais serveris un publiskā tīkla adrese, automātiska NAT caurumu noteikšana, pamatojoties uz izplatītās tīkla publiskās atslēgas adresi.
- **Milisekundes līmeņa reāllaika sinhronizācija un metadatu apzināšanās**: balstās uz vietējo failu notikumu uzraudzību (`FSEvents` / `inotify` / `ReadDirectoryChangesW`), failu pievienošanu, dzēšanu reāllaikā, POSIX atļaujām (`mode`) un milisekundes = modifikācijas laiku ("><b">5); nulles datu bloka pārraide, kad tiek mainītas tikai atļaujas vai laiks.
- **BT stila vienlaicīga lejupielāde un augšupielāde un reģionālās radniecības plānošana**: lejupielādes mezglam nav jāgaida, līdz tiks lejupielādēta visa summa. Visi bloku intervāli (`Op::Have`), kas ir izturējuši 128 bitu jaukšanas pārbaudi, tiek nekavējoties bloķēti atmiņā/pagaidu pūlā, lai citi mezgli varētu tos veikt vienlaikus. Kopā ar dinamisku PEX mezglu apmaiņu un lēnu mezgla puses priekšrocību (Work-Stealing) tiek palielināta klastera caurlaidspēja.
- **Pretļaunprātīga mezgla iejaukšanās un klasteru saindēšanās imunitāte**: autoritatīvs avotu saraksts `root_hash` Paškonsekventa bloķēšana + obligāta aparatūras paātrināšana `gxhash128` īssavilkuma pārbaude pirms katra bloka ievietošanas diskā un ārējas izsējas; ja tiek atrasts bojāts bloks, milisekundes līmeņa ķēdes pārtraucējs aptumšo ļaunprātīgo mezglu un automātiski atkops no godīgā avota.
- **FastCDC in situ palielināšana un mazu failu apkopošana**: lielu failu daļējai modificēšanai tiek pārsūtīti tikai diferenciālie bloki un ievietoti diskā; masveida mazo failu atmiņas iesaiņojums un prefiksu diferenciālā saspiešana novērš pieskaitāmās izmaksas pa failiem.
- **Pašdziedināšanās pēc strāvas padeves pārtraukuma un atsākšana pēc restartēšanas**: Redis līdzīgais RDB + AOF hibrīds formāts (`.s0_subs.bin`) tiek izmantots, lai saglabātu sēšanas statusu un failu sarakstu, ikdienas izmaiņas tiek pievienotas mikrosekundēs un tiek automātiski pārrakstītas un regulāri sakārtotas. Pēc servera restartēšanas bezsaistes izmaiņas tiek automātiski pārsūtītas uz sēšanas mezglu.

## Tehniskā arhitektūra un dokumentācija

- **Divplakņu atsaistes protokols**:
  - **Vadības un reāllaika izmaiņu virsma (`iroh` QUIC/TCP, pirmā baita `PROTO_VERSION` versijas rokasspiediens) **: atbild par Ed25519 vizītkartes verifikāciju, atļauju verifikāciju, Swarm sēklu atklāšanu un atbalstu `u64::MAX` līmeņa masveida izmaiņu straumēšanai un <b">b ="4"> <b">b id = 5 id="6"> / `FsOp::Remove` / `FsOp::Meta` Trīs primitīvi).
  - **Datu izplatīšanas virsma (vietējā UDP + `compio` asinhronā I/O) **: 20 B kompaktā galvene, 18 B TACK periodiskā atgriezeniskā saite, BBRv3/Copa hibrīda sastrēgumu kontrole, RACK-TLP pakešu zudumu atkopšana, pirmā bloka Šenonas entropijas adaptīvā Zstd kompresijas apvedceļš="2"> un <b end-to.
- **Uzziniet vairāk**:
  - [Veiktspējas mērījumu atskaite un reproducēšanas skripts](https://github.com/webc-site/sync0/blob/main/doc/zh/benchmark.md)
  - [Sistēmas arhitektūras projektēšana](https://github.com/webc-site/sync0/blob/main/doc/zh/design_v4.md)
  - [Bināro vadu protokols un noturības specifikācija](https://github.com/webc-site/sync0/blob/main/doc/zh/proto.md)
  - [Identitātes autentifikācija, autoritātes kontrole un integritātes pārbaude](https://github.com/webc-site/sync0/blob/main/doc/zh/crypto.md)