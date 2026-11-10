# sync0

`sync0` je decentralizirano P2P dijeljenje datoteka, sinkronizacija u stvarnom vremenu i alat za distribuciju s više izvora koji se koristi za zamjenu `rsync` - izmjerena brzina transkontinentalnog WAN-a je do `rsync` **5,89×** (prosječna geometrijska brzina u jednoj točki **2,67×**, prosječna geometrijska brzina P2P s dva izvora **3,16×**), i P2P ubrzanje transoceanske veze s visokom latencijom do **7,68×** (sveobuhvatna geometrijska srednja brzina 24 scenarija pune veze **2,56×**).

Svaki je stroj i poslužitelj i klijent: nema potrebe za središnjim poslužiteljem ili javnom mrežnom adresom, automatsko bušenje rupa temelji se na adresi javnog ključa distribuirane mreže i podržava izravan prijenos između dva računala, hosting grupnih dopuštenja za više dijeljenja, višestruki odabir poddirektorija na zahtjev, inkrementalnu sinkronizaciju u stvarnom vremenu na razini milisekunde i P2P istodobno ubrzanje s više izvora.

## Instalirati

### Metoda 1: Instalacija tereta

```bash
cargo install sync0
```

### Metoda 2: preuzmite unaprijed kompajlirane binarne datoteke

