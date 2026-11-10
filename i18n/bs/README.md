# sync0

`sync0` je decentralizirani P2P dijeljenje datoteka, sinhronizacija u realnom vremenu i alat za distribuciju iz više izvora koji se koristi za zamjenu `rsync` - izmjerena brzina transkontinentalne mreže širokog područja je do **6,95×** od `rsync` (geometrijska prosječna brzina u jednoj tački **2,01×3× geometrijska prosječna brzina), P srednja geometrijska brzina **2,01×3×**. a sinhronizacija velikih malih fajlova preko okeana je brza od **7,19×**.

Svaka mašina je i server i klijent: nema potrebe za centralnim serverom ili javnom mrežnom adresom, automatsko bušenje rupa je bazirano na adresi javnog ključa distribuirane mreže i podržava direktan prenos između dve mašine, hosting grupnih dozvola za više deljenja, višestruki izbor poddirektorija na zahtev, inkrementalnu sinkronizaciju na nivou milisekundi u realnom vremenu i istovremeno P2P ubrzanje više izvora.

## Instaliraj

### Metoda 1: Instalacija tereta

```bash
cargo install sync0
```

### Metoda 2: Preuzmite unaprijed kompajlirane binarne datoteke

Idite na [GitHub Releases](https://github.com/webc-site/sync0/releases) da preuzmete `.tar.xz` komprimirani paket koji odgovara sistemu (podržava Linux, macOS, Windows, Android, iOS, FreeBSD), raspakirajte i stavite u `PATH`:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Započnite brzo

### 1. Direktan prijenos nulte konfiguracije između dvije mašine (`send` / `recv`)

Nema potrebe da konfigurišete grupu za deljenje, direktno prenosite datoteke ili direktorijume između dve mašine velikom brzinom:

```bash
# Primalac sluša port i upisuje u ./backup
sync0 recv 0.0.0.0:9000 ./backup

# Direktan prijenos od pošiljatelja (opcionalno -- izbrišite ogledalo da očistite višak fajlova, -- ocijenite ograničenje brzine od 50 MB)
sync0 send ./project 192.168.1.10:9000
```

### 2. Dijeljenje objava i povlačenje/učitavanje na zahtjev (`share` / `pull` / `push`)

```bash
# Server: Objavite grupu za dijeljenje jednom linijom komande (iznesite iroh adresu bez IP-a nakon pokretanja)
sync0 share dataset:./data

# Klijent: Povucite cijelu grupu za dijeljenje skupa podataka na lokalni ./local_data
sync0 pull dataset:// ./local_data -p <adresa čvora>

# Klijent: Povucite više odabira na zahtjev (sinhronizirajte samo određene pojedinačne datoteke i poddirektorije)
sync0 pull dataset://docs/readme.md,assets/icons,src/core ./local_data -p <adresa čvora>

# Klijent: Učitajte datoteke ili direktorije u ovlaštene grupe za dijeljenje
sync0 push dataset://releases/v1 ./build_output -p <adresa čvora>
```

### 3. Postavljanje pretplate u stvarnom vremenu i dvosmjerna sinhronizacija s više mašina (`sub` / `group`)

Nakon uspostavljanja duge veze, ako se bilo koja datoteka **dodavanje, modificiranje, brisanje, mijenjanje dozvola ili vrijeme modifikacije** dogodi na udaljenom kraju, lokalni direktorij će se sinhrono ažurirati u milisekundama u stvarnom vremenu (novi/modificirani veliki fajlovi će samo povući blokove razlike i ažurirati ih na mjestu, brisanje datoteka će automatski očistiti lokalne datoteke i isprazniti samo podatke o vremenu i direktno blokirati zmp direktorije, zmp blokirati direktno).

- **`sync0 sub`(Jednosmjerno zrcaljenje u realnom vremenu + automatsko zamjena)**: Lokalno realno vrijeme prati dodavanja, brisanja i modifikacije kraja dijeljenja i automatski služi kao P2P izvorni izvor za pružanje istovremenog ubrzanja za druge povlačne čvorove.
- **`sync0 group`(Multi-machine peer-to-peer dvosmjerna sinhronizacija u realnom vremenu)**: Ako se fajlovi dodaju, brišu ili modificiraju na bilo kojem stroju u grupi, svi ostali mrežni čvorovi će se ažurirati sinhrono u realnom vremenu i služiti kao seme jedni drugima.

```bash
# Pretplatite se na cijelu grupu za dijeljenje: daljinski dodaci, brisanja i izmjene se sinkroniziraju lokalno u stvarnom vremenu i automatski se postavljaju
sync0 sub dataset:// ./local_mirror -p <adresa čvora>

# Pretplata na višestruki odabir na zahtjev: samo sinkronizirajte i započnite određene pojedinačne datoteke i poddirektorije u stvarnom vremenu
sync0 sub dataset://models/llama.onnx,configs ./local_mirror -p <adresa čvora>

# Dvosmjerna sinkronizacija u realnom vremenu (također podržava pune ili višestruke putanje za odabir zarezima)
sync0 group team-docs:// ./docs -p <adresa čvora>
```

### 4. Grupa za višestruko dijeljenje tima i konfiguracija dozvola za članove (`.nt` konfiguracija)

Zadani direktorij konfiguracije je `~/.config/sync0` (može se prebaciti putem `-c <directory>`), koristeći minimalistički [NestedText](https://nestedtext.org) (`.nt`) format:
- **Podržava ograničavanje čitanja javne mreže (mogu čitati samo ovlašteni korisnici)**: Sve dok je grupa za dijeljenje konfigurirana sa `users:` listom članova (ili komandna linija specificira `--allow-r` / `--allow-rw` / `--allow-w` / `--auth`), sistem se **privatno uključuje`--auth`, sistem se **privatno uključuje`users` su prema zadanim postavkama javne i samo za čitanje (ako želite da budu javne i samo za čitanje i otvorene za učitavanje samo određenim članovima, možete eksplicitno postaviti `auth: false`).
- **Precizne dozvole za čitanje-upisivanje i isporuku poddirektorijuma**: Članovi su prema zadanim postavkama samo za čitanje (`r` ne mora biti eksplicitno napisan) i podržavaju `r` (samo za čitanje), `w` (samo pisati u okvir za isporuku, nije čitljivo), `rw` (limit id="4">) prefiks poddirektorijuma dozvoljen za učitavanje).

**Primjer konfiguracije člana (`~/.config/sync0/users/`):**

```yaml
# ~/.config/sync0/users/alice.nt (zadano samo za čitanje, ograničenje brzine 80MB)
id: 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c
rate: 80MB
```

```yaml
# ~/.config/sync0/users/bob.nt (dozvole čitanja i pisanja, dozvoljeno samo učitavanje u specificirani poddirektorij)
id: 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f
perm: rw
write_prefix:
  - uploads/bob
  - shared/common
```

```yaml
# ~/.config/sync0/users/charlie.nt (samo dozvola za isporuku, čitanje je zabranjeno)
id: 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
perm: w
```

**Primjer konfiguracije grupe za dijeljenje (`~/.config/sync0/config.nt`):**

```yaml
users_dir: ./users

groups:
  # Korisnici koji nisu konfigurirani: zadana javna mreža samo za čitanje
  dataset:
    path: ./data
    rate: 50MB

  # Konfigurirani korisnici: automatski ograničavaju čitanje javne mreže (samo Alice i Bob mogu čitati, Charlie može samo isporučiti i učitati)
  team-vault:
    path: ./vault
    users:
      - alice
      - bob
      - charlie
```

Pokrenite uslugu (učitajte konfiguracijsku datoteku ili direktno koristite parametre komandne linije da ograničite čitanje javne mreže i ovlastite članove):

```bash
# Učitajte ~/.config/sync0/config.nt za početak
sync0 share --region SHA

# Ili objavite privatnu grupu za dijeljenje putem čiste komandne linije (navedite --allow-* da automatski ograničite čitanje javne mreže i samo ovlašteni korisnici joj mogu pristupiti)
sync0 share team-vault:./vault \
  --allow-r 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c \
  --allow-rw 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f:uploads/bob,shared/common \
  --allow-w 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
```

### 5. Adresa čvora, vizit karta i višejezičnost

```bash
# Pogledajte lokalnu adresu čvora bez IP-a (ključ se automatski generira za prvo pokretanje)
sync0 id

# Generirajte Ed25519 samopotpisanu posjetnicu čvora (nadimak, šifra grada, e-mail, web stranica, uvod)
sync0 profile -u alice --region SHA --email alice@example.com -b "Shanghai node"

# Postoji 75 ugrađenih jezika, koji podrazumevano prate sistemski jezik i mogu se prebacivati ​​kroz varijable okruženja.
SYNC0_LANG=en sync0 --help
```

## Performanse i osnovne karakteristike

### Stvarno mjerenje međukontinentalnog WAN-a (Poređenje `rsync`)

Stvarno okruženje mjerenja: Sjedinjene Američke Države `c1` → Evropa `n1` (RTT 65 ms), skup podataka je Linux kernel `linux-7.3.3`:

#### 1) A la carte način u odnosu na `rsync`

| Skup podataka | veličina | `sync0` Jedna tačka<br>(Mbps) | `rsync`<br>(Mbps) | Ubrzavanje (`sync0` / `rsync`) |
| :--- | ---: | ---: | ---: | ---: |
| `linux-7.3.3.tar.xz`(nije kompresibilno) | 155 MB | **121.07** | 107.89 | **1,12×** |
| `linux-7.3.3.tar`(kompresibilno) | 1.6 GB | **442.07** | 506.62 | **0,87×** |
| `linux-7.3.3.tar`(porast lokalne promjene) | 1.6 GB | **8,123.87** | 1,169.49 | **6,95×** |
| `linux-7.3.3/` (95.979 malih fajlova) | 1.5 GB | **565.49** | 235.27 | **2,40×** |
| **geometrijska sredina** | — | **704.17** | 350.19 | **2,01×** |

#### 2) P2P dual-source mod vs `rsync`

| Skup podataka | veličina | `sync0` P2P Dual Source<br>(Mbps) | `rsync`<br>(Mbps) | Ubrzavanje (`sync0` / `rsync`) |
| :--- | ---: | ---: | ---: | ---: |
| `linux-7.3.3.tar.xz`(nije kompresibilno) | 155 MB | **466.85** | 107.89 | **4,33×** |
| `linux-7.3.3.tar`(kompresibilno) | 1.6 GB | **992.17** | 506.62 | **1,96×** |
| `linux-7.3.3.tar`(porast lokalne promjene) | 1.6 GB | **5,666.84** | 1,169.49 | **4,85×** |
| `linux-7.3.3/` (95.979 malih fajlova) | 1.5 GB | **703.30** | 235.27 | **2,99×** |
| **geometrijska sredina** | — | **1,165.63** | 350.19 | **3,33×** |

> Napomena: U P2P modu sa dvostrukim izvorom, evropski čvorovi `s1` sarađuju za zasijavanje, a RTT međukontinentalne okosnice iznosi 65 ms.

### Osnovne karakteristike proizvoda

- **Automatsko probijanje rupa za adresiranje javnog ključa**: Nema potrebe za centralnim serverom i javnom mrežnom adresom, automatsko NAT bušenje rupa na osnovu adrese distribuiranog mrežnog javnog ključa.
- **Sinhronizacija u realnom vremenu na nivou milisekunde i svijest o metapodacima**: Zasnovano na praćenju događaja u izvornom fajlu (`FSEvents` / `inotify` / `ReadDirectoryChangesW`), u stvarnom vremenu dodavanja datoteka, brisanja, POSIX dozvola (`mode`) i vremena modifikacije nivoa i milisekunde=""> u realnom vremenu; nulti prijenos bloka podataka kada se mijenjaju samo dozvole ili vrijeme.
- **simultano preuzimanje i otpremanje u BT stilu i zakazivanje regionalnih afiniteta**: Čvor za preuzimanje ne mora čekati da se preuzme pun iznos. Svi blok intervali (`Op::Have`) koji su prošli 128-bitnu heš provjeru odmah se zaključavaju u memoriji/privremenom spremištu kako bi drugi čvorovi mogli povući istovremeno. Zajedno sa dinamičkom razmjenom PEX čvorova i sporom polovičnom preuzimanjem čvorova (Krađa posla), propusnost klastera je maksimizirana.
- **Imunitet protiv neovlaštenog mijenjanja čvorova i trovanja klastera**: autoritativna lista izvora `root_hash` Samodosljedno zaključavanje + obavezno hardversko ubrzanje `gxhash128` provjera sažetka prije nego se svaki blok postavi na disk i eksterno seeda; ako se pronađe neovlašteni blok, prekidač na nivou milisekundi će zamračiti zlonamjerni čvor i automatski se oporaviti od iskrenog izvora.
- **FastCDC in-situ inkrement i agregacija malih datoteka**: Samo diferencijalni blokovi se prenose za delimičnu modifikaciju velikih datoteka i stavljaju na disk; masivno pakovanje male memorije datoteka i diferencijalna kompresija prefiksa eliminišu povratne troškove fajl-po-datoteke.
- **Samooporavljanje nakon nestanka struje i nastavak nakon ponovnog pokretanja**: hibridni format RDB + AOF (`.s0_subs.bin`) sličan Redisu (`.s0_subs.bin`) koristi se za održavanje statusa zasada i liste datoteka, dnevne promjene se dodaju u mikrosekundama i automatski se prepisuju i redovno organiziraju. Nakon ponovnog pokretanja servera, vanmrežne promjene se automatski guraju u početni čvor.

## Tehnička arhitektura i dokumentacija

- **Protokol razdvajanja dvostruke ravni**:
  - **Kontrola i površina promjene u stvarnom vremenu (`iroh` QUIC / TCP, prvi bajt `PROTO_VERSION` rukovanje verzije) **: Odgovoran za Ed25519 verifikaciju vizitkarte, verifikaciju dozvole, otkrivanje semena roja i podršku za `u64::MAX` nivo masovne promjene u stvarnom vremenu prijenosa i <b> i kompresije <b> id" (`FsOp::Upsert` / `FsOp::Remove` / `FsOp::Meta` Tri primitiva).
  - **Površina distribucije podataka (nativni UDP + `compio` asinhroni I/O) **: 20B kompaktno zaglavlje, 18B TACK periodične povratne informacije, BBRv3/Copa hibridna kontrola zagušenja, RACK-TLP oporavak gubitka paketa, prvi blok Shannon entropijski adaptivni Zstd-kompresija <b end-2" pas i kompresija na kraju i. verifikacija.
- **Saznajte više**:
  - [Izvještaj o mjerenju učinka i skripta za reprodukciju](https://github.com/webc-site/sync0/blob/main/doc/zh/benchmark.md)
  - [Dizajn arhitekture sistema](https://github.com/webc-site/sync0/blob/main/doc/zh/design_v4.md)
  - [Protokol binarne žice i specifikacija postojanosti](https://github.com/webc-site/sync0/blob/main/doc/zh/proto.md)
  - [Provjera identiteta, kontrola ovlaštenja i provjera integriteta](https://github.com/webc-site/sync0/blob/main/doc/zh/crypto.md)