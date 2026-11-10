# sync0

`sync0` on detsentraliseeritud P2P-failide jagamise, reaalajas sünkroonimise ja mitme allikaga levitamise tööriist, mida kasutatakse `rsync` asendamiseks – mandriülese laivõrgu mõõdetud kiirus on kuni **6,95×** `rsync`-st (ühe punkti** geomeetriline keskmine kiirus **2,01 × allikas 3 ja 3 keskmine kiirus **2,01×**, P.2). suurte väikeste failide sünkroonimine üle ookeanide on sama kiire kui **7,19 ×**.

Iga masin on nii server kui ka klient: puudub vajadus keskserveri või avaliku võrguaadressi järele, automaatne augustamine põhineb hajutatud võrgu avaliku võtme aadressil ja toetab otseedastust kahe masina vahel, mitme jagamisrühma loa hostimist, mitmekordset alamkataloogide valikut nõudmisel, millisekunditasemel reaalajas järkjärgulist sünkroonimist ja P2P-i mitme allika samaaegset kasutamist.

## Installige

### 1. meetod: lasti paigaldamine

```bash
cargo install sync0
```

### 2. meetod: laadige alla eelkompileeritud kahendfailid

Süsteemile vastava `.tar.xz` tihendatud paketi allalaadimiseks (toetab Linuxi, macOS-i, Windowsi, Androidi, iOS-i, FreeBSD-d), avage [GitHubi väljaanded](https://github.com/webc-site/sync0/releases), pakkige lahti ja sisestage `PATH`:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Alustage kiiresti

### 1. Nullkonfiguratsiooniga otseedastus kahe masina vahel (`send` / `recv`)

Pole vaja konfigureerida jagamisrühma, faile või katalooge otse kahe masina vahel suurel kiirusel üle kanda:

```bash
# Vastuvõttev ots kuulab porti ja kirjutab aadressile ./backup
sync0 recv 0.0.0.0:9000 ./backup

# Otseedastus saatjalt (valikuline -- kustutage peegel liigsete failide puhastamiseks, -- kiirus 50 MB kiiruspiirang)
sync0 send ./project 192.168.1.10:9000
```

### 2. Postituste jagamine ja nõudmisel tõmbamine/üleslaadimine (`share` / `pull` / `push`)

```bash
# Server: avaldage jagamisrühm ühe käsureaga (väljastage pärast käivitamist IP-vaba iroh-sõlme aadress)
sync0 share dataset:./data

# Klient: tõmmake kogu andmestiku jagamise rühm kohalikku ./local_data
sync0 pull dataset:// ./local_data -p <sõlme aadress>

# Klient: tõmmake nõudmisel mitu valikut (sünkroonige ainult määratud üksikuid faile ja alamkatalooge)
sync0 pull dataset://docs/readme.md,assets/icons,src/core ./local_data -p <sõlme aadress>

# Klient: laadige failid või kataloogid üles volitatud jagamisrühmadesse
sync0 push dataset://releases/v1 ./build_output -p <sõlme aadress>
```

### 3. Reaalajas tellimuste külvamine ja mitme masinaga kahesuunaline sünkroonimine (`sub` / `group`)

Kui pärast pika ühenduse loomist toimub kaugotsas mõni fail **lisatakse, muudetakse, kustutatakse, muudetakse lubasid või muutmisaeg**, värskendatakse kohalikku kataloogi sünkroonselt millisekundites reaalajas (uued/muudetud suured failid tõmbavad ainult erinevuse plokid ja värskendavad neid paigas, failide kustutamine puhastab automaatselt kohalikud failid ja tühjad kataloogid ning muuda ainult lube):

- **`sync0 sub`(Ühesuunaline reaalajas peegeldamine + automaatne külvamine)**: kohalik reaalajas järgib jagamisotsa lisamisi, kustutamisi ja muudatusi ning toimib automaatselt P2P algallikana, et pakkuda samaaegset kiirendust teistele tõmbamissõlmedele.
- **`sync0 group`(Mitme masinaga peer-to-peer kahesuunaline reaalajas sünkroonimine)**: kui faile lisatakse, kustutatakse või muudetakse mõnes grupi masinas, värskendatakse kõiki teisi võrgusõlmi reaalajas sünkroonselt ja need toimivad üksteise seemnetena.

```bash
# Liituge kogu jagamisrühmaga: kauglisamised, kustutamised ja muudatused sünkroonitakse reaalajas kohapeal ja külvatakse automaatselt
sync0 sub dataset:// ./local_mirror -p <sõlme aadress>

# Nõudmisel mitme valikuga tellimus: sünkroonige ja sisestage reaalajas ainult määratud üksikud failid ja alamkataloogid
sync0 alamandmekogum://models/llama.onnx,configs ./local_mirror -p <sõlme aadress>

# Mitme masinaga partnerrühma kahesuunaline reaalajas sünkroonimine (toetab ka täis- või komavalikuteid)
sync0 rühm team-docs:// ./docs -p <sõlme aadress>
```

### 4. Meeskonna mitme jagamise rühma ja liikme lubade konfiguratsioon (`.nt` konfiguratsioon)

Vaikimisi seadistuste kataloog on `~/.config/sync0` (saab vahetada `-c <kataloogi` kaudu), kasutades minimalistlikku [NestedText](https://nestedtext.org) (`.nt`) vormingut:
- **Toetab avaliku võrgu lugemise piiramist (lugeda saavad ainult volitatud kasutajad)**: seni, kuni jagamisrühm on konfigureeritud `users:` liikmete loendiga (või käsureal on määratud `--allow-r` / `--allow-rw` / `--allow-w` / `--auth`), lülitab süsteem automaatselt sisse ** ** ymous =" ja ei keela avalik võrk 6 (">) loendamine või lugemine ning külvisõlmede P2P-tellimine pärib samaaegselt ka valge nimekirja, võimaldades lugeda ainult volitatud kasutajatel; jagamisrühmad, mis pole seadistatud funktsiooniga `users`, on vaikimisi avalikud ja kirjutuskaitstud (kui soovite olla avalikud ja kirjutuskaitstud ning avatud üleslaadimiseks ainult määratud liikmetele, saate määrata `auth: false`).
- **Peened lugemis-kirjutus- ja alamkataloogide edastamisõigused**: liikmed on vaikimisi kirjutuskaitstud (`r` ei pea olema selgesõnaliselt kirjutatud) ja toetavad `r` (kirjutuskaitstud), `w` (kirjutamine ainult tilkkasti, pole loetav), `rw` (lubatud alamparandus =="4">) ja <b>prefix-w idrite üleslaadimiseks).

