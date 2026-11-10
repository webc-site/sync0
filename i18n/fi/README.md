# sync0

`sync0` on hajautettu P2P-tiedostonjako-, reaaliaikainen synkronointi- ja usean lähteen jakelutyökalu, jota käytetään korvaamaan `rsync` - mannertenvälisen WAN-verkon mitattu nopeus on jopa `rsync`:n **5,89×** (yhden pisteen geometrinen keskinopeus **2,67×**), P2P.16× **2,67×**, P2P. valtameren korkean latenssin linkin P2P-kiihtyvyys on jopa **7,68×** (täyden linkin 24 skenaarion kattava geometrinen keskinopeus **2,56×**).

Jokainen kone on sekä palvelin että asiakas: ei tarvita keskuspalvelinta tai julkista verkko-osoitetta, automaattinen rei'itys perustuu hajautetun verkon julkisen avaimen osoitteeseen, ja se tukee suoraa tiedonsiirtoa kahden koneen välillä, usean jakamisen ryhmäluvan isännöintiä, useampaa alihakemistojen valintaa tarpeen mukaan, millisekuntitason reaaliaikaista inkrementaalista synkronointia ja P2P-monilähdeyhteyttä.

## Asentaa

### Tapa 1: Lastin asennus

```bash
cargo install sync0
```

### Tapa 2: Lataa esikäännetyt binaarit

