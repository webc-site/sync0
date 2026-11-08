# sync0

`sync0` je orodje za skupno rabo datotek enakovrednim (P2P) in orodje za sinhronizacijo več strojev, ki tekmuje z Resilio Sync.

Vsak stroj je hkrati strežnik in odjemalec: vsako vozlišče lahko objavi več skupin za skupno rabo datotek. Ko se druga vozlišča pridružijo, lahko samodejno pridobijo razpoložljiva semenska vozlišča iz enakovrednega strežnika in jih hkrati potegnejo iz več virov. Po končani sinhronizaciji bodo samodejno ustvarjena semena za več vozlišč za pospešitev prenosov.

## Osnovni poudarki

- **Vsakdo je hkrati strežnik in odjemalec**: Primerljivo z Resilio Sync, brez odvisnosti od centralnega strežnika. Vsako vozlišče lahko ustvari več skupin za skupno rabo datotek in se lahko tudi pridruži skupinam za skupno rabo drugih ljudi; vozlišča samodejno izmenjujejo in pridobivajo razpoložljiva semenska vozlišča, da dosežejo sočasno vlečenje in sinhronizacijo več strojev v realnem času.
- **Naslov vozlišča brez IP-ja**: Privzeto sta ključ vozlišča in `base64url` kodiran naslov `iroh` samodejno ustvarjen v `~/.config/sync0`, ne da bi si bilo treba zapomniti ali izpostaviti javni IP.
- **Sinhronizacija z več izbirami na zahtevo**: podpira sinhronizacijo celotne skupine za skupno rabo in podpira tudi sinhronizacijo na zahtevo z izbiro več datotek ali podimenikov hkrati.
- **Hirarhična dovoljenja članov skupine in nadzor imenika za nalaganje**: podpira dovoljenja samo za branje (`ro`), nalaganje za branje in pisanje (`rw`) in upravljanje (`admin`) ter lahko posamezno določi predpono poti, ki jo lahko vsak član naloži (privzeto je dovoljeno nalaganje v korenski imenik skupne rabe).
- **Sinhronizacija s polno hitrostjo in postopna sinhronizacija v omrežjih**: sočasni prenos velikih datotek, združevanje ogromnih majhnih datotek v pomnilniku in samo sinhronizirani podatkovni bloki, ko so datoteke delno spremenjene.

## Primerjava hitrosti z rsync

### Dejanska meritev čezoceanskega omrežja z enim virom (Kitajska → Združene države, zakasnitev 260 ms)

| Scenarij prenosa | Velikost podatkov | model | `sync0` | `rsync -az` | Povečanje množitelja |
| :--- | :---: | :---: | :---: | :---: | :---: |
| 500 majhnih datotek | 1.91 MB | Popolna sinhronizacija | 18.84 Mbps | 5.32 Mbps | 3.5x |
| 500 majhnih datotek | 1.91 MB | Postopna sinhronizacija | 27.15 Mbps | 3.83 Mbps | 7.1x |
| Kodni in arhivski paketi (`.tar`) | 10.0 MB | Popolna sinhronizacija | 139.70 Mbps | 24.97 Mbps | 5.6x |
| Kodni in arhivski paketi (`.tar`) | 10.0 MB | Postopna sinhronizacija | 83.38 Mbps | 26.02 Mbps | 3.2x |
| Video datoteka (`.mp4`) | 15.0 MB | Popolna sinhronizacija | 54.06 Mbps | 27.04 Mbps | 2.0x |
| Video datoteka (`.mp4`) | 15.0 MB | Postopna sinhronizacija | 216.83 Mbps | 21.51 Mbps | 10.1x |

### Test vlečenja čezoceanskega omrežja P2P z več vozlišči (Kitajska → dvojna vozlišča Združenih držav, zakasnitev 260 ms)

Izmerjena primerjava črtastega sočasnega vlečenja semenskega vozlišča z več viri:

