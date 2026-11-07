# sync0

`sync0` on komentorivityökalu tiedostojen siirtämiseen verkkojen välillä, hakemistojen asteittaiseen synkronointiin ja tietojen samanaikaiseen jakamiseen useille koneille.

## Alkuperäinen suunnittelun tarkoitus

Koneiden välisissä huoneissa, maissa tai ympäristöissä, joissa on verkkoviiveitä ja pakettihäviöitä, työkalut, kuten `rsync` ja `scp`, perustuvat yksittäisen yhteyssekvenssin vahvistukseen, mikä rajoittaa kaistanleveyden käyttöä. Kun useat koneet vetävät tietoja samasta palvelimesta samanaikaisesti, lähdepalvelimen verkkokortista voi helposti tulla pullonkaula.

`sync0`:n tavoitteena on ratkaista kaksi ongelmaa:

1. Anna tiedostojen ja hakemistojen synkronoinnin kahden koneen välillä suorittaa koko fyysisen kaistanleveyden ja siirtää muutettu osa vain, kun tiedostoa on osittain muokattu.
2. Useat koneet voivat siirtää tietolohkoja toisilleen samaa hakemistoa synkronoidessaan, mikä vähentää lähdepalvelimen kuormitusta ja lyhentää synkronoinnin kokonaisaikaa.

## Nopeuden vertailu rsyncin kanssa

Testiympäristö: eurooppalainen solmu $\leftrightarrow$ Pohjois-Amerikan solmu, edestakaisen matkan viive 86 ms, mittaa koko aikaa vievää, mukaan lukien hakemistoskannaus, kopiointivertailu, verkkolähetys ja levyn sijoittelun tarkistus:

| Lähetyksen skenaario | Tietojen koko | `sync0` | `rsync` | useita |
| :--- | :---: | :---: | :---: | :---: |
| 10 000 pientä tiedostoa | 77.6 MB | 0.90 s (688 Mbps) | 4.86 s (128 Mbps) | 5.4x |
| Koodi- ja arkistopaketit (`.tar`) | 512 MB | 1.94 s (2111 Mbps) | 9.36 s (438 Mbps) | 4.8x |
| Videotiedosto (`.mp4`) | 512 MB | 7.36 s (556 Mbps) | 12.11 s (338 Mbps) | 1.6x |
| Inkrementaalinen synkronointi (muokkaa 1 % sisältöä) | 512 MB | 0.12 s | 1.85 s | 15.4x |

## Suorituskyvyn parantamismenetelmät

- **UDP-samaaikainen lähetys**: Lähetä tietolohkot UDP:n kautta ja lähetä uudelleen vain kadonneet fragmentit, jotta yksittäiset paketit eivät estä myöhempiä tiedostojen siirtoja.
- **Sisällön segmentointi ja duplikoinnin poistaminen**: Jaa tiedosto tietolohkoihin sisällön mukaan ja laske tarkistusarvo. Vastaanottavassa päässä oleva sama sisältö käytetään uudelleen suoraan paikallisesti ilman verkon monistamista.
- **Säädä pakkausta automaattisesti verkon nopeuden mukaan**: Ota pakkaus käyttöön pakattavalle sisällölle, kuten tekstille ja koodille. ohittaa automaattisesti videoiden, kuvien, pakattujen pakettien tai nopean lähiverkon pakkaamisen vähentääksesi suorittimen kulutusta.
- **Usean solmun lähistöllä veto**: Kun useat koneet vetävät samaa hakemistoa, tietolohkot vedetään samanaikaisesti ladatuista solmuista samassa kaupungissa tai pienemmällä viiveellä.

## Asentaa

### Tapa 1: Asenna Cargon kautta

```bash
cargo install sync0
```

### Tapa 2: Lataa esikäännetyt binaarit

