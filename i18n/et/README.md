# sync0

`sync0` on võrdõigusvõrgu (P2P) failide jagamise ja mitme masinaga sünkroonimise tööriist, mis konkureerib Resilio Synciga.

Iga masin on nii server kui ka klient: iga sõlm võib avaldada mitu failijagamisgruppi. Pärast teiste sõlmede liitumist saavad nad automaatselt hankida saadaolevad algsõlmed partnerserverist ja tõmmata need samaaegselt mitmest allikast. Pärast sünkroonimise lõpetamist luuakse allalaadimise kiirendamiseks automaatselt seemned rohkemate sõlmede jaoks.

## Peamised esiletõstmised

- **Kõik on nii serverid kui ka kliendid**: võrreldav Resilio Synciga, puudub keskne serverisõltuvus. Iga sõlm võib luua mitu failijagamisgruppi ja liituda ka teiste inimeste jagamisrühmadega; sõlmed vahetavad automaatselt ja hangivad saadaolevaid algsõlmesid, et saavutada mitme masina samaaegne tõmbamine ja reaalajas sünkroonimine.
- **IP-vaba sõlme aadress**: vaikimisi luuakse sõlme võti ja `base64url` kodeeritud `iroh` aadress `~/.config/sync0` automaatselt, ilma et oleks vaja avalikku IP-d meeles pidada või paljastada.
- **Mitme valikuga sünkroonimine nõudmisel**: toetab kogu jagamisrühma sünkroonimist ja toetab ka nõudmisel sünkroonimist, valides korraga mitu faili või alamkataloogi.
- **Grupiliikmete hierarhilised load ja üleslaadimiskataloogi juhtimine**: toetab kirjutuskaitstud (`ro`), lugemis- ja kirjutamisõigust (`rw`) ja haldusõigust (`admin`) ning saab individuaalselt määrata tee prefiksi, mida igal liikmel on lubatud üles laadida (vaikimisi on jagamise juurkataloogi üleslaadimine lubatud).
- **Täiskiirus ja järkjärguline sünkroonimine võrkude vahel**: suurte failide samaaegne edastamine, suurte väikeste failide koondamine mällu ja ainult sünkroonitud andmeplokid, kui faile on osaliselt muudetud.

## Kiiruse võrdlus rsynciga

### Ookeaniülese võrgu ühe allika tegelik mõõtmine (Hiina → Ameerika Ühendriigid, viivitus 260 ms)

| Edastamise stsenaarium | Andmemaht | mudel | `sync0` | `rsync -az` | Suurenda kordajat |
| :--- | :---: | :---: | :---: | :---: | :---: |
| 500 väikest faili | 1.91 MB | Täielik sünkroonimine | 18.84 Mbps | 5.32 Mbps | 3.5x |
| 500 väikest faili | 1.91 MB | Järkjärguline sünkroonimine | 27.15 Mbps | 3.83 Mbps | 7.1x |
| Koodi- ja arhiivipaketid (`.tar`) | 10.0 MB | Täielik sünkroonimine | 139.70 Mbps | 24.97 Mbps | 5.6x |
| Koodi- ja arhiivipaketid (`.tar`) | 10.0 MB | Järkjärguline sünkroonimine | 83.38 Mbps | 26.02 Mbps | 3.2x |
| Videofail (`.mp4`) | 15.0 MB | Täielik sünkroonimine | 54.06 Mbps | 27.04 Mbps | 2.0x |
| Videofail (`.mp4`) | 15.0 MB | Järkjärguline sünkroonimine | 216.83 Mbps | 21.51 Mbps | 10.1x |

### Ookeaniülese võrgu P2P mitme sõlme tõmbetest (Hiina → Ameerika Ühendriigid, kaks sõlme, viivitus 260 ms)

Mitme allika seemnesõlme triibulise samaaegse tõmbe mõõdetud võrdlus:

| Edastamise stsenaarium | Andmemaht | Ühe sõlme külvamine | Kahe sõlmega P2P samaaegne külvamine | Suurenda kordajat |
| :--- | :---: | :---: | :---: | :---: |
| 500 väikest faili | 1.97 MB | 5.28 Mbps | 7.87 Mbps | 1.5x |
| Koodi- ja arhiivipaketid (`.tar`) | 30.0 MB | 56.35 Mbps | 83.17 Mbps | 1.5x |
| Videofail (`.mp4`) | 30.0 MB | 27.00 Mbps | 39.46 Mbps | 1.5x |