**Liikme konfiguratsiooni näide (`~/.config/sync0/users/`):**

```yaml
# ~/.config/sync0/users/alice.nt (vaikimisi kirjutuskaitstud, kiiruspiirang 80 MB)
id: 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c
rate: 80MB
```

```yaml
# ~/.config/sync0/users/bob.nt (lugemis- ja kirjutamisõigused, lubatud ainult määratud alamkataloogi üles laadida)
id: 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f
perm: rw
write_prefix:
  - uploads/bob
  - shared/common
```

```yaml
# ~/.config/sync0/users/charlie.nt (ainult kirjutamisluba, lugemine on keelatud)
id: 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
perm: w
```

**Jagamisgrupi konfiguratsiooni näide (`~/.config/sync0/config.nt`):**

```yaml
users_dir: ./users

groups:
  # Konfigureerimata kasutajad: vaikimisi kirjutuskaitstud avalik võrk
  dataset:
    path: ./data
    rate: 50MB

  # Konfigureeritud kasutajad: piirake automaatselt avaliku võrgu lugemist (ainult Alice ja Bob saavad lugeda, Charlie saab ainult tarnida ja üles laadida)
  team-vault:
    path: ./vault
    users:
      - alice
      - bob
      - charlie
```

Käivitage teenus (laadige konfiguratsioonifail või kasutage otse käsurea parameetreid avaliku võrgu lugemise piiramiseks ja liikmete volitamiseks):

