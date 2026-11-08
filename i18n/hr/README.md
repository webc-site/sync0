# sync0

`sync0` je peer-to-peer (P2P) alat za dijeljenje datoteka i sinkronizaciju na više strojeva koji se natječe s Resilio Sync.

Svaki je stroj i poslužitelj i klijent: svaki čvor može objaviti više grupa za dijeljenje datoteka. Nakon što se drugi čvorovi pridruže, mogu automatski dobiti dostupne početne čvorove od ravnopravnog poslužitelja i istovremeno ih povući iz više izvora. Nakon dovršetka sinkronizacije, automatski će se stvoriti početni elementi za više čvorova radi ubrzanja preuzimanja.

## Osnovni naglasci

- **Svatko je i poslužitelj i klijent**: usporedivo s Resilio Syncom, nema ovisnosti o središnjem poslužitelju. Svaki čvor može stvoriti višestruke grupe za dijeljenje datoteka, a također se može pridružiti grupama za dijeljenje drugih ljudi; čvorovi automatski razmjenjuju i dobivaju dostupne početne čvorove kako bi se postiglo istovremeno povlačenje i sinkronizacija više strojeva u stvarnom vremenu.
- **Adresa čvora bez IP-a**: Prema zadanim postavkama, ključ čvora i `base64url` kodirana `iroh` adresa automatski se generiraju u `~/.config/sync0` bez potrebe za pamćenjem ili izlaganjem javnog IP-a.
- **Sinkronizacija višestrukog odabira na zahtjev**: Podržava sinkronizaciju cijele grupe za dijeljenje, a također podržava sinkronizaciju na zahtjev odabirom više datoteka ili poddirektorija u isto vrijeme.
- **Hijerarhijska dopuštenja članova grupe i kontrola direktorija za učitavanje**: Podržava dopuštenja samo za čitanje (`ro`), učitavanje za čitanje i pisanje (`rw`) i upravljanje (`admin`), a može pojedinačno odrediti prefiks staze koji je svakom članu dopušteno učitavati (prema zadanim postavkama dopušteno je učitavanje u korijenski direktorij dijeljenja).
- **Puna i inkrementalna sinkronizacija preko mreža**: istovremeni prijenos velikih datoteka, agregacija ogromnih malih datoteka u memoriji i samo sinkronizirani blokovi podataka kada su datoteke djelomično izmijenjene.

## Usporedba brzine s rsync

### Stvarno mjerenje prekooceanske mreže iz jednog izvora (Kina → Sjedinjene Države, kašnjenje 260 ms)

| Scenarij prijenosa | Veličina podataka | model | `sync0` | `rsync -az` | Povećaj množitelj |
| :--- | :---: | :---: | :---: | :---: | :---: |
| 500 malih datoteka | 1.91 MB | Potpuna sinkronizacija | 18.84 Mbps | 5.32 Mbps | 3.5x |
| 500 malih datoteka | 1.91 MB | Inkrementalna sinkronizacija | 27.15 Mbps | 3.83 Mbps | 7.1x |
| Paketi koda i arhive (`.tar`) | 10.0 MB | Potpuna sinkronizacija | 139.70 Mbps | 24.97 Mbps | 5.6x |
| Paketi koda i arhive (`.tar`) | 10.0 MB | Inkrementalna sinkronizacija | 83.38 Mbps | 26.02 Mbps | 3.2x |
| Video datoteka (`.mp4`) | 15.0 MB | Potpuna sinkronizacija | 54.06 Mbps | 27.04 Mbps | 2.0x |
| Video datoteka (`.mp4`) | 15.0 MB | Inkrementalna sinkronizacija | 216.83 Mbps | 21.51 Mbps | 10.1x |

### Test povlačenja prekooceanske mreže P2P s više čvorova (Kina → dvostruki čvorovi Sjedinjenih Država, kašnjenje 260 ms)

Izmjerena usporedba prugastog istodobnog povlačenja s više izvora sjemenskog čvora:

| Scenarij prijenosa | Veličina podataka | Sijanje jednog čvora | P2P istodobno sijanje s dva čvora | Povećaj množitelj |
| :--- | :---: | :---: | :---: | :---: |
| 500 malih datoteka | 1.97 MB | 5.28 Mbps | 7.87 Mbps | 1.5x |
| Paketi koda i arhive (`.tar`) | 30.0 MB | 56.35 Mbps | 83.17 Mbps | 1.5x |
| Video datoteka (`.mp4`) | 30.0 MB | 27.00 Mbps | 39.46 Mbps | 1.5x |

### Stvarno mjerenje transkontinentalne namjenske linije velike brzine (Europa → Sjeverna Amerika, kašnjenje 86 ms)

| Scenarij prijenosa | Veličina podataka | `sync0` | `rsync` | Povećaj množitelj |
| :--- | :---: | :---: | :---: | :---: |
| 10 000 malih datoteka | 77.6 MB | 688 Mbps | 128 Mbps | 5.4x |
| Paketi koda i arhive (`.tar`) | 512 MB | 2111 Mbps | 438 Mbps | 4.8x |
| Video datoteka (`.mp4`) | 512 MB | 556 Mbps | 338 Mbps | 1.6x |
| Inkrementalna sinkronizacija (izmijenite 1% sadržaja) | 512 MB | 34133 Mbps | 2214 Mbps | 15.4x |

## Instalirati

### Metoda 1: Instalirajte putem Carga

```bash
cargo install sync0
```

### Metoda 2: preuzmite unaprijed kompajlirane binarne datoteke

