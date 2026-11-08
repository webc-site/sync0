# sync0

`sync0` je peer-to-peer (P2P) alat za dijeljenje datoteka i sinhronizaciju na više uređaja koji se takmiči sa Resilio Sync.

Svaka mašina je i server i klijent: svaki čvor može objaviti više grupa za dijeljenje datoteka. Nakon što se drugi čvorovi pridruže, oni mogu automatski dobiti dostupne početne čvorove od ravnopravnog servera i istovremeno ih povući iz više izvora. Nakon što se sinhronizacija završi, seeds će se automatski kreirati za više čvorova kako bi se ubrzala preuzimanja.

## Core Highlights

- **Svi su i serveri i klijenti**: Uporedivo sa Resilio Sync, nema zavisnosti od centralnog servera. Svaki čvor može kreirati više grupa za dijeljenje datoteka, a može se pridružiti i grupama za dijeljenje drugih ljudi; čvorovi automatski razmjenjuju i dobijaju dostupne sjemenske čvorove kako bi se postiglo istovremeno povlačenje i sinhronizacija više mašina u realnom vremenu.
- **Adresa čvora bez IP-a**: Podrazumevano, ključ čvora i `base64url` kodirana `iroh` adresa se automatski generiraju u `~/.config/sync0` bez potrebe za pamćenjem ili otkrivanjem javne IP adrese.
- **Sinhronizacija sa više izbora na zahtjev**: Podržava sinhronizaciju cijele grupe za dijeljenje, a podržava i sinhronizaciju na zahtjev odabirom više datoteka ili poddirektorija u isto vrijeme.
- **Hijerarhijske dozvole članova grupe i kontrola direktorija za otpremanje**: Podržava dozvole samo za čitanje (`ro`), prijenos za čitanje-upisivanje (`rw`) i upravljanje (`admin`) i može pojedinačno odrediti prefiks putanje koji je svakom članu dozvoljeno učitavanje (prema zadanim postavkama, otpremanje dijeljenja je dozvoljeno u root direktorij).
- **Puna brzina i inkrementalna sinhronizacija kroz mreže**: Istovremeni prenos velikih datoteka, agregacija velikih malih datoteka u memoriji i sinhronizovani blokovi podataka samo kada su fajlovi delimično modifikovani.

## Poređenje brzine sa rsync

### Stvarno mjerenje prekooceanske mreže iz jednog izvora (Kina → Sjedinjene Američke Države, kašnjenje 260 ms)

| Scenario prijenosa | Veličina podataka | model | `sync0` | `rsync -az` | Povećajte množitelj |
| :--- | :---: | :---: | :---: | :---: | :---: |
| 500 malih fajlova | 1.91 MB | Potpuna sinhronizacija | 18.84 Mbps | 5.32 Mbps | 3.5x |
| 500 malih fajlova | 1.91 MB | Inkrementalna sinhronizacija | 27.15 Mbps | 3.83 Mbps | 7.1x |
| Kod i arhivski paketi (`.tar`) | 10.0 MB | Potpuna sinhronizacija | 139.70 Mbps | 24.97 Mbps | 5.6x |
| Kod i arhivski paketi (`.tar`) | 10.0 MB | Inkrementalna sinhronizacija | 83.38 Mbps | 26.02 Mbps | 3.2x |
| Video fajl (`.mp4`) | 15.0 MB | Potpuna sinhronizacija | 54.06 Mbps | 27.04 Mbps | 2.0x |
| Video fajl (`.mp4`) | 15.0 MB | Inkrementalna sinhronizacija | 216.83 Mbps | 21.51 Mbps | 10.1x |

### Transoceanska mreža P2P test povlačenja s više čvorova (Kina → Sjedinjene Američke Države dvostruki čvorovi, kašnjenje 260 ms)

Izmjereno poređenje istodobnog povlačenja prugastog sjemenskog čvora sa više izvora:

| Scenario prijenosa | Veličina podataka | Sijanje jednog čvora | P2P istovremeno zasijavanje sa dva čvora | Povećajte množitelj |
| :--- | :---: | :---: | :---: | :---: |
| 500 malih fajlova | 1.97 MB | 5.28 Mbps | 7.87 Mbps | 1.5x |
| Kod i arhivski paketi (`.tar`) | 30.0 MB | 56.35 Mbps | 83.17 Mbps | 1.5x |
| Video fajl (`.mp4`) | 30.0 MB | 27.00 Mbps | 39.46 Mbps | 1.5x |

### Stvarno mjerenje transkontinentalne namjenske linije velike brzine (Evropa → Sjeverna Amerika, kašnjenje 86 ms)

| Scenario prijenosa | Veličina podataka | `sync0` | `rsync` | Povećajte množitelj |
| :--- | :---: | :---: | :---: | :---: |
| 10.000 malih fajlova | 77.6 MB | 688 Mbps | 128 Mbps | 5.4x |
| Kod i arhivski paketi (`.tar`) | 512 MB | 2111 Mbps | 438 Mbps | 4.8x |
| Video fajl (`.mp4`) | 512 MB | 556 Mbps | 338 Mbps | 1.6x |
| Inkrementalna sinhronizacija (izmijeniti 1% sadržaja) | 512 MB | 34133 Mbps | 2214 Mbps | 15.4x |

## Instaliraj

### Metoda 1: Instalirajte preko Cargo-a

```bash
cargo install sync0
```

### Metoda 2: Preuzmite unaprijed kompajlirane binarne datoteke

