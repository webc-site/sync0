# sync0

`sync0` on P2P (peer-to-peer) -tiedostonjako- ja usean koneen synkronointityökalu, joka kilpailee Resilio Syncin kanssa.

Jokainen kone on sekä palvelin että asiakas: mikä tahansa solmu voi julkaista useita tiedostonjakoryhmiä. Kun muut solmut ovat liittyneet, ne voivat automaattisesti hankkia käytettävissä olevat siemensolmut vertaispalvelimelta ja hakea ne samanaikaisesti useista lähteistä. Kun synkronointi on valmis, uusille solmuille luodaan automaattisesti siemeniä latausten nopeuttamiseksi.

## Ydinkohokohdat

- **Kaikki ovat sekä palvelimia että asiakkaita**: Verrattavissa Resilio Synciin, ei keskitettyä palvelinriippuvuutta. Mikä tahansa solmu voi luoda useita tiedostonjakoryhmiä ja voi myös liittyä muiden ihmisten jakamisryhmiin. solmut vaihtavat automaattisesti ja hankkivat käytettävissä olevat siemensolmut saavuttaakseen useiden koneiden samanaikaisen vedon ja reaaliaikaisen synkronoinnin.
- **IP-vapaa solmuosoite**: Oletuksena solmuavain ja `base64url` koodattu `iroh` osoite luodaan automaattisesti `~/.config/sync0`:een ilman, että julkista IP-osoitetta tarvitsee muistaa tai paljastaa.
- **Monivalintainen synkronointi pyynnöstä**: Tukee koko jakamisryhmän synkronointia ja tukee myös synkronointia tarpeen mukaan valitsemalla useita tiedostoja tai alihakemistoja samanaikaisesti.
- **Ryhmän jäsenten hierarkkiset käyttöoikeudet ja lataushakemiston hallinta**: Tukee vain luku- (`ro`), luku- ja kirjoitus-lataus- (`rw`) ja hallinta- (`admin`) oikeuksia ja voi määrittää yksilöllisesti polun etuliiteen, jonka jokainen jäsen saa ladata (oletusarvoisesti lataaminen jaon juurihakemistoon on sallittu).
- **Täysinopeuksinen ja asteittainen synkronointi verkkojen välillä**: suurten tiedostojen samanaikainen siirto, suurien pienten tiedostojen yhdistäminen muistiin ja vain synkronoidut tietolohkot, kun tiedostoja muutetaan osittain.

## Nopeuden vertailu rsyncin kanssa

### Valtameren ylittävä verkko yhden lähteen todellinen mittaus (Kiina → Yhdysvallat, viive 260 ms)

| Lähetyksen skenaario | Tietojen koko | malli | `sync0` | `rsync -az` | Kasvata kerrointa |
| :--- | :---: | :---: | :---: | :---: | :---: |
| 500 pientä tiedostoa | 1.91 MB | Täysi synkronointi | 18.84 Mbps | 5.32 Mbps | 3.5x |
| 500 pientä tiedostoa | 1.91 MB | Inkrementaalinen synkronointi | 27.15 Mbps | 3.83 Mbps | 7.1x |
| Koodi- ja arkistopaketit (`.tar`) | 10.0 MB | Täysi synkronointi | 139.70 Mbps | 24.97 Mbps | 5.6x |
| Koodi- ja arkistopaketit (`.tar`) | 10.0 MB | Inkrementaalinen synkronointi | 83.38 Mbps | 26.02 Mbps | 3.2x |
| Videotiedosto (`.mp4`) | 15.0 MB | Täysi synkronointi | 54.06 Mbps | 27.04 Mbps | 2.0x |
| Videotiedosto (`.mp4`) | 15.0 MB | Inkrementaalinen synkronointi | 216.83 Mbps | 21.51 Mbps | 10.1x |

### Valtameren ylittävä verkko P2P monisolmuinen vetotesti (Kiina → Yhdysvallat kaksoissolmut, viive 260 ms)

Mitattu vertailu usean lähteen siemensolmun raidallisesta samanaikaisesta vedosta:

| Lähetyksen skenaario | Tietojen koko | Yhden solmun kylvö | Kahden solmun P2P samanaikainen kylvö | Kasvata kerrointa |
| :--- | :---: | :---: | :---: | :---: |
| 500 pientä tiedostoa | 1.97 MB | 5.28 Mbps | 7.87 Mbps | 1.5x |
| Koodi- ja arkistopaketit (`.tar`) | 30.0 MB | 56.35 Mbps | 83.17 Mbps | 1.5x |
| Videotiedosto (`.mp4`) | 30.0 MB | 27.00 Mbps | 39.46 Mbps | 1.5x |