Idite na [GitHub Releases](https://github.com/webc-site/sync0/releases) da preuzmete `.tar.xz` komprimirani paket za odgovarajući sustav (podržava Linux, macOS, Windows, Android, FreeBSD, NetBSD), raspakirajte ga i stavite u `PATH` direktorij:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Počnite brzo

`sync0` prema zadanim postavkama koristi `~/.config/sync0` kao konfiguracijski direktorij (ako se trebate prebaciti na druge direktorije, možete dodati `-c <directory>`).

### 1. Pogledajte adresu lokalnog čvora i posjetnicu postavki

```bash
# Pogledajte adresu lokalnog čvora (ključ se prvi put automatski generira u ~/.config/sync0)
sync0 id

# Generirajte osobnu posjetnicu čvora (podržava korisničko ime, grad, e-poštu, web stranicu i potpis unutar 256 riječi)
sync0 profile -u alice --region SHA --email alice@example.com --url https://example.com -b "Shanghai node"
```

### 2. Objavite više grupa za dijeljenje datoteka i postavite dopuštenja za članove grupe (`.nt` Konfiguracija)

`sync0` je konfiguriran u formatu [NestedText](https://nestedtext.org) (`.nt`). Svaki korisnik ima zasebnu `.nt` konfiguracijsku datoteku. Možete navesti direktorij korisničkog profila kroz `users_dir` u `config.nt` (što automatski uključuje sve `.nt` datoteke u ovom direktoriju). I korisnik i prefiks staze dopušteni za učitavanje koriste neuređeni popis (`-`). **Kada prefiks staze za učitavanje nije ispunjen, učitavanje u korijenski direktorij dijeljenja dopušteno je prema zadanim postavkama**.

1. Stvorite zasebne `.nt` konfiguracijske datoteke za svakog člana u `~/.config/sync0/users/` direktoriju:

`~/.config/sync0/users/alice.nt` (član samo za čitanje, ograničena stopa `80MB`, `rate` podržava jedinice `GB`, `MB`, `KB`):

```yaml
id: 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c
perm: ro
rate: 80MB
```

`~/.config/sync0/users/bob.nt` (članovi s ograničenim učitavanjem: dozvoljeno učitavanje samo u direktorije `uploads/bob` i `shared/common`):

```yaml
id: 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f
perm: rw
write_prefix:
  - uploads/bob
  - shared/common
```

`~/.config/sync0/users/charlie.nt` (Zadano učitavanje člana: Kada je `write_prefix` izostavljeno, učitavanje u korijenski direktorij dijeljenja dopušteno je prema zadanim postavkama):

```yaml
id: 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
perm: rw
```

2. Konfigurirajte korisnički direktorij i više grupa za dijeljenje datoteka u `~/.config/sync0/config.nt`:

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

Pokrenite uslugu dijeljenja:

```bash
# Izravno pokretanje (automatski učitava sve .nt korisničke konfiguracije u direktorije ~/.config/sync0/config.nt i users_dir)
sync0 share --region SHA

# Ili brzo objavite više grupa za dijeljenje i autorizirajte članove grupe izravno putem naredbenog retka
sync0 share dataset:./data team-vault:./vault \
  --auth \
  --allow-ro 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c \
  --allow-rw 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f:uploads/bob,shared/common \
  --allow-rw 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
```

### 3. Povucite sav sadržaj ili odaberite više datoteka i direktorija (`sync0 pull`)

Povežite se s bilo kojim ravnopravnim čvorom kako biste automatski dobili sve dostupne početne čvorove u grupi za dijeljenje i istovremeno povlačili iz više početnih čvorova:

```bash
# Povucite cijelu grupu za dijeljenje skupa podataka na lokalni ./local_data
sync0 povući skup podataka:// ./local_data -p <adresa čvora>

# Sinkronizacija s višestrukim odabirom: istovremeno povucite jednu datoteku docs/readme.md i poddirektorije assets/icons, src/core
sync0 pull dataset://docs/readme.md,assets/icons,src/core ./local_data -p <adresa čvora>
```

### 4. Prenesite datoteku ili direktorij u grupu za dijeljenje (`sync0 push`)

Članovi s dopuštenjima za učitavanje (`rw` ili `admin`) mogu učitavati lokalne datoteke ili direktorije u grupe za udaljeno dijeljenje:

```bash
# Članovi s dozvolom za učitavanje korijenskog direktorija: Učitajte izravno u zajednički korijenski direktorij
sync0 push team-vault:// ./new_release -p <adresa čvora>

# Samo članovi ovlašteni za određeni prefiks (kao što je uploads/bob): učitaj na dopušteni prefiks staze
sync0 push team-vault://uploads/bob/report.pdf ./report.pdf -p <adresa čvora>
```

### 5. Pretplatite se na automatsku sinkronizaciju i sijanje (`sync0 sub`)

Nakon pretplate na grupu za dijeljenje određenog čvora, lokalni poslužitelj je i klijent koji prima i novi poslužitelj za sijanje; kada se datoteka izvornog čvora ažurira, ona se automatski sinkronizira i pruža podijeljena preuzimanja za druge čvorove:

```bash
# Pretplatite se na cijelu grupu za dijeljenje i automatski pošaljite
sync0 sub dataset:// ./local_mirror -p <adresa čvora>

# Višestruki odabir samo za pretplatu na određene datoteke i poddirektorije i njihovo postavljanje
sync0 sub dataset://models/llama.onnx,configs ./local_mirror -p <adresa čvora>
```

### 6. Grupna ravnopravna sinkronizacija s više strojeva (`sync0 group`)

Više strojeva pridružuje se istoj grupi za dijeljenje, jedno drugo automatski otkriva dostupne početne čvorove i održava dvosmjernu sinkronizaciju (podržava potpunu sinkronizaciju ili višestruki odabir datoteka i direktorija):

```bash
# Potpuno se pridružite grupi i dvosmjerna sinkronizacija
sync0 group team-docs:// ./docs -p <adresa čvora>

# Samo direktorij wal i datoteka meta.json u grupi za sinkronizaciju višestrukog odabira
sync0 group team-docs://wal,meta.json ./docs -p <adresa čvora>
```

### 7. Izravan prijenos od točke do točke između dva računala (`send` / `recv`)

Nema potrebe unaprijed konfigurirati grupu i možete privremeno prenijeti datoteke ili direktorije izravno između dva računala:

```bash
# Praćenje prijemnika
sync0 recv 0.0.0.0:9000 ./backup

# Pošiljatelj izravno prenosi datoteke ili direktorije (opcionalno --delete mirror za čišćenje viška datoteka, --rate ograničenje brzine)
sync0 send ./project 192.168.1.10:9000
```

### 8. Prebacivanje na više jezika

`sync0` podržava 75 jezika i automatski se prebacuje prema zadanom jezičnom okruženju sustava; također se može odrediti ručno kroz `SYNC0_LANG`:

```bash
SYNC0_LANG=en sync0 --help
SYNC0_LANG=ja sync0 --help
SYNC0_LANG=de sync0 --help
```