| Scenarij prenosa | Velikost podatkov | Sejanje z enim vozliščem | Sočasno sejanje P2P z dvema vozliščema | Povečanje množitelja |
| :--- | :---: | :---: | :---: | :---: |
| 500 majhnih datotek | 1.97 MB | 5.28 Mbps | 7.87 Mbps | 1.5x |
| Kodni in arhivski paketi (`.tar`) | 30.0 MB | 56.35 Mbps | 83.17 Mbps | 1.5x |
| Video datoteka (`.mp4`) | 30.0 MB | 27.00 Mbps | 39.46 Mbps | 1.5x |

### Dejanska meritev transkontinentalne namenske linije za visoke hitrosti (Evropa → Severna Amerika, zakasnitev 86 ms)

| Scenarij prenosa | Velikost podatkov | `sync0` | `rsync` | Povečanje množitelja |
| :--- | :---: | :---: | :---: | :---: |
| 10.000 majhnih datotek | 77.6 MB | 688 Mbps | 128 Mbps | 5.4x |
| Kodni in arhivski paketi (`.tar`) | 512 MB | 2111 Mbps | 438 Mbps | 4.8x |
| Video datoteka (`.mp4`) | 512 MB | 556 Mbps | 338 Mbps | 1.6x |
| Prirastna sinhronizacija (spremeni 1 % vsebine) | 512 MB | 34133 Mbps | 2214 Mbps | 15.4x |

## Namestite

### 1. način: namestitev prek Cargo

```bash
cargo install sync0
```

### 2. način: Prenesite vnaprej prevedene binarne datoteke