### Mandriülese kiirliini tegelik mõõtmine (Euroopa → Põhja-Ameerika, viivitus 86 ms)

| Edastamise stsenaarium | Andmemaht | `sync0` | `rsync` | Suurenda kordajat |
| :--- | :---: | :---: | :---: | :---: |
| 10 000 väikest faili | 77.6 MB | 688 Mbps | 128 Mbps | 5.4x |
| Koodi- ja arhiivipaketid (`.tar`) | 512 MB | 2111 Mbps | 438 Mbps | 4.8x |
| Videofail (`.mp4`) | 512 MB | 556 Mbps | 338 Mbps | 1.6x |
| Järkjärguline sünkroonimine (muuda 1% sisu) | 512 MB | 34133 Mbps | 2214 Mbps | 15.4x |

## Installige

### 1. meetod: paigaldage Cargo kaudu

```bash
cargo install sync0
```

### 2. meetod: laadige alla eelkompileeritud kahendfailid

Minge saidile [GitHub Releases](https://github.com/webc-site/sync0/releases), et laadida alla `.tar.xz` tihendatud pakett vastava süsteemi jaoks (toetab Linuxit, macOS-i, Windowsi, Androidi, FreeBSD-d, NetBSD-d), pakkige lahti ja asetage see kataloogi `PATH`:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Alustage kiiresti

`sync0` kasutab vaikimisi seadistuskataloogina `~/.config/sync0` (kui teil on vaja lülituda teistele kataloogidele, saate lisada `-c <kataloog>`).

### 1. Vaadake kohaliku sõlme aadressi ja sätte visiitkaarti

```bash
# Vaadake kohaliku sõlme aadressi (võti genereeritakse automaatselt failis ~/.config/sync0 esimest korda)
sync0 id

# Looge isiklik sõlme visiitkaart (toetab kasutajanime, linna, e-posti aadressi, veebisaiti ja allkirja 256 sõna ulatuses)
sync0 profile -u alice --region SHA --email alice@example.com --url https://example.com -b "Shanghai node"
```

### 2. Mitme failijagamisrühma avaldamine ja rühmaliikmete õiguste määramine (`.nt` konfiguratsioon)

`sync0` on konfigureeritud [NestedText](https://nestedtext.org) (`.nt`) vormingus. Igal kasutajal on eraldi `.nt` konfiguratsioonifail. Saate määrata kasutajaprofiili kataloogi `users_dir` `config.nt` kaudu (mis sisaldab automaatselt kõiki selles kataloogis olevaid `.nt` faile). Nii kasutaja kui ka üleslaadimiseks lubatud tee eesliide kasutavad järjestamata loendit (`-`). **Kui üleslaadimistee eesliide pole täidetud, on üleslaadimine jagamise juurkataloogi vaikimisi lubatud**.

1. Looge kataloogis `~/.config/sync0/users/` iga liikme jaoks eraldi `.nt` konfiguratsioonifailid:

`~/.config/sync0/users/alice.nt` (kirjutuskaitstud liige, määr piiratud `80MB`, `rate` toetab `GB`, `MB`, `KB` üksusi):

```yaml
id: 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c
perm: ro
rate: 80MB
```

`~/.config/sync0/users/bob.nt` (piiratud üleslaadimisliikmed: lubatud üles laadida ainult `uploads/bob` ja `shared/common` kataloogidesse):

```yaml
id: 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f
perm: rw
write_prefix:
  - uploads/bob
  - shared/common
```

`~/.config/sync0/users/charlie.nt` (Liikme vaikeüleslaadimine: kui `write_prefix` on välja jäetud, on jagamise juurkataloogi üleslaadimine vaikimisi lubatud):

```yaml
id: 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
perm: rw
```

2. Seadistage `~/.config/sync0/config.nt` kasutajakataloog ja mitu failijagamisrühma:

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

Alusta teenuse jagamist:

```bash
# Otsene käivitamine (laadib automaatselt kõik .nt kasutaja konfiguratsioonid kataloogides ~/.config/sync0/config.nt ja users_dir)
sync0 share --region SHA

# Või avaldage kiiresti mitu jagamisrühma ja volitage rühma liikmeid otse käsurea kaudu
sync0 share dataset:./data team-vault:./vault \
  --auth \
  --allow-ro 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c \
  --allow-rw 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f:uploads/bob,shared/common \
  --allow-rw 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
```

### 3. Tõmmake kogu sisu või valige mitu faili ja kataloogi (`sync0 pull`)

Looge ühendus mis tahes partnersõlmega, et hankida automaatselt kõik saadaolevad algsõlmed jagamisrühmas ja tõmmata korraga mitmest algsõlmest:

```bash
# Tõmmake kogu andmestiku jagamise rühm kohalikku ./local_data
sync0 pull dataset:// ./local_data -p <sõlme aadress>

# Mitme valikuga sünkroonimine: tõmmake samaaegselt üks fail docs/readme.md ja alamkataloogid Varad/ikoonid, src/core
sync0 pull dataset://docs/readme.md,assets/icons,src/core ./local_data -p <sõlme aadress>
```

### 4. Faili või kataloogi üleslaadimine jagamisgruppi (`sync0 push`)

Üleslaadimislubadega liikmed (`rw` või `admin`) saavad kohalikke faile või katalooge kaugjagamisgruppidesse üles laadida:

```bash
# Juurkataloogi üleslaadimisõigusega liikmed: laadige üles otse jagatud juurkataloogi
sync0 push team-vault:// ./new_release -p <sõlme aadress>

# Ainult liikmed, kes on volitatud kasutama kindlat prefiksit (nt üleslaadimised/bob): laadige üles lubatud tee prefiksile
sync0 push team-vault://uploads/bob/report.pdf ./report.pdf -p <sõlme aadress>
```

### 5. Automaatse sünkroonimise ja külvamise tellimine (`sync0 sub`)

Pärast teatud sõlme jagamisrühmaga liitumist on kohalik server nii vastuvõttev klient kui ka uus külviserver; kui lähtesõlme faili värskendatakse, sünkroonitakse see automaatselt ja see pakub muude sõlmede jaoks tükeldatud allalaadimisi:

```bash
# Liituge kogu jagamisrühmaga ja alustage automaatselt
sync0 sub dataset:// ./local_mirror -p <sõlme aadress>

# Määratud failide ja alamkataloogide tellimiseks ja nende külvamiseks valige ainult mitu valikut
sync0 alamandmekogum://models/llama.onnx,configs ./local_mirror -p <sõlme aadress>
```

### 6. Mitme masina grupi kaaslaste sünkroonimine (`sync0 group`)

Mitu masinat liitub sama jagamisrühmaga, üksteist tuvastavad automaatselt saadaolevad algsõlmed ja säilitavad kahesuunalise sünkroonimise (toetab täielikku sünkroonimist või failide ja kataloogide mitut valikut):

```bash
# Täielikult grupiga liitumine ja kahesuunaline sünkroonimine
sync0 rühm team-docs:// ./docs -p <sõlme aadress>

# Ainult wali kataloog ja fail meta.json mitme valikuga sünkroonimisrühmas
sync0 rühm team-docs://wal,meta.json ./docs -p <sõlme aadress>
```

### 7. Punkt-punkti otseedastus kahe masina vahel (`send` / `recv`)

Gruppi pole vaja eelkonfigureerida ja saate ajutiselt faile või katalooge otse kahe masina vahel üle kanda.

```bash
# Vastuvõtja jälgimine
sync0 recv 0.0.0.0:9000 ./backup

# Saatmisots kannab otse faile või katalooge (valikuline --delete peegel liigsete failide puhastamiseks, --rate kiiruspiirang)
sync0 send ./project 192.168.1.10:9000
```

### 8. Mitme keele vahetamine

`sync0` toetab 75 keelt ja lülitub automaatselt vastavalt süsteemi keelekeskkonnale vaikimisi; seda saab määrata ka käsitsi `SYNC0_LANG` kaudu:

```bash
SYNC0_LANG=en sync0 --help
SYNC0_LANG=ja sync0 --help
SYNC0_LANG=de sync0 --help
```