```bash
# Alustamiseks laadige ~/.config/sync0/config.nt
sync0 share --region SHA

# Või avaldage privaatne jagamisrühm puhta käsurea kaudu (määrake --allow-*, et automaatselt piirata avaliku võrgu lugemist ja ainult volitatud kasutajad pääsevad sellele juurde)
sync0 share team-vault:./vault \
  --allow-r 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c \
  --allow-rw 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f:uploads/bob,shared/common \
  --allow-w 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
```

### 5. Sõlme aadress, visiitkaart ja mitmekeelne

```bash
# Vaadake kohalikku IP-vaba sõlme aadressi (võti genereeritakse esmakordsel käivitamisel automaatselt)
sync0 id

# Looge Ed25519 iseallkirjastatud sõlme visiitkaart (hüüdnimi, linnakood, e-post, veebisait, tutvustus)
sync0 profile -u alice --region SHA --email alice@example.com -b "Shanghai node"

# Seal on 75 sisseehitatud keelt, mis järgivad vaikimisi süsteemi keelt ja mida saab keskkonnamuutujate kaudu vahetada.
SYNC0_LANG=en sync0 --help
```

## Jõudlus ja põhifunktsioonid

### Mandritevaheline WAN-i tegelik mõõtmine (võrdlus `rsync`)

Tegelik mõõtmiskeskkond: Ameerika Ühendriigid `c1` → Euroopa `n1` (RTT 65 ms), andmekogumiks on Linuxi kernel `linux-7.3.3`:

#### 1) A la carte režiim vs `rsync`

| Andmekogum | suurus | `sync0` Üks punkt<br>(Mbps) | `rsync`<br>(Mbps) | Kiirendus (`sync0` / `rsync`) |
| :--- | ---: | ---: | ---: | ---: |
| `linux-7.3.3.tar.xz`(pole tihendatav) | 155 MB | **121.07** | 107.89 | **1,12×** |
| `linux-7.3.3.tar`(kokkusurutav) | 1.6 GB | **442.07** | 506.62 | **0,87×** |
| `linux-7.3.3.tar`(kohaliku muudatuse juurdekasv) | 1.6 GB | **8,123.87** | 1,169.49 | **6,95×** |
| `linux-7.3.3/` (95 979 väikest faili) | 1.5 GB | **565.49** | 235.27 | **2,40×** |
| **geomeetriline keskmine** | — | **704.17** | 350.19 | **2.01×** |

#### 2) P2P kahe allika režiim vs `rsync`

| Andmekogum | suurus | `sync0` P2P kahe allikaga<br>(Mbps) | `rsync`<br>(Mbps) | Kiirendus (`sync0` / `rsync`) |
| :--- | ---: | ---: | ---: | ---: |
| `linux-7.3.3.tar.xz`(pole tihendatav) | 155 MB | **466.85** | 107.89 | **4,33×** |
| `linux-7.3.3.tar`(kokkusurutav) | 1.6 GB | **992.17** | 506.62 | **1,96×** |
| `linux-7.3.3.tar`(kohaliku muudatuse juurdekasv) | 1.6 GB | **5,666.84** | 1,169.49 | **4,85×** |
| `linux-7.3.3/` (95 979 väikest faili) | 1.5 GB | **703.30** | 235.27 | **2,99×** |
| **geomeetriline keskmine** | — | **1,165.63** | 350.19 | **3,33×** |

> Märkus. Kahe allika P2P-režiimis teevad Euroopa sõlmed `s1` külvimisel koostööd ja mandriteülene magistraalvõrgu RTT on 65 ms.

### Toote põhiomadused