Siirry kohtaan [GitHub Releases](https://github.com/webc-site/sync0/releases) ladataksesi `.tar.xz`-pakatun paketin, joka vastaa järjestelmää (tukee Linuxia, macOS:ää, Windowsia, Androidia, iOS:ää, FreeBSD:tä), pura ja laita `PATH`:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Aloita nopeasti

### 1. Nollakonfiguraatio suora lähetys kahden koneen välillä (`send` / `recv`)

Ei tarvitse määrittää jakoryhmää, siirtää tiedostoja tai hakemistoja suoraan kahden koneen välillä suurella nopeudella:

```bash
# Vastaanottava pää kuuntelee porttia ja kirjoittaa tiedostoon ./backup
sync0 recv 0.0.0.0:9000 ./backup

# Suora lähetys lähettäjältä (valinnainen --poista peili ylimääräisten tiedostojen puhdistamiseksi, --nopeus 50 Mt nopeusrajoitus)
sync0 send ./project 192.168.1.10:9000
```

### 2. Viestien jakaminen ja pyynnöstä veto/lataus (`share` / `pull` / `push`)

```bash
# Palvelin: Julkaise jakamisryhmä yhdellä komentorivillä (lähetä IP-vapaa iroh-solmun osoite käynnistyksen jälkeen)
sync0 share dataset:./data

# Asiakas: Vedä koko tietojoukon jakamisryhmä paikalliseen ./local_data-tiedostoon
sync0 pull dataset:// ./local_data -p <solmun osoite>

# Asiakas: Vedä useita valintoja pyynnöstä (synkronoi vain tietyt yksittäiset tiedostot ja alihakemistot)
sync0 pull dataset://docs/readme.md,assets/icons,src/core ./local_data -p <solmun osoite>

# Asiakas: Lataa tiedostoja tai hakemistoja valtuutettuihin jakamisryhmiin
sync0 push dataset://releases/v1 ./build_output -p <solmun osoite>
```

### 3. Reaaliaikainen tilauskylvö ja usean koneen kaksisuuntainen synkronointi (`sub` / `group`)

Pitkän yhteyden muodostamisen jälkeen, jos etäpäässä tapahtuu tiedostoa **lisäämällä, muokkaamalla, poistamalla, muuttamalla käyttöoikeuksia tai muokkausaikaa**, paikallinen hakemisto päivitetään synkronisesti millisekunneissa reaaliajassa (uudet/muokatut suuret tiedostot keräävät vain erotuslohkot ja päivittävät ne paikoilleen, tiedostojen poistaminen puhdistaa automaattisesti paikalliset tiedostot ja tyhjät hakemistot, ja vain muuttaa käyttöoikeuksia):

- **`sync0 sub`(Yksisuuntainen reaaliaikainen peilaus + automaattinen kylvö)**: Paikallinen reaaliaikainen seuraa jakamispään lisäyksiä, poistoja ja muutoksia ja toimii automaattisesti P2P-siemenlähteenä tarjotakseen samanaikaista kiihdytystä muille vetosolmuille.
- **`sync0 group`(Monen koneen välinen kaksisuuntainen reaaliaikainen synkronointi)**: Jos tiedostoja lisätään, poistetaan tai muokataan missä tahansa ryhmän koneessa, kaikki muut online-solmut päivitetään synkronisesti reaaliajassa ja toimivat siemeninä toisilleen.

```bash
# Tilaa koko jakamisryhmä: etälisäykset, -poistot ja -muutokset synkronoidaan paikallisesti reaaliajassa ja aloitetaan automaattisesti
sync0 alitietojoukko:// ./local_mirror -p <solmun osoite>

# On-demand monivalintatilaus: synkronoi ja siirrä vain tietyt yksittäiset tiedostot ja alihakemistot reaaliajassa
sync0 alitietojoukko://models/llama.onnx,configs ./local_mirror -p <solmun osoite>

# Monen koneen vertaisryhmän kaksisuuntainen reaaliaikainen synkronointi (tukee myös täydellisiä tai pilkullisia monivalintapolkuja)
sync0 ryhmä team-docs:// ./docs -p <solmun osoite>
```

### 4. Ryhmän usean jakamisen ryhmä ja jäsenten käyttöoikeudet (`.nt` määritykset)

Oletusasetushakemisto on `~/.config/sync0` (voidaan vaihtaa komennolla `-c <hakemisto>`) käyttämällä minimalistista [NestedText](https://nestedtext.org) (`.nt`) -muotoa:
- **Tukee julkisen verkon lukemisen rajoittamista (vain valtuutetut käyttäjät voivat lukea)**: Niin kauan kuin jakamisryhmässä on `users:` jäsenluettelo (tai komentorivillä on `--allow-r` / `--allow-rw` / `--allow-w` / `--auth`), järjestelmä ottaa automaattisesti käyttöön **id="on" ja** julkisen todennuksen (">de) luettelointi tai lukeminen ja P2P-tilaus kylvösolmuihin perii myös sallittujen luettelon samanaikaisesti, jolloin vain valtuutetut käyttäjät voivat lukea; jakamisryhmät, joita ei ole määritetty `users`:lla, ovat oletuksena julkisia ja vain luku -muotoisia (jos haluat olla julkisia ja vain luku -tilassa ja avoinna vain määritetyille jäsenille, voit määrittää nimenomaisesti `auth: false`).
- **Hienorakeiset luku-kirjoitus- ja alihakemiston toimitusoikeudet**: Jäsenet ovat oletusarvoisesti vain luku -oikeuksia (`r` ei tarvitse kirjoittaa erikseen), ja ne tukevat `r` (vain luku), `w` (kirjoita vain pudotuslaatikkoon, ei luettavissa), `rw` (suora aliarvo="5") ja `rw` (read-b it idrite) lataamista varten).

**Jäsenmääritysesimerkki (`~/.config/sync0/users/`):**

```yaml
# ~/.config/sync0/users/alice.nt (oletus vain luku, nopeusrajoitus 80 Mt)
id: 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c
rate: 80MB
```

```yaml
# ~/.config/sync0/users/bob.nt (luku- ja kirjoitusoikeudet, sallittu vain ladata määritettyyn alihakemistoon)
id: 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f
perm: rw
write_prefix:
  - uploads/bob
  - shared/common
```

```yaml
# ~/.config/sync0/users/charlie.nt (vain kirjoitusoikeus, lukeminen kielletty)
id: 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
perm: w
```

**Jakoryhmän määritysesimerkki (`~/.config/sync0/config.nt`):**

```yaml
users_dir: ./users

groups:
  # Määrittämättömät käyttäjät: oletusarvoinen julkinen verkko, vain luku
  dataset:
    path: ./data
    rate: 50MB

  # Määritetyt käyttäjät: rajoittaa automaattisesti julkisen verkon lukemista (vain alice ja bob voivat lukea, charlie voi vain toimittaa ja ladata)
  team-vault:
    path: ./vault
    users:
      - alice
      - bob
      - charlie
```

Käynnistä palvelu (lataa asetustiedosto tai käytä suoraan komentoriviparametreja rajoittaaksesi julkisen verkon lukemista ja valtuutaksesi jäseniä):

```bash
# Aloita lataamalla ~/.config/sync0/config.nt
sync0 share --region SHA

# Tai julkaise yksityinen jakoryhmä puhtaalla komentorivillä (määritä --allow-* rajoittaaksesi automaattisesti julkisen verkon lukemista ja vain valtuutetut käyttäjät voivat käyttää sitä)
sync0 share team-vault:./vault \
  --allow-r 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c \
  --allow-rw 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f:uploads/bob,shared/common \
  --allow-w 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
```

### 5. Solmuosoite, käyntikortti ja monikielinen

```bash
# Näytä paikallinen IP-vapaa solmuosoite (avain luodaan automaattisesti ensimmäistä kertaa)
sync0 id

# Luo itse allekirjoitettu Ed25519 solmun käyntikortti (lempinimi, kaupunkikoodi, sähköposti, verkkosivusto, esittely)
sync0 profile -u alice --region SHA --email alice@example.com -b "Shanghai node"

# Sisäänrakennettuja kieliä on 75, jotka seuraavat oletusarvoisesti järjestelmän kieltä ja joita voidaan vaihtaa ympäristömuuttujien kautta.
SYNC0_LANG=en sync0 --help
```

## Suorituskyky ja ydinominaisuudet

### Mannerten välinen WAN:n todellinen mittaus (vertailu `rsync`)

Todellinen mittausympäristö: Yhdysvallat `c1` → Eurooppa `n1` (RTT 65 ms), tietojoukko on Linux-ydin `linux-7.3.3`:

#### 1) A la carte -tila vs. `rsync`

| Tietojoukko | koko | `sync0` Yksi piste<br>(Mbps) | `rsync`<br>(Mbps) | Nopeus (`sync0` / `rsync`) |
| :--- | ---: | ---: | ---: | ---: |
| `linux-7.3.3.tar.xz`(ei pakattavissa) | 155 MB | **236.48** | 108.74 | **2,17×** |
| `linux-7.3.3.tar`(pakkattavissa) | 1.6 GB | **843.83** | 385.06 | **2,19×** |
| `linux-7.3.3.tar`(paikallinen muutoksen lisäys) | 1.6 GB | **8,014.15** | 1,361.72 | **5,89×** |
| `linux-7.3.3/` (95 979 pientä tiedostoa) | 1.5 GB | **627.58** | 347.80 | **1,80×** |
| **geometrinen keskiarvo** | — | **1,000.91** | 375.26 | **2,67×** |

#### 2) P2P-kaksoislähdetila vs. `rsync`

| Tietojoukko | koko | `sync0` P2P-kaksoislähde<br>(Mbps) | `rsync`<br>(Mbps) | Nopeus (`sync0` / `rsync`) |
| :--- | ---: | ---: | ---: | ---: |
| `linux-7.3.3.tar.xz`(ei pakattavissa) | 155 MB | **330.09** | 108.74 | **3,04×** |
| `linux-7.3.3.tar`(pakkattavissa) | 1.6 GB | **1,551.86** | 385.06 | **4,03×** |
| `linux-7.3.3.tar`(paikallinen muutoksen lisäys) | 1.6 GB | **5,474.17** | 1,361.72 | **4,02×** |
| `linux-7.3.3/` (95 979 pientä tiedostoa) | 1.5 GB | **706.56** | 347.80 | **2,03×** |
| **geometrinen keskiarvo** | — | **1,186.42** | 375.26 | **3,16×** |

> Huomautus: P2P-kaksoislähdetilassa eurooppalaiset solmut `s1` tekevät yhteistyötä kylvössä, ja mannertenvälisen runkoverkon RTT on 65 ms.

### Tuotteen ydinominaisuudet

- **Julkisen avaimen osoitteen automaattinen rei'itys**: Ei tarvita keskuspalvelinta ja julkista verkko-osoitetta, automaattinen NAT-rei'itys perustuu hajautetun verkon julkisen avaimen osoitteeseen.
- **Millisekuntitason reaaliaikainen synkronointi ja metatietojen tietoisuus**: Perustuu natiivitiedostojen tapahtumien seurantaan (`FSEvents` / `inotify` / `ReadDirectoryChangesW`), tiedostojen lisäysten, poistojen, POSIX-oikeuksien (`mode`) ja millisekunnin tason muokkausaikaan (>5) reaaliaikaiseen push-toimintoon. nolla datalohkon lähetystä, kun vain lupia tai aikaa muutetaan.
- **BT-tyylinen samanaikainen lataus ja lähetys sekä alueellinen affiniteettiajoitus**: Lataussolmun ei tarvitse odottaa, että koko määrä ladataan. Kaikki 128-bittisen hash-tarkistuksen läpäisseet lohkovälit (`Op::Have`) lukitaan välittömästi muistiin/väliaikaiseen pooliin, jotta muut solmut voivat vetää samanaikaisesti. Yhdessä dynaamisen PEX-solmunvaihdon ja hitaan solmun puolien ennaltaehkäisyn (Work-Stealing) kanssa klusterin suorituskyky maksimoidaan.
- **Haitallisten solmujen peukaloinnin ja klusterin myrkytyssuoja**: arvovaltainen lähdeluettelo `root_hash` Itsestään johdonmukainen lukitus + pakollinen laitteistokiihdytys `gxhash128` tiivistelmän tarkistus ennen kuin jokainen lohko asetetaan levylle ja siirretään ulkoisesti; Jos peukaloitu lohko löytyy, millisekunnin tason katkaisija katkaisee haitallisen solmun ja palautuu automaattisesti rehellisestä lähteestä.
- **FastCDC in situ -lisäys ja pieni tiedostojen yhdistäminen**: Vain differentiaalilohkot siirretään suurten tiedostojen osittaiseen muokkaamiseen ja sijoitetaan levylle; massiivinen pieni tiedostomuistipakkaus ja etuliitedifferentiaalinen pakkaus eliminoivat tiedostokohtaiset meno-paluukustannukset.
- **Itseparantuminen sähkökatkon ja uudelleenkäynnistyksen jälkeen**: Redis-tyyppistä RDB + AOF -hybridimuotoa (`.s0_subs.bin`) käytetään säilyttämään kylvötilan ja tiedostoluettelon, päivittäiset muutokset liitetään mikrosekunneissa ja ne kirjoitetaan automaattisesti uudelleen ja järjestetään säännöllisesti. Kun palvelin on käynnistetty uudelleen, offline-muutokset työnnetään automaattisesti kylvösolmuun.

## Tekninen arkkitehtuuri ja dokumentaatio

- **Kaksitasoinen irrotusprotokolla**:
  - **Ohjaus- ja reaaliaikainen muutospinta (`iroh` QUIC / TCP, ensimmäisen tavun `PROTO_VERSION` -version kättely) **: Vastaa Ed25519-käyntikortin varmentamisesta, lupien tarkistuksesta, Swarm-siementen etsinnästä ja `u64::MAX`-tason massamuutosten suoratoiston tuesta (reaaliaikainen push="4">) ja <b">id="4">. id="6"> / `FsOp::Remove` / `FsOp::Meta` Kolme primitiiviä).
  - **Datan jakelupinta (natiivi UDP + `compio` asynkroninen I/O) **: 20B kompakti otsikko, 18B TACK jaksollinen palaute, BBRv3/Copa-hybridiruuhkanhallinta, RACK-TLP-pakettihäviön palautus, ensimmäisen lohkon Shannon-entropian adaptiivinen Zstd-pakkauksen ohitus="2"> lohkon loppu.
- **Lisätietoja**:
  - [Suorituskyvyn mittausraportti ja toistoskripti](https://github.com/webc-site/sync0/blob/main/doc/zh/benchmark.md)
  - [Järjestelmäarkkitehtuurin suunnittelu](https://github.com/webc-site/sync0/blob/main/doc/zh/design_v4.md)
  - [Binary Wire Protocol ja Persistence Specification](https://github.com/webc-site/sync0/blob/main/doc/zh/proto.md)
  - [Henkilöllisyyden todennus, auktoriteetin valvonta ja eheyden todentaminen](https://github.com/webc-site/sync0/blob/main/doc/zh/crypto.md)