Idite na [GitHub Releases](https://github.com/webc-site/sync0/releases) da preuzmete `.tar.xz` komprimirani paket koji odgovara sustavu (podržava Linux, macOS, Windows, Android, iOS, FreeBSD), raspakirajte i stavite u `PATH`:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Počnite brzo

### 1. Izravan prijenos nulte konfiguracije između dva računala (`send` / `recv`)

Nema potrebe za konfiguriranjem grupe za dijeljenje, izravnim prijenosom datoteka ili direktorija između dva stroja velikom brzinom:

```bash
# Primatelj sluša port i piše u ./backup
sync0 recv 0.0.0.0:9000 ./backup

# Izravan prijenos od pošiljatelja (opcionalno --izbrišite zrcalo za čišćenje viška datoteka, --rate ograničenje brzine od 50 MB)
sync0 send ./project 192.168.1.10:9000
```

### 2. Dijeljenje postova i povlačenje/upload na zahtjev (`share` / `pull` / `push`)

```bash
# Poslužitelj: objavite grupu za dijeljenje jednim retkom naredbe (ispiši adresu iroh čvora bez IP-a nakon pokretanja)
sync0 share dataset:./data

# Klijent: Povucite cijelu grupu za dijeljenje skupa podataka na lokalni ./local_data
sync0 povući skup podataka:// ./local_data -p <adresa čvora>

# Klijent: povucite više odabira na zahtjev (sinkronizirajte samo određene pojedinačne datoteke i poddirektorije)
sync0 pull dataset://docs/readme.md,assets/icons,src/core ./local_data -p <adresa čvora>

# Klijent: Učitajte datoteke ili direktorije u ovlaštene grupe za dijeljenje
sync0 push dataset://releases/v1 ./build_output -p <adresa čvora>
```

### 3. Sijanje pretplate u stvarnom vremenu i dvosmjerna sinkronizacija s više strojeva (`sub` / `group`)

Nakon uspostavljanja duge veze, ako se bilo koja datoteka **dodaje, mijenja, briše, mijenja dopuštenja ili vrijeme izmjene** dogodi na udaljenom kraju, lokalni imenik će se sinkronizirano ažurirati u milisekundama u stvarnom vremenu (nove/modificirane velike datoteke samo će povući blokove razlika i ažurirati ih na mjestu, brisanje datoteka će automatski očistiti lokalne datoteke i prazne direktorije, a samo izravno promijeniti dopuštenja ili izravno označiti vremenske oznake nula blokova podataka):

- **`sync0 sub`(Jednosmjerno zrcaljenje u stvarnom vremenu + automatsko sijanje)**: Lokalno stvarno vrijeme prati dodavanja, brisanja i izmjene na kraju dijeljenja i automatski služi kao izvor P2P seeda za pružanje istovremenog ubrzanja za druge vučne čvorove.
- **`sync0 group`(Multi-machine peer-to-peer dvosmjerna sinkronizacija u stvarnom vremenu)**: Ako se datoteke dodaju, izbrišu ili modificiraju na bilo kojem računalu u grupi, svi ostali mrežni čvorovi ažurirat će se sinkronizirano u stvarnom vremenu i služiti kao sjemena jedni drugima.

```bash
# Pretplatite se na cijelu grupu za dijeljenje: udaljeni dodaci, brisanja i izmjene sinkroniziraju se lokalno u stvarnom vremenu i automatski se dodaju
sync0 sub dataset:// ./local_mirror -p <adresa čvora>

# Pretplata s višestrukim odabirom na zahtjev: sinkronizirajte i postavljajte samo određene pojedinačne datoteke i poddirektorije u stvarnom vremenu
sync0 sub dataset://models/llama.onnx,configs ./local_mirror -p <adresa čvora>

# Dvosmjerna sinkronizacija peer grupe s više strojeva u stvarnom vremenu (također podržava pune putanje ili putanje s višestrukim odabirom zareza)
sync0 group team-docs:// ./docs -p <adresa čvora>
```

### 4. Grupa za više dijeljenja i konfiguracija dozvola za članove (`.nt` konfiguracija)

Zadani konfiguracijski direktorij je `~/.config/sync0` (može se prebaciti preko `-c <directory>`), koristeći minimalistički [NestedText](https://nestedtext.org) (`.nt`) format:
- **Podržava ograničavanje čitanja javne mreže (samo ovlašteni korisnici mogu čitati)**: Sve dok je grupa za dijeljenje konfigurirana s `users:` popisom članova (ili naredbeni redak navodi `--allow-r` / `--allow-rw` / `--allow-w` / `--auth`), sustav **automatski uključuje privatnu provjeru autentičnosti (`auth: true`)** i zabranjuje nabrajanje ili čitanje anonimnih čvorova javne mreže, a P2P pretplata na čvorove za sijanje također će naslijediti popis dopuštenih simultano, dopuštajući čitanje samo ovlaštenim korisnicima; grupe za dijeljenje koje nisu konfigurirane s `users` prema zadanim su postavkama javne i samo za čitanje (ako želite biti javne i samo za čitanje te otvorene za prijenose samo određenim članovima, možete eksplicitno postaviti `auth: false`).
- **Precizna dopuštenja za čitanje-pisanje i isporuku poddirektorija**: Članovi su prema zadanim postavkama samo za čitanje (`r` ne treba biti eksplicitno upisan) i podržavaju `r` (samo za čitanje), `w` (samo pisanje u padajući okvir, nije čitljivo), `rw` (čitanje-pisanje) i `write_prefix` (ograničenje prefiks poddirektorija dopušten za učitavanje).

**Primjer konfiguracije člana (`~/.config/sync0/users/`):**

```yaml
# ~/.config/sync0/users/alice.nt (zadano samo za čitanje, ograničenje brzine 80 MB)
id: 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c
rate: 80MB
```

```yaml
# ~/.config/sync0/users/bob.nt (dozvole za čitanje i pisanje, dozvoljeno je samo učitavanje u navedeni poddirektorij)
id: 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f
perm: rw
write_prefix:
  - uploads/bob
  - shared/common
```

```yaml
# ~/.config/sync0/users/charlie.nt (dopuštenje samo za pisanje, čitanje je zabranjeno)
id: 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
perm: w
```

**Primjer konfiguracije grupe za dijeljenje (`~/.config/sync0/config.nt`):**

```yaml
users_dir: ./users

groups:
  # Nekonfigurirani korisnici: zadana javna mreža samo za čitanje
  dataset:
    path: ./data
    rate: 50MB

  # Konfigurirani korisnici: automatski ograniči čitanje javne mreže (samo alice i bob mogu čitati, charlie može samo isporučivati ​​i učitavati)
  team-vault:
    path: ./vault
    users:
      - alice
      - bob
      - charlie
```

Pokrenite uslugu (učitajte konfiguracijsku datoteku ili izravno upotrijebite parametre naredbenog retka za ograničavanje čitanja javne mreže i autoriziranje članova):

```bash
# Učitajte ~/.config/sync0/config.nt za početak
sync0 share --region SHA

# Ili objavite privatnu grupu za dijeljenje putem čistog naredbenog retka (navedite --allow-* da automatski ograničite čitanje javne mreže i da joj samo ovlašteni korisnici mogu pristupiti)
sync0 share team-vault:./vault \
  --allow-r 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c \
  --allow-rw 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f:uploads/bob,shared/common \
  --allow-w 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
```

### 5. Adresa čvora, posjetnica i više jezika

```bash
# Prikaz lokalne adrese čvora bez IP-a (ključ se automatski generira za prvo pokretanje)
sync0 id

# Generirajte Ed25519 samopotpisanu posjetnicu čvora (nadimak, kod grada, e-pošta, web stranica, uvod)
sync0 profile -u alice --region SHA --email alice@example.com -b "Shanghai node"

# Postoji 75 ugrađenih jezika, koji prema zadanim postavkama slijede jezik sustava i mogu se mijenjati putem varijabli okruženja.
SYNC0_LANG=en sync0 --help
```

## Izvedba i osnovne značajke

### Stvarno mjerenje preko kontinentalnog WAN-a (Usporedba `rsync`)

Stvarno mjerno okruženje: Sjedinjene Države `c1` → Europa `n1` (RTT 65 ms), skup podataka je Linux kernel `linux-7.3.3`:

#### 1) A la carte način rada u odnosu na `rsync`

| Skup podataka | veličina | `sync0` Jedna točka<br>(Mbps) | `rsync`<br>(Mbps) | Ubrzanje (`sync0` / `rsync`) |
| :--- | ---: | ---: | ---: | ---: |
| `linux-7.3.3.tar.xz`(nije kompresivan) | 155 MB | **236.48** | 108.74 | **2,17×** |
| `linux-7.3.3.tar`(sažimanje) | 1.6 GB | **843.83** | 385.06 | **2,19×** |
| `linux-7.3.3.tar`(povećanje lokalne promjene) | 1.6 GB | **8,014.15** | 1,361.72 | **5,89×** |
| `linux-7.3.3/` (95.979 malih datoteka) | 1.5 GB | **627.58** | 347.80 | **1,80×** |
| **geometrijska sredina** | — | **1,000.91** | 375.26 | **2,67×** |

#### 2) P2P način s dva izvora u odnosu na `rsync`

| Skup podataka | veličina | `sync0` P2P Dual Source<br>(Mbps) | `rsync`<br>(Mbps) | Ubrzanje (`sync0` / `rsync`) |
| :--- | ---: | ---: | ---: | ---: |
| `linux-7.3.3.tar.xz`(nije kompresivan) | 155 MB | **330.09** | 108.74 | **3,04×** |
| `linux-7.3.3.tar`(sažimanje) | 1.6 GB | **1,551.86** | 385.06 | **4,03×** |
| `linux-7.3.3.tar`(povećanje lokalne promjene) | 1.6 GB | **5,474.17** | 1,361.72 | **4,02×** |
| `linux-7.3.3/` (95.979 malih datoteka) | 1.5 GB | **706.56** | 347.80 | **2,03×** |
| **geometrijska sredina** | — | **1,186.42** | 375.26 | **3,16×** |

> Napomena: U P2P načinu rada s dva izvora, europski čvorovi `s1` surađuju za sijanje, a RTT međukontinentalne okosnice je 65 ms.

### Osnovne karakteristike proizvoda

- **Automatsko bušenje rupa za adresiranje javnog ključa**: Nema potrebe za središnjim poslužiteljem i javnom mrežnom adresom, automatsko bušenje rupa NAT-a na temelju adrese javnog ključa distribuirane mreže.
- **Sinkronizacija u stvarnom vremenu i svijest o metapodacima na razini milisekunde**: Na temelju izvornog praćenja događaja u datotekama (`FSEvents` / `inotify` / `ReadDirectoryChangesW`), dodavanja datoteka, brisanja u stvarnom vremenu, POSIX dozvola (`mode`) i vremena izmjene na razini milisekunde (`mtime`); prijenos nultog bloka podataka kada se mijenjaju samo dopuštenja ili vrijeme.
- **Istovremeno preuzimanje i učitavanje u stilu BT-a i planiranje regionalnog afiniteta**: Čvor za preuzimanje ne mora čekati da se puni iznos preuzme. Svi intervali blokova (`Op::Have`) koji su prošli 128-bitnu raspršivačku provjeru odmah se zaključavaju u memoriji/privremenom spremištu da bi drugi čvorovi mogli povlačiti istovremeno. Zajedno s dinamičkom razmjenom PEX čvorova i sporim polupreuzimanjem čvora (Work-Stealing), propusnost klastera je maksimizirana.
- **Protiv zlonamjernog petljanja čvora i otpornost na trovanje klastera**: autoritativni izvorni popis `root_hash` Samodosljedno zaključavanje + obavezno hardversko ubrzanje `gxhash128` provjera sažetka prije nego što se svaki blok postavi na disk i eksterno popuni; ako se pronađe neovlašteni blok, prekidač na razini milisekunde zamračit će zlonamjerni čvor i automatski se oporaviti od poštenog izvora.
- **FastCDC in-situ inkrement i mala agregacija datoteka**: Samo se diferencijalni blokovi prenose radi djelomične izmjene velikih datoteka i smještaju na disk; veliko memorijsko pakiranje malih datoteka i diferencijalna kompresija prefiksa eliminiraju povratne troškove datoteke po datoteke.
- **Samooporavljanje nakon nestanka struje i nastavak nakon ponovnog pokretanja**: RDB + AOF hibridni format sličan Redisu (`.s0_subs.bin`) koristi se za održavanje statusa sijanja i popisa datoteka, dnevne promjene se dodaju u mikrosekundama i automatski se ponovno pišu i redovito organiziraju. Nakon ponovnog pokretanja poslužitelja, izvanmrežne promjene automatski se guraju u čvor za sijanje.

## Tehnička arhitektura i dokumentacija

- **Protokol razdvajanja dvostruke ravnine**:
  - **Kontrola i površina promjene u stvarnom vremenu (`iroh` QUIC / TCP, prvi bajt `PROTO_VERSION` rukovanje verzijom) **: Odgovoran za Ed25519 provjeru posjetnice, provjeru dopuštenja, otkrivanje sjemena roja i podršku za `u64::MAX` razinu masovne promjene strujanja i `mzstd` komprimirani `SubPush` push u stvarnom vremenu (`FsOp::Upsert` / `FsOp::Remove` / `FsOp::Meta` Tri primitive).
  - **Površina distribucije podataka (izvorni UDP + `compio` asinkroni I/O) **: 20B kompaktno zaglavlje, 18B TACK periodične povratne informacije, BBRv3/Copa hibridna kontrola zagušenja, RACK-TLP oporavak od gubitka paketa, premosnica kompresije Shannonove adaptivne entropije prvog bloka i `gxhash128` provjera end-to-end bloka.
- **Saznajte više**:
  - [Izvješće o mjerenju učinka i skripta za reprodukciju](https://github.com/webc-site/sync0/blob/main/doc/zh/benchmark.md)
  - [Projektiranje arhitekture sustava](https://github.com/webc-site/sync0/blob/main/doc/zh/design_v4.md)
  - [Protokol binarne žice i specifikacija postojanosti](https://github.com/webc-site/sync0/blob/main/doc/zh/proto.md)
  - [Autentikacija identiteta, kontrola autoriteta i provjera integriteta](https://github.com/webc-site/sync0/blob/main/doc/zh/crypto.md)