- **Avaliku võtme adresseerimise automaatne augustamine**: pole vaja keskserverit ja avalikku võrguaadressi, automaatne NAT-i augustamine, mis põhineb hajutatud võrgu avaliku võtme aadressil.
- **Millisekunditasemel reaalajas sünkroonimine ja metaandmete teadlikkus**: põhineb natiivsel failisündmuste jälgimisel (`FSEvents` / `inotify` / `ReadDirectoryChangesW`), failide lisamise, kustutamise, POSIX-i lubade (`mode`) ja millisekundi tasemel muutmise ajal ("><b">5); null andmeploki edastamine, kui muudetakse ainult õigusi või aega.
- **BT-stiilis samaaegne alla- ja üleslaadimine ning piirkondliku afiinsuse ajastamine**: allalaadimissõlm ei pea ootama, kuni kogu summa alla laaditakse. Kõik plokkide intervallid (`Op::Have`), mis on läbinud 128-bitise räsikontrolli, lukustatakse kohe mällu/ajutisse kogumisse, et teised sõlmed saaksid samaaegselt tõmmata. Koos dünaamilise PEX-sõlmede vahetuse ja aeglase sõlme poole eelostuga (Work-Stealing) maksimeeritakse klastri läbilaskevõime.
- **Pahatahtlike sõlmede võltsimise ja klastri mürgistuse kaitse**: autoriteetne allikaloend `root_hash` Iseseisev lukustamine + kohustuslik riistvarakiirendus `gxhash128` kokkuvõtte kontrollimine enne iga ploki kettale asetamist ja välist külvi; kui leitakse rikutud plokk, lülitab millisekundi taseme kaitselüliti pahatahtliku sõlme välja ja taastub automaatselt ausast allikast.
- **FastCDC in situ juurdekasv ja väikeste failide koondamine**: suurte failide osaliseks muutmiseks edastatakse ainult diferentsiaalplokid ja paigutatakse need kettale; massiivne väike failimälu pakkimine ja eesliidete diferentsiaalne pakkimine välistavad failide kaupa edasi-tagasi liikumise.
- **Iseparanemine pärast voolukatkestust ja taastumist pärast taaskäivitamist**: külvioleku ja faililoendi säilitamiseks kasutatakse Redis-laadset RDB + AOF hübriidvormingut (`.s0_subs.bin`), igapäevased muudatused lisatakse mikrosekundites ning need kirjutatakse automaatselt ümber ja korraldatakse regulaarselt. Pärast serveri taaskäivitamist lükatakse võrguühenduseta muudatused automaatselt külvisõlme.

## Tehniline arhitektuur ja dokumentatsioon

- **Kahetasandilise lahtisidumise protokoll**:
  - **Juhtimis- ja reaalajas muutmispind (`iroh` QUIC / TCP, esimese baidi `PROTO_VERSION` versiooni käepigistus) **: vastutab Ed25519 visiitkaardi kinnitamise, loa kontrollimise, sülemi seemnete avastamise ja `u64::MAX` taseme massimuutuste voogesituse (reaalajas tõukepakkimise pakkimise ja <b">b="4"> <b">b="4"> toe eest). id="6"> / `FsOp::Remove` / `FsOp::Meta` Kolm primitiivi).
  - **Andmete jaotuspind (native UDP + `compio` asünkroonne I/O) **: 20B kompaktne päis, 18B TACK perioodiline tagasiside, BBRv3/Copa hübriidülekoormuskontroll, RACK-TLP pakettkadude taastamine, esimese ploki Shannoni entroopia adaptiivne Zstd tihendamise möödaviik="2">verifitseerimine end-to-to.
- **Lisateave**:
  - [Jõudlusmõõtmise aruanne ja reprodutseerimisskript](https://github.com/webc-site/sync0/blob/main/doc/zh/benchmark.md)
  - [Süsteemi arhitektuuri projekteerimine](https://github.com/webc-site/sync0/blob/main/doc/zh/design_v4.md)
  - [Binaarjuhtme protokoll ja püsivuse spetsifikatsioon](https://github.com/webc-site/sync0/blob/main/doc/zh/proto.md)
  - [Identiteedi autentimine, volituste kontroll ja terviklikkuse kontrollimine](https://github.com/webc-site/sync0/blob/main/doc/zh/crypto.md)