### Mannerten välisen nopean erillisen linjan todellinen mittaus (Eurooppa → Pohjois-Amerikka, viive 86 ms)

| Lähetyksen skenaario | Tietojen koko | `sync0` | `rsync` | Kasvata kerrointa |
| :--- | :---: | :---: | :---: | :---: |
| 10 000 pientä tiedostoa | 77.6 MB | 688 Mbps | 128 Mbps | 5.4x |
| Koodi- ja arkistopaketit (`.tar`) | 512 MB | 2111 Mbps | 438 Mbps | 4.8x |
| Videotiedosto (`.mp4`) | 512 MB | 556 Mbps | 338 Mbps | 1.6x |
| Inkrementaalinen synkronointi (muokkaa 1 % sisältöä) | 512 MB | 34133 Mbps | 2214 Mbps | 15.4x |

## Asentaa

### Tapa 1: Asenna Cargon kautta

```bash
cargo install sync0
```

### Tapa 2: Lataa esikäännetyt binaarit

Siirry kohtaan [GitHub Releases](https://github.com/webc-site/sync0/releases) ladataksesi `.tar.xz`-pakatun paketin vastaavalle järjestelmälle (tukee Linuxia, macOS:ää, Windowsia, Androidia, FreeBSD:tä, NetBSD:tä), pura ja laita se `PATH`-hakemistoon:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Aloita nopeasti

`sync0` käyttää `~/.config/sync0` oletusarvoisesti määrityshakemistona (jos sinun on vaihdettava muihin hakemistoihin, voit lisätä `-c <hakemistoon>`).

### 1. Tarkastele paikallisen solmun osoitetta ja asetuskäyntikorttia

```bash
# Tarkastele paikallisen solmun osoitetta (avain luodaan automaattisesti tiedostossa ~/.config/sync0 ensimmäistä kertaa)
sync0 id

# Luo henkilökohtainen solmun käyntikortti (tukee käyttäjänimeä, kaupunkia, sähköpostiosoitetta, verkkosivustoa ja allekirjoitusta 256 sanan sisällä)
sync0 profile -u alice --region SHA --email alice@example.com --url https://example.com -b "Shanghai node"
```

### 2. Julkaise useita tiedostonjakoryhmiä ja aseta ryhmän jäsenten käyttöoikeudet (`.nt` Määritykset)

`sync0` on määritetty muodossa [Sisäkkäinen teksti](https://nestedtext.org) (`.nt`). Jokaisella käyttäjällä on erillinen `.nt` määritystiedosto. Voit määrittää käyttäjäprofiilin hakemiston `users_dir` -kohdan `config.nt` kautta (joka sisältää automaattisesti kaikki tämän hakemiston `.nt` tiedostot). Sekä käyttäjä että ladattava polun etuliite käyttävät järjestämätöntä luetteloa (`-`). **Jos latauspolun etuliitettä ei ole täytetty, lataaminen jaon juurihakemistoon on oletuksena sallittu**.

1. Luo erilliset `.nt` määritystiedostot kullekin jäsenelle `~/.config/sync0/users/` hakemistossa:

`~/.config/sync0/users/alice.nt` (vain luku -jäsen, rajoitettu määrä `80MB`, `rate` tukee `GB`, `MB`, `KB` yksiköitä):

```yaml
id: 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c
perm: ro
rate: 80MB
```

`~/.config/sync0/users/bob.nt` (rajoitetut lähetysjäsenet: sallittu vain ladata `uploads/bob` ja `shared/common` hakemistoihin):

```yaml
id: 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f
perm: rw
write_prefix:
  - uploads/bob
  - shared/common
```

`~/.config/sync0/users/charlie.nt` (Oletusjäsenen lataus: Kun `write_prefix` jätetään pois, lataaminen jaon juurihakemistoon on oletuksena sallittu):

```yaml
id: 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
perm: rw
```

2. Määritä käyttäjähakemisto ja useita tiedostonjakoryhmiä kohteessa `~/.config/sync0/config.nt`:

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

Aloita jakamispalvelu:

```bash
# Suora käynnistys (lataa automaattisesti kaikki .nt-käyttäjämääritykset ~/.config/sync0/config.nt- ja users_dir-hakemistoissa)
sync0 share --region SHA

# Tai julkaise nopeasti useita jakamisryhmiä ja valtuuta ryhmän jäseniä suoraan komentorivin kautta
sync0 share dataset:./data team-vault:./vault \
  --auth \
  --allow-ro 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c \
  --allow-rw 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f:uploads/bob,shared/common \
  --allow-rw 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
```

### 3. Vedä kaikki sisältö tai valitse useita tiedostoja ja hakemistoja (`sync0 pull`)

Muodosta yhteys mihin tahansa vertaissolmuun saadaksesi automaattisesti kaikki käytettävissä olevat siemensolmut jakamisryhmässä ja noutaa useista siemensolmuksista samanaikaisesti:

```bash
# Vedä koko tietojoukon jakamisryhmä paikalliseen ./local_data-tiedostoon
sync0 pull dataset:// ./local_data -p <solmun osoite>

# Monen valinnan synkronointi: vedä samanaikaisesti yksittäinen tiedosto docs/readme.md ja alihakemistot Asset/icons, src/core
sync0 pull dataset://docs/readme.md,assets/icons,src/core ./local_data -p <solmun osoite>
```

### 4. Lataa tiedosto tai hakemisto jakoryhmään (`sync0 push`)

Jäsenet, joilla on latausoikeudet (`rw` tai `admin`), voivat ladata paikallisia tiedostoja tai hakemistoja etäjakoryhmiin:

```bash
# Jäsenet, joilla on juurihakemiston latausoikeus: Lataa suoraan jaettuun juurihakemistoon
sync0 push team-vault:// ./new_release -p <solmun osoite>

# Vain jäsenet, joilla on oikeus tiettyyn etuliitteeseen (kuten lataukset/bob): lataa sallittuun polun etuliiteeseen
sync0 push team-vault://uploads/bob/report.pdf ./report.pdf -p <solmun osoite>
```

### 5. Tilaa automaattinen synkronointi ja kylvö (`sync0 sub`)

Tietyn solmun jakamisryhmään liittymisen jälkeen paikallinen palvelin on sekä vastaanottava asiakas että uusi siemenpalvelin; kun lähdesolmutiedosto päivitetään, se synkronoidaan automaattisesti ja tarjoaa lohkotetut lataukset muille solmuille:

```bash
# Tilaa koko jakamisryhmä ja siirry automaattisesti
sync0 alitietojoukko:// ./local_mirror -p <solmun osoite>

# Valitse vain useita, jos haluat tilata tietyt tiedostot ja alihakemistot ja siementä ne
sync0 alitietojoukko://models/llama.onnx,configs ./local_mirror -p <solmun osoite>
```

### 6. Usean koneen ryhmän vertaissynkronointi (`sync0 group`)

Useat koneet liittyvät samaan jakamisryhmään, toistensa löytävät automaattisesti käytettävissä olevat siemensolmut ja ylläpitävät kaksisuuntaista synkronointia (tukee täydellistä synkronointia tai tiedostojen ja hakemistojen useampaa valintaa):

```bash
# Liity kokonaan ryhmään ja kaksisuuntainen synkronointi
sync0 ryhmä team-docs:// ./docs -p <solmun osoite>

# Vain wal-hakemisto ja meta.json-tiedosto monivalintasynkronointiryhmässä
sync0 ryhmä team-docs://wal,meta.json ./docs -p <solmun osoite>
```

### 7. Suora lähetys pisteestä pisteeseen kahden koneen välillä (`send` / `recv`)

Ryhmää ei tarvitse määrittää valmiiksi, ja voit tilapäisesti siirtää tiedostoja tai hakemistoja suoraan kahden koneen välillä:

```bash
# Vastaanottimen valvonta
sync0 recv 0.0.0.0:9000 ./backup

# Lähetyspää siirtää suoraan tiedostoja tai hakemistoja (valinnainen --delete peili ylimääräisten tiedostojen puhdistamiseen, --nopeusrajoitus)
sync0 send ./project 192.168.1.10:9000
```

### 8. Monikielinen vaihto

`sync0` tukee 75 kieltä ja vaihtaa automaattisesti järjestelmän kieliympäristön mukaan oletusarvoisesti; se voidaan määrittää myös manuaalisesti `SYNC0_LANG`:n kautta:

```bash
SYNC0_LANG=en sync0 --help
SYNC0_LANG=ja sync0 --help
SYNC0_LANG=de sync0 --help
```