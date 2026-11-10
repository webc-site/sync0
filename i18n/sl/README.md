# sync0

`sync0` je decentralizirano orodje za skupno rabo datotek P2P, sinhronizacijo v realnem času in distribucijo iz več virov, ki se uporablja za zamenjavo `rsync` - izmerjena hitrost transkontinentalnega prostranega omrežja je do **6,95×** od `rsync` (enotočkovna geometrijska povprečna hitrost **2,01×**, P2P dvojna geometrična povprečna hitrost **3,33×**), in sinhronizacija ogromnih majhnih datotek čez oceane je tako hitra kot **7,19×**.

Vsaka naprava je hkrati strežnik in odjemalec: ni potrebe po osrednjem strežniku ali javnem omrežnem naslovu, samodejno luknjanje temelji na porazdeljenem omrežnem naslovu javnega ključa in podpira neposreden prenos med dvema napravama, gostovanje dovoljenj skupine za več souporab, večkratno izbiro podimenikov na zahtevo, inkrementalno sinhronizacijo v realnem času na ravni milisekunde in sočasno pospeševanje več virov P2P.

## Namestite

### 1. način: namestitev tovora

```bash
cargo install sync0
```

### 2. način: Prenesite vnaprej prevedene binarne datoteke

Pojdite na [GitHub Releases](https://github.com/webc-site/sync0/releases), da prenesete `.tar.xz` stisnjen paket, ki ustreza sistemu (podpira Linux, macOS, Windows, Android, iOS, FreeBSD), razpakirajte in vstavite v `PATH`:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Hitro začnite

### 1. Neposredni prenos brez konfiguracije med dvema napravama (`send` / `recv`)

Ni potrebe po konfiguraciji skupine za skupno rabo, neposrednem prenosu datotek ali imenikov med dvema strojema pri visoki hitrosti:

```bash
# Prejemni konec posluša vrata in piše v ./backup
sync0 recv 0.0.0.0:9000 ./backup

# Neposredni prenos od pošiljatelja (izbirno --delete mirror za čiščenje odvečnih datotek, --rate omejitev hitrosti 50 MB)
sync0 send ./project 192.168.1.10:9000
```

### 2. Skupna raba objave in vlečenje/nalaganje na zahtevo (`share` / `pull` / `push`)

```bash
# Strežnik: objavite skupino za skupno rabo z eno vrstico ukaza (po zagonu izpišite naslov vozlišča iroh brez IP-ja)
sync0 share dataset:./data

# Odjemalec: povlecite celotno skupino za skupno rabo nabora podatkov v lokalni ./local_data
sync0 pull dataset:// ./local_data -p <naslov vozlišča>

# Odjemalec: potegnite več izbir na zahtevo (sinhronizirajte samo določene posamezne datoteke in podimenike)
sync0 pull dataset://docs/readme.md,assets/icons,src/core ./local_data -p <naslov vozlišča>

# Odjemalec: naložite datoteke ali imenike v pooblaščene skupine za skupno rabo
sync0 push dataset://releases/v1 ./build_output -p <naslov vozlišča>
```

### 3. Sejanje naročnine v realnem času in dvosmerna sinhronizacija z več stroji (`sub` / `group`)

Če po vzpostavitvi dolge povezave pride do **dodajanja, spreminjanja, brisanja, spreminjanja dovoljenj ali časa spreminjanja** katere koli datoteke na oddaljenem koncu, bo lokalni imenik sinhrono posodobljen v milisekundah v realnem času (nove/spremenjene velike datoteke bodo potegnile samo bloke razlike in jih posodobile na mestu, brisanje datotek bo samodejno očistilo lokalne datoteke in prazne imenike ter neposredno spremenilo dovoljenja ali časovno označilo bloke nič podatkov):

- **`sync0 sub`(Enosmerno zrcaljenje v realnem času + samodejno sejanje)**: Lokalni realni čas sledi dodajanjem, izbrisom in spremembam na koncu skupne rabe in samodejno služi kot začetni izvor P2P za zagotavljanje sočasnega pospeševanja za druga vlečna vozlišča.
- **`sync0 group`(Dvosmerna sinhronizacija enakovrednih v realnem času za več strojev)**: Če se datoteke dodajo, izbrišejo ali spremenijo na katerem koli računalniku v skupini, bodo vsa druga spletna vozlišča posodobljena sinhrono v realnem času in bodo druga drugemu služila kot semena.

```bash
# Naročite se na celotno skupino za skupno rabo: dodajanje, brisanje in spreminjanje na daljavo se sinhronizira lokalno v realnem času in se samodejno doda
sync0 sub dataset:// ./local_mirror -p <naslov vozlišča>

# Naročnina na več izbir na zahtevo: sinhronizirajte in zasedite samo določene posamezne datoteke in podimenike v realnem času
sync0 sub dataset://models/llama.onnx,configs ./local_mirror -p <naslov vozlišča>

# Dvosmerna sinhronizacija skupine enakovrednih več strojev v realnem času (podpira tudi polne poti ali poti za več izbir z vejico)
skupina sync0 team-docs:// ./docs -p <naslov vozlišča>
```

### 4. Konfiguracija dovoljenj skupine za več skupin in članov skupine (`.nt` konfiguracija)

Privzeti konfiguracijski imenik je `~/.config/sync0` (lahko preklopite prek `-c <directory>`), z uporabo minimalistične [NestedText](https://nestedtext.org) (`.nt`) oblike:
- **Podpira omejevanje branja javnega omrežja (samo pooblaščeni uporabniki lahko berejo)**: Dokler je skupina za skupno rabo konfigurirana s seznamom članov `users:` (ali ukazna vrstica določa `--allow-r` / `--allow-rw` / `--allow-w` / `--auth`), sistem **samodejno vklopi zasebno preverjanje pristnosti (`auth: true`)** in prepoveduje oštevilčevanje ali branje anonimnih vozlišč v javnem omrežju in P2P Naročanje na sejalna vozlišča bo istočasno podedovalo tudi seznam dovoljenih, kar bo branje omogočilo samo pooblaščenim uporabnikom; skupine za skupno rabo, ki niso konfigurirane z `users`, so privzeto javne in samo za branje (če želite biti javne in samo za branje ter odprte za nalaganje samo določenim članom, lahko izrecno nastavite `auth: false`).
- **Drobno zrnata dovoljenja za branje in pisanje ter dostavo podimenikov**: člani so privzeto samo za branje (`r` ni treba izrecno napisati) in podpirajo `r` (samo za branje), `w` (samo pisanje v spustno polje, ni berljivo), `rw` (branje-pisanje) in `write_prefix` (omejitev predpona podimenika, dovoljena za nalaganje).

**Primer konfiguracije člana (`~/.config/sync0/users/`):**

```yaml
# ~/.config/sync0/users/alice.nt (privzeto samo za branje, omejitev hitrosti 80 MB)
id: 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c
rate: 80MB
```

```yaml
# ~/.config/sync0/users/bob.nt (dovoljenja za branje in pisanje, dovoljeno samo nalaganje v določen podimenik)
id: 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f
perm: rw
write_prefix:
  - uploads/bob
  - shared/common
```

```yaml
# ~/.config/sync0/users/charlie.nt (samo dovoljenje za dostavo pisanja, branje je prepovedano)
id: 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
perm: w
```

**Primer konfiguracije skupine za skupno rabo (`~/.config/sync0/config.nt`):**

```yaml
users_dir: ./users

groups:
  # Nekonfigurirani uporabniki: privzeto javno omrežje samo za branje
  dataset:
    path: ./data
    rate: 50MB

  # Konfigurirani uporabniki: samodejno omeji branje javnega omrežja (samo alice in bob lahko bereta, charlie lahko samo dostavlja in nalaga)
  team-vault:
    path: ./vault
    users:
      - alice
      - bob
      - charlie
```

Zaženite storitev (naložite konfiguracijsko datoteko ali neposredno uporabite parametre ukazne vrstice, da omejite branje javnega omrežja in pooblastite člane):

```bash
# Za zagon naložite ~/.config/sync0/config.nt
sync0 share --region SHA

# Ali objavite zasebno skupino za skupno rabo prek čiste ukazne vrstice (podajte --allow-*, da samodejno omejite branje javnega omrežja in do nje lahko dostopajo samo pooblaščeni uporabniki)
sync0 share team-vault:./vault \
  --allow-r 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c \
  --allow-rw 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f:uploads/bob,shared/common \
  --allow-w 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
```

### 5. Naslov vozlišča, vizitka in večjezičnost

```bash
# Ogled lokalnega naslova vozlišča brez IP (ključ se samodejno ustvari za prvi zagon)
sync0 id

# Ustvari samopodpisano vizitko vozlišča Ed25519 (vzdevek, koda mesta, e-pošta, spletno mesto, uvod)
sync0 profile -u alice --region SHA --email alice@example.com -b "Shanghai node"

# Obstaja 75 vgrajenih jezikov, ki privzeto sledijo sistemskemu jeziku in jih je mogoče preklapljati s spremenljivkami okolja.
SYNC0_LANG=en sync0 --help
```

## Zmogljivost in osnovne funkcije

### Dejanska meritev medcelinskega omrežja WAN (Primerjava `rsync`)

Dejansko merilno okolje: Združene države `c1` → Evropa `n1` (RTT 65 ms), nabor podatkov je jedro Linuxa `linux-7.3.3`:

#### 1) Način a la carte proti `rsync`

| Nabor podatkov | velikost | `sync0` Ena točka<br>(Mb/s) | `rsync`<br>(Mbps) | Pospešitev (`sync0` / `rsync`) |
| :--- | ---: | ---: | ---: | ---: |
| `linux-7.3.3.tar.xz`(ni stisnjen) | 155 MB | **121.07** | 107.89 | **1,12×** |
| `linux-7.3.3.tar`(stisljivo) | 1.6 GB | **442.07** | 506.62 | **0,87×** |
| `linux-7.3.3.tar`(lokalni korak spremembe) | 1.6 GB | **8,123.87** | 1,169.49 | **6,95×** |
| `linux-7.3.3/` (95.979 majhnih datotek) | 1.5 GB | **565.49** | 235.27 | **2,40×** |
| **geometrijska sredina** | — | **704.17** | 350.19 | **2,01×** |

#### 2) Dvojni izvorni način P2P proti `rsync`

| Nabor podatkov | velikost | `sync0` Dvojni vir P2P<br>(Mbps) | `rsync`<br>(Mbps) | Pospešitev (`sync0` / `rsync`) |
| :--- | ---: | ---: | ---: | ---: |
| `linux-7.3.3.tar.xz`(ni stisnjen) | 155 MB | **466.85** | 107.89 | **4,33×** |
| `linux-7.3.3.tar`(stisljivo) | 1.6 GB | **992.17** | 506.62 | **1,96×** |
| `linux-7.3.3.tar`(lokalni korak spremembe) | 1.6 GB | **5,666.84** | 1,169.49 | **4,85×** |
| `linux-7.3.3/` (95.979 majhnih datotek) | 1.5 GB | **703.30** | 235.27 | **2,99×** |
| **geometrijska sredina** | — | **1,165.63** | 350.19 | **3,33×** |

> Opomba: V načinu dvojnega vira P2P evropska vozlišča `s1` sodelujejo pri sejanju, RTT medcelinske hrbtenice pa je 65 ms.

### Glavne lastnosti izdelka

- **Samodejno luknjanje naslavljanja javnih ključev**: Ni potrebe po centralnem strežniku in naslovu javnega omrežja, samodejno luknjanje NAT na podlagi naslova javnega ključa porazdeljenega omrežja.
- **Sinhronizacija v realnem času na ravni milisekunde in zaznavanje metapodatkov**: Temelji na izvornem nadzoru dogodkov v datotekah (`FSEvents` / `inotify` / `ReadDirectoryChangesW`), sprotnem potiskanju dodajanja, brisanja datotek, dovoljenj POSIX (`mode`) in časa spreminjanja na ravni milisekunde (`mtime`); prenos ničelnega bloka podatkov, ko so spremenjena samo dovoljenja ali čas.
- **Hkraten prenos in nalaganje v slogu BT ter načrtovanje regionalne afinitete**: vozlišču za prenos ni treba čakati na prenos celotne količine. Vsi blokovni intervali (`Op::Have`), ki so prestali 128-bitno preverjanje zgoščevanja, se takoj zaklenejo v pomnilniški/začasni bazen, da lahko druga vozlišča sočasno vlečejo. Skupaj z dinamično izmenjavo vozlišč PEX in počasnim prevzemom polovice vozlišča (kraja dela) je prepustnost gruče največja.
- **Odpornost proti zlonamernim posegom v vozlišče in zastrupitev gruče**: verodostojen seznam virov `root_hash` Samodosledno zaklepanje + obvezno pospeševanje strojne opreme `gxhash128` preverjanje prebave, preden je vsak blok nameščen na disk in zunanje zaseden; če je najden spremenjen blok, bo odklopnik na milisekundni ravni zatemnil zlonamerno vozlišče in se samodejno obnovil iz poštenega vira.
- **FastCDC in situ inkrement in združevanje majhnih datotek**: za delno spreminjanje velikih datotek se prenesejo samo diferencialni bloki in postavijo na disk; ogromna pomnilniška embalaža za majhne datoteke in diferencialno stiskanje predpone odpravita dodatne stroške povratnega potovanja po datotekah.
- **Samoobnavljanje po izpadu električne energije in nadaljevanje po ponovnem zagonu**: Redisu podoben hibridni format RDB + AOF (`.s0_subs.bin`) se uporablja za ohranjanje statusa sejanja in seznama datotek, dnevne spremembe so dodane v mikrosekundah in se samodejno prepisujejo in redno organizirajo. Po ponovnem zagonu strežnika se spremembe brez povezave samodejno potisnejo v sejalno vozlišče.

## Tehnična arhitektura in dokumentacija

- **Protokol ločevanja dvojne ravnine**:
  - **Površina nadzora in spreminjanja v realnem času (`iroh` QUIC / TCP, prvi bajt `PROTO_VERSION` rokovanje različice) **: Odgovoren za preverjanje vizitke Ed25519, preverjanje dovoljenj, odkrivanje semena roja in podpora za paketno pretakanje množičnih sprememb ravni `u64::MAX` in `mzstd` stisnjeno `SubPush` potiskanje v realnem času (`FsOp::Upsert` / `FsOp::Remove` / `FsOp::Meta` Tri primitive).
  - **Podatkovna površina za distribucijo (izvorni UDP + `compio` asinhroni V/I) **: 20B kompaktna glava, 18B TACK periodične povratne informacije, BBRv3/Copa hibridni nadzor zastojev, RACK-TLP obnovitev izgube paketov, obvod kompresije Zstd z prilagoditvijo entropije prvega bloka Shannon in `gxhash128` preverjanje od konca do konca bloka.
- **Več o tem**:
  - [Poročilo o merjenju uspešnosti in scenarij za reprodukcijo](https://github.com/webc-site/sync0/blob/main/doc/zh/benchmark.md)
  - [Načrtovanje sistemske arhitekture](https://github.com/webc-site/sync0/blob/main/doc/zh/design_v4.md)
  - [Protokol binarnega kabla in specifikacija obstojnosti](https://github.com/webc-site/sync0/blob/main/doc/zh/proto.md)
  - [Preverjanje pristnosti identitete, nadzor avtoritete in preverjanje integritete](https://github.com/webc-site/sync0/blob/main/doc/zh/crypto.md)