Pojdite na [GitHub Releases](https://github.com/webc-site/sync0/releases), da prenesete `.tar.xz` stisnjen paket za ustrezen sistem (podpira Linux, macOS, Windows, Android, FreeBSD, NetBSD), razpakirajte in ga postavite v imenik `PATH`:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Hitro začnite

`sync0` privzeto uporablja `~/.config/sync0` kot konfiguracijski imenik (če morate preklopiti na druge imenike, lahko dodate `-c <imenik>`).

### 1. Oglejte si naslov lokalnega vozlišča in nastavite vizitko

```bash
# Oglejte si naslov lokalnega vozlišča (ključ se prvič samodejno ustvari v ~/.config/sync0)
sync0 id

# Ustvari osebno vizitko vozlišča (podpira uporabniško ime, mesto, e-pošto, spletno mesto in podpis v 256 besedah)
sync0 profile -u alice --region SHA --email alice@example.com --url https://example.com -b "Shanghai node"
```

### 2. Objavite več skupin za skupno rabo datotek in nastavite dovoljenja za člane skupine (`.nt` Konfiguracija)

`sync0` je konfiguriran v formatu [NestedText](https://nestedtext.org) (`.nt`). Vsak uporabnik ima ločeno `.nt` konfiguracijsko datoteko. Imenik uporabniškega profila lahko določite prek `users_dir` v `config.nt` (kar samodejno vključuje vse `.nt` datoteke v tem imeniku). Uporabnik in predpona poti, ki ju je dovoljeno naložiti, uporabljata neurejen seznam (`-`). **Če predpona poti za nalaganje ni izpolnjena, je nalaganje v korenski imenik skupne rabe privzeto dovoljeno**.

1. Ustvarite ločene konfiguracijske datoteke `.nt` za vsakega člana v imeniku `~/.config/sync0/users/`:

`~/.config/sync0/users/alice.nt` (član samo za branje, omejena stopnja `80MB`, `rate` podpira `GB`, `MB`, `KB` enot):

```yaml
id: 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c
perm: ro
rate: 80MB
```

`~/.config/sync0/users/bob.nt` (člani z omejenim nalaganjem: dovoljeno samo nalaganje v `uploads/bob` in `shared/common` imenika):

```yaml
id: 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f
perm: rw
write_prefix:
  - uploads/bob
  - shared/common
```

`~/.config/sync0/users/charlie.nt` (Privzeto nalaganje članov: Če je `write_prefix` izpuščeno, je nalaganje v korenski imenik skupne rabe privzeto dovoljeno):

```yaml
id: 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
perm: rw
```

2. Konfigurirajte uporabniški imenik in več skupin za skupno rabo datotek v `~/.config/sync0/config.nt`:

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

Začetek storitve skupne rabe:

```bash
# Neposredni zagon (samodejno naloži vse uporabniške konfiguracije .nt v imenike ~/.config/sync0/config.nt in users_dir)
sync0 share --region SHA

# Ali pa hitro objavite več skupin za skupno rabo in pooblastite člane skupine neposredno prek ukazne vrstice
sync0 share dataset:./data team-vault:./vault \
  --auth \
  --allow-ro 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c \
  --allow-rw 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f:uploads/bob,shared/common \
  --allow-rw 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
```

### 3. Povlecite vso vsebino ali izberite več datotek in imenikov (`sync0 pull`)

Povežite se s poljubnim enakovrednim vozliščem, da samodejno pridobite vsa razpoložljiva semenska vozlišča v skupini za skupno rabo in vlečete iz več semenskih vozlišč hkrati:

```bash
# Potegnite celotno skupino za skupno rabo nabora podatkov v lokalni ./local_data
sync0 pull dataset:// ./local_data -p <naslov vozlišča>

# Sinhronizacija z več izbirami: istočasno potegnite eno datoteko docs/readme.md in podimenike, sredstva/ikone, src/core
sync0 pull dataset://docs/readme.md,assets/icons,src/core ./local_data -p <naslov vozlišča>
```

### 4. Prenesite datoteko ali imenik v skupino za skupno rabo (`sync0 push`)

Člani z dovoljenji za nalaganje (`rw` ali `admin`) lahko naložijo lokalne datoteke ali imenike v skupine za oddaljeno skupno rabo:

```bash
# Člani z dovoljenjem za nalaganje v korenski imenik: nalaganje neposredno v korenski imenik v skupni rabi
sync0 push team-vault:// ./new_release -p <naslov vozlišča>

# Samo člani, pooblaščeni za določeno predpono (kot je nalaganje/bob): nalaganje na dovoljeno predpono poti
sync0 push team-vault://uploads/bob/report.pdf ./report.pdf -p <naslov vozlišča>
```

### 5. Naročite se na samodejno sinhronizacijo in sejanje (`sync0 sub`)

Po vpisu v skupino za skupno rabo določenega vozlišča je lokalni strežnik hkrati sprejemni odjemalec in nov sejalni strežnik; ko je datoteka izvornega vozlišča posodobljena, se samodejno sinhronizira in zagotavlja prenose v kosih za druga vozlišča:

```bash
# Naročite se na celotno skupino za skupno rabo in samodejno seme
sync0 sub dataset:// ./local_mirror -p <naslov vozlišča>

# Večkratna izbira samo za naročanje na določene datoteke in podimenike ter njihovo seme
sync0 sub dataset://models/llama.onnx,configs ./local_mirror -p <naslov vozlišča>
```

### 6. Sinhronizacija enakovrednih skupin več strojev (`sync0 group`)

Več strojev se pridruži isti skupini za skupno rabo, drug drugi samodejno odkrije razpoložljiva semenska vozlišča in vzdržuje dvosmerno sinhronizacijo (podpira popolno sinhronizacijo ali večkratno izbiro datotek in imenikov):

```bash
# Popolna pridružitev skupini in dvosmerna sinhronizacija
skupina sync0 team-docs:// ./docs -p <naslov vozlišča>

# Samo imenik wal in datoteka meta.json v sinhronizacijski skupini z več izbirami
skupina sync0 team-docs://wal,meta.json ./docs -p <naslov vozlišča>
```

### 7. Neposredni prenos od točke do točke med dvema napravama (`send` / `recv`)

Ni vam treba predhodno konfigurirati skupine in lahko začasno prenašate datoteke ali imenike neposredno med dvema računalnikoma:

```bash
# Spremljanje sprejemnika
sync0 recv 0.0.0.0:9000 ./backup

# Pošiljatelj neposredno prenese datoteke ali imenike (izbirno --delete mirror za čiščenje odvečnih datotek, --rate omejitev hitrosti)
sync0 send ./project 192.168.1.10:9000
```

### 8. Preklapljanje med več jeziki

`sync0` podpira 75 jezikov in privzeto samodejno preklaplja glede na jezikovno okolje sistema; lahko ga določite tudi ročno prek `SYNC0_LANG`:

```bash
SYNC0_LANG=en sync0 --help
SYNC0_LANG=ja sync0 --help
SYNC0_LANG=de sync0 --help
```