Idite na [GitHub Releases](https://github.com/webc-site/sync0/releases) da preuzmete `.tar.xz` komprimirani paket za odgovarajući sistem (podržava Linux, macOS, Windows, Android, FreeBSD, NetBSD), raspakirajte i stavite ga u `PATH` direktorij:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Započnite brzo

`sync0` po defaultu koristi `~/.config/sync0` kao konfiguracijski direktorij (ako se trebate prebaciti na druge direktorije, možete dodati `-c <direktorij>`).

### 1. Pogledajte adresu lokalnog čvora i vizitkartu podešavanja

```bash
# Pogledajte adresu lokalnog čvora (ključ se automatski generira u ~/.config/sync0 po prvi put)
sync0 id

# Generirajte osobnu vizit kartu (podržava korisničko ime, grad, e-mail, web stranicu i potpis unutar 256 riječi)
sync0 profile -u alice --region SHA --email alice@example.com --url https://example.com -b "Shanghai node"
```

### 2. Objavite više grupa za dijeljenje datoteka i postavite dozvole za članove grupe (`.nt` Konfiguracija)

`sync0` je konfigurisan u formatu [NestedText](https://nestedtext.org) (`.nt`). Svaki korisnik ima zaseban `.nt` konfiguracijski fajl. Možete odrediti direktorij korisničkog profila kroz `users_dir` u `config.nt` (koji automatski uključuje sve `.nt` datoteke u ovom direktoriju). I korisnik i prefiks putanje kojima je dozvoljeno učitavanje koriste neuređenu listu (`-`). **Kada prefiks putanje za otpremanje nije popunjen, otpremanje u korijenski direktorij dijeljenja je dozvoljeno prema zadanim postavkama**.

1. Kreirajte zasebne `.nt` konfiguracijske datoteke za svakog člana u `~/.config/sync0/users/` direktoriju:

`~/.config/sync0/users/alice.nt` (član samo za čitanje, ograničena stopa `80MB`, `rate` podržava jedinice `GB`, `MB`, `KB`):

```yaml
id: 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c
perm: ro
rate: 80MB
```

`~/.config/sync0/users/bob.nt` (članovi s ograničenim učitavanjem: dozvoljeno samo učitavanje u direktorije `uploads/bob` i `shared/common`):

```yaml
id: 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f
perm: rw
write_prefix:
  - uploads/bob
  - shared/common
```

`~/.config/sync0/users/charlie.nt` (Zadano otpremanje člana: Kada je `write_prefix` izostavljen, otpremanje u korijenski direktorij dijeljenja je dozvoljeno prema zadanim postavkama):

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

Počnite dijeliti uslugu:

```bash
# Direktno pokretanje (automatski učitava sve .nt korisničke konfiguracije u direktorijima ~/.config/sync0/config.nt i users_dir)
sync0 share --region SHA

# Ili brzo objavite više grupa za dijeljenje i ovlastite članove grupe direktno preko komandne linije
sync0 share dataset:./data team-vault:./vault \
  --auth \
  --allow-ro 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c \
  --allow-rw 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f:uploads/bob,shared/common \
  --allow-rw 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
```

### 3. Povucite sav sadržaj ili odaberite više datoteka i direktorija (`sync0 pull`)

Povežite se na bilo koji ravnopravni čvor da automatski dobijete sve dostupne polazne čvorove u grupi za dijeljenje i istovremeno povlačite iz više početnih čvorova:

```bash
# Povucite cijelu grupu za dijeljenje skupa podataka na lokalni ./local_data
sync0 pull dataset:// ./local_data -p <adresa čvora>

# Sinhronizacija sa višestrukim odabirom: istovremeno povucite jednu datoteku docs/readme.md i poddirektorije imovine/ikone, src/core
sync0 pull dataset://docs/readme.md,assets/icons,src/core ./local_data -p <adresa čvora>
```

### 4. Prenesite datoteku ili direktorij u grupu za dijeljenje (`sync0 push`)

Članovi s dozvolama za prijenos (`rw` ili `admin`) mogu učitavati lokalne datoteke ili direktorije u grupe za udaljeno dijeljenje:

```bash
# Članovi s dozvolom za prijenos korijenskog direktorija: Prenesite direktno u zajednički korijenski direktorij
sync0 push team-vault:// ./new_release -p <adresa čvora>

# Samo članovi ovlašteni za određeni prefiks (kao što je uploads/bob): učitavanje na dozvoljeni prefiks putanje
sync0 push team-vault://uploads/bob/report.pdf ./report.pdf -p <adresa čvora>
```

### 5. Pretplatite se na automatsku sinhronizaciju i postavljanje (`sync0 sub`)

Nakon što se pretplatite na grupu za dijeljenje određenog čvora, lokalni server je istovremeno i klijent koji prima i novi server; kada se datoteka izvornog čvora ažurira, automatski se sinkronizira i pruža podijeljena preuzimanja za druge čvorove:

```bash
# Pretplatite se na cijelu grupu za dijeljenje i automatski započnite
sync0 sub dataset:// ./local_mirror -p <adresa čvora>

# Samo višestruki selektujte da biste se pretplatili na određene datoteke i poddirektorije i zadali ih
sync0 sub dataset://models/llama.onnx,configs ./local_mirror -p <adresa čvora>
```

### 6. Sinkronizacija grupe više mašina (`sync0 group`)

Više mašina se pridružuje istoj grupi za dijeljenje, međusobno automatski otkrivaju dostupne izvorne čvorove i održavaju dvosmjernu sinhronizaciju (podržava punu sinhronizaciju ili višestruki odabir datoteka i direktorija):

```bash
# Potpuno se pridružite grupi i dvosmjerna sinhronizacija
sync0 group team-docs:// ./docs -p <adresa čvora>

# Samo direktorij wal i meta.json datoteka u grupi za sinhronizaciju s višestrukim odabirom
sync0 group team-docs://wal,meta.json ./docs -p <adresa čvora>
```

### 7. Direktan prijenos od tačke do tačke između dvije mašine (`send` / `recv`)

Nema potrebe da unapred konfigurišete grupu i možete privremeno prenositi datoteke ili direktorijume direktno između dva računara:

```bash
# Monitoring prijemnika
sync0 recv 0.0.0.0:9000 ./backup

# Kraj koji šalje direktno prenosi fajlove ili direktorijume (opciono -- izbrišite ogledalo da biste očistili višak fajlova, --rate ograničenje brzine)
sync0 send ./project 192.168.1.10:9000
```

### 8. Prebacivanje na više jezika

`sync0` podržava 75 jezika i automatski se prebacuje u skladu sa jezičkim okruženjem sistema prema zadanim postavkama; također se može odrediti ručno putem `SYNC0_LANG`:

```bash
SYNC0_LANG=en sync0 --help
SYNC0_LANG=ja sync0 --help
SYNC0_LANG=de sync0 --help
```