Siirry kohtaan [GitHub Releases](https://github.com/webc-site/sync0/releases) ladataksesi `.tar.xz`-pakatun paketin vastaavalle järjestelmälle (tukee Linux x86_64/ARM64, macOS Apple Silicon/Intel, Windows x64/ARM64, Android ARM64/x86_64, FreeBSD, NetBSD). Pakkauksen purkamisen jälkeen aseta `sync0` hakemistoon `PATH`:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Käyttöopas

### 1. Siirrä suoraan tiedostoja tai hakemistoja kahden koneen välillä

Aloita valvonta vastaanottavalla koneella ja määritä vastaanotto- ja tallennuspolku:

```bash
sync0 recv 0.0.0.0:9000 ./backup
```

Lähetä yksittäinen tiedosto tai koko hakemisto lähettävästä koneesta:

```bash
sync0 send ./project 192.168.1.10:9000
```

Yleisesti käytetyt parametrit:

```bash
# Poista ylimääräiset tiedostot, jotka ovat vastaanottopäässä, mutta eivät lähetyspäässä (peilisynkronointi)
sync0 send ./project 192.168.1.10:9000 --delete

# Rajoita lähetysnopeutta (yksikkö: tavua/sekunti, esimerkiksi 50 Mt/s)
sync0 send ./project 192.168.1.10:9000 --rate 50000000

# Hiljainen tila (etenemispalkkia ei näy)
sync0 send ./project 192.168.1.10:9000 -q
```

### 2. Jaettujen hakemistojen julkaiseminen ja hakeminen useista solmuista

Julkaise jaettu hakemisto (nimeltään `dataset`) alkuperäpalvelimella:

```bash
sync0 share dataset ./data --region SHA
```

Vedä jaettu hakemisto muissa koneissa (voit täyttää useita solmuosoitteita samanaikaisesti pilkuilla erotettuina, jotta voit vetää samanaikaisesti):

```bash
sync0 pull 10.0.0.1:9000,10.0.0.2:9000 dataset ./local_data
```

Vedä vain tietty alihakemisto jaetussa hakemistossa:

```bash
sync0 pull 10.0.0.1:9000 dataset ./local_data --subpath 2026/logs
```

Työnnä paikalliset muutokset takaisin jaettuun etähakemistoon:

```bash
sync0 push 10.0.0.1:9000 dataset ./local_data
```

### 3. Tilaa luettelon automaattiset päivitykset

Tilaa lähdesolmun jaettu hakemisto. Kun tiedosto lisätään tai sitä muutetaan lähdehakemistoon, se synkronoidaan automaattisesti paikalliseen tietokoneeseen. Samanaikaisesti paikallinen kone toimii automaattisesti solmuna muille koneille, jotka voivat vetää:

```bash
sync0 sub 10.0.0.1:9000 dataset ./local_mirror
```

### 4. Monikoneryhmien kaksisuuntainen synkronointi

Useita koneita liittyy samaan ryhmän nimeen, ja minkä tahansa solmun hakemiston muutokset synkronoidaan automaattisesti ryhmän muihin solmuihin:

```bash
sync0 group team-docs ./docs 10.0.0.1:9000,10.0.0.2:9000
```

### 5. Solmun henkilöllisyys ja henkilökohtainen käyntikortti

Jokainen solmu tukee käyttäjänimen, kaupungin, sähköpostiosoitteen, verkkosivuston ja henkilökohtaisen allekirjoituksen (enintään 256 merkkiä) asettamista ja välittää paikallisen yksityisen avaimen allekirjoituksen todennuksen:

```bash
# Tarkastele paikallisen julkisen avaimen tunnusta (määritä määrityshakemisto, jotta avain säilyy)
sync0 id -c ~/.config/sync0

# Luo ja näytä solmujen käyntikortteja henkilökohtaisilla allekirjoituksilla
sync0 profile -c ~/.config/sync0 \
  -u alice \
  --region SHA \
  --email alice@example.com \
  --url https://example.com \
  -b "Shanghai backup node"
```

`share`, `pull`, `sub`, `group` voidaan käynnistää myös näillä profiiliparametreilla:

```bash
sync0 share dataset ./data -c ~/.config/sync0 -u alice --region SHA -b "Shanghai backup node"
```

### 6. Monikielinen vaihto

`sync0` Tukee 75 kieltä. Oletusarvon mukaan komentorivin ohje ja kehotteet vastaavalla kielellä näytetään automaattisesti käyttöjärjestelmän kieliympäristön mukaan. jos järjestelmän kieli ei ole tuetussa luettelossa, oletusarvoisesti käytetään englantia.

Määritä kieli manuaalisesti asettamalla ympäristömuuttuja `SYNC0_LANG`:

```bash
SYNC0_LANG=en sync0 --help
SYNC0_LANG=ja sync0 --help
SYNC0_LANG=de sync0 --help
```