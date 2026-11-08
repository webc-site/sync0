# sync0

`sync0` Peer-to-peer (P2P) fitxategiak partekatzeko eta hainbat makina sinkronizatzeko tresna bat da, Resilio Sync-ekin lehiatzen dena.

Makina bakoitza zerbitzari bat eta bezero bat da: edozein nodok fitxategiak partekatzeko hainbat talde argitaratu ditzake. Beste nodo batzuk elkartu ondoren, automatikoki eskura ditzakete hazi-nodoak pareko zerbitzaritik eta aldi berean hainbat iturritatik atera ditzakete. Sinkronizazioa amaitu ondoren, haziak automatikoki sortuko dira deskargak azkartzeko nodo gehiagorentzat.

## Oinarrizko Nabarmenak

- **Denok zerbitzaria eta bezeroa dira**: Resilio Sync-en parekoa, zerbitzari zentralaren menpekotasunik gabe. Edozein nodok fitxategiak partekatzeko hainbat talde sor ditzake, eta beste pertsonen partekatze taldeetan ere bat egin dezake; nodoek automatikoki trukatzen dituzte eta eskuragarri dauden hazi-nodoak lortzen dituzte makina anitzen aldibereko tiraketa eta denbora errealean sinkronizatzeko.
- **IP gabeko nodoaren helbidea**: lehenespenez, nodoaren gakoa eta `base64url` kodetutako `iroh` helbidea `~/.config/sync0`n sortzen dira automatikoki IP publikoa gogoratu edo agerian utzi beharrik gabe.
- **Eskari anitzeko sinkronizazioa**: partekatze-talde osoaren sinkronizazioa onartzen du, eta eskaeraren araberako sinkronizazioa ere onartzen du hainbat fitxategi edo azpidirektorio hautatuz aldi berean.
- **Taldeko kideen hierarkizazio-baimenak eta karga-direktorioaren kontrola**: irakurtzeko soilik (`ro`), irakurtzeko-idazteko kargatzeko (`rw`) eta kudeatzeko (`admin`) baimenak onartzen ditu, eta kide bakoitzak kargatzeko baimena duen bide-aurrizkia banan-banan zehaztu dezake (lehenespenez, partekatutako direktoriora kargatzea baimenduta dago).
- **Abiadura osoko eta inkrementala sinkronizazioa sareetan**: fitxategi handien aldibereko transmisioa, fitxategi txiki masiboak memorian gehitzea eta fitxategiak partzialki aldatzen direnean soilik sinkronizatutako datu-blokeak.

## Abiadura konparaketa rsync-ekin

### Sare transozeanikoa iturri bakarreko benetako neurketa (Txina → Estatu Batuak, 260 ms atzerapena)

| Transmisioaren eszenatokia | Datuen tamaina | eredua | `sync0` | `rsync -az` | Handitu biderkatzailea |
| :--- | :---: | :---: | :---: | :---: | :---: |
| 500 fitxategi txiki | 1.91 MB | Sinkronizazio osoa | 18.84 Mbps | 5.32 Mbps | 3.5x |
| 500 fitxategi txiki | 1.91 MB | Sinkronizazio inkrementala | 27.15 Mbps | 3.83 Mbps | 7.1x |
| Kode eta artxibo paketeak (`.tar`) | 10.0 MB | Sinkronizazio osoa | 139.70 Mbps | 24.97 Mbps | 5.6x |
| Kode eta artxibo paketeak (`.tar`) | 10.0 MB | Sinkronizazio inkrementala | 83.38 Mbps | 26.02 Mbps | 3.2x |
| Bideo-fitxategia (`.mp4`) | 15.0 MB | Sinkronizazio osoa | 54.06 Mbps | 27.04 Mbps | 2.0x |
| Bideo-fitxategia (`.mp4`) | 15.0 MB | Sinkronizazio inkrementala | 216.83 Mbps | 21.51 Mbps | 10.1x |

### Sare transozeanikoa P2P nodo anitzeko tiraketa proba (Txina → Estatu Batuetako nodo bikoitzak, 260 ms atzerapena)

Iturburu anitzeko hazi-nodo marradun tira aldiberekoaren konparaketa neurtua:

| Transmisioaren eszenatokia | Datuen tamaina | Nodo bakarreko hazia | Bi nodo P2P aldibereko hazia | Handitu biderkatzailea |
| :--- | :---: | :---: | :---: | :---: |
| 500 fitxategi txiki | 1.97 MB | 5.28 Mbps | 7.87 Mbps | 1.5x |
| Kode eta artxibo paketeak (`.tar`) | 30.0 MB | 56.35 Mbps | 83.17 Mbps | 1.5x |
| Bideo-fitxategia (`.mp4`) | 30.0 MB | 27.00 Mbps | 39.46 Mbps | 1.5x |

### Abiadura handiko linea dedikatu transkontinentalaren benetako neurketa (Europa → Ipar Amerika, 86 ms atzerapena)

| Transmisioaren eszenatokia | Datuen tamaina | `sync0` | `rsync` | Handitu biderkatzailea |
| :--- | :---: | :---: | :---: | :---: |
| 10.000 fitxategi txiki | 77.6 MB | 688 Mbps | 128 Mbps | 5.4x |
| Kode eta artxibo paketeak (`.tar`) | 512 MB | 2111 Mbps | 438 Mbps | 4.8x |
| Bideo-fitxategia (`.mp4`) | 512 MB | 556 Mbps | 338 Mbps | 1.6x |
| Sinkronizazio gehigarria (aldatu % 1eko edukia) | 512 MB | 34133 Mbps | 2214 Mbps | 15.4x |

## Instalatu

### 1. metodoa: instalatu zama bidez

```bash
cargo install sync0
```

### 2. metodoa: deskargatu aurrez konpilatutako bitarrak

Joan [GitHub Releases](https://github.com/webc-site/sync0/releases) atalera dagokion sistemarako `.tar.xz` pakete konprimitua deskargatzeko (Linux, macOS, Windows, Android, FreeBSD, NetBSD onartzen ditu), deskonprimitu eta jarri `PATH` direktorioan:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Hasi azkar

`sync0`k `~/.config/sync0` erabiltzen du konfigurazio direktorio gisa lehenespenez (beste direktorio batzuetara aldatu behar baduzu, `-c <directory>` gehi dezakezu).

### 1. Ikusi tokiko nodoaren helbidea eta bisita-txartela ezarri

```bash
# Ikusi tokiko nodoaren helbidea (gakoa automatikoki sortzen da ~/.config/sync0 lehen aldiz)
sync0 id

# Sortu nodo pertsonaleko bisita-txartel bat (erabiltzaile-izena, hiria, posta elektronikoa, webgunea eta sinadura onartzen ditu 256 hitzetan)
sync0 profile -u alice --region SHA --email alice@example.com --url https://example.com -b "Shanghai node"
```

### 2. Argitaratu fitxategiak partekatzeko hainbat talde eta ezarri taldekideen baimenak (`.nt` Konfigurazioa)

`sync0` [NestedText](https://nestedtext.org) (`.nt`) formatuan konfiguratuta dago. Erabiltzaile bakoitzak `.nt` konfigurazio-fitxategi bat du. Erabiltzaile-profilaren direktorioa `users_dir` bidez zehaz dezakezu `config.nt`-n (direktorio honetako `.nt` fitxategi guztiak automatikoki barne hartzen ditu). Bai erabiltzaileak bai kargatu daitekeen bide-aurrizkiak ordenatu gabeko zerrenda bat erabiltzen dute (`-`). **Kargatzeko bide-aurrizkia betetzen ez denean, partekatutako erroko direktoriora kargatzea onartzen da lehenespenez**.

1. Sortu `.nt` konfigurazio-fitxategiak `~/.config/sync0/users/` direktorioko kide bakoitzarentzat:

`~/.config/sync0/users/alice.nt` (irakurtzeko soilik den kidea, tarifa mugatua `80MB`, `rate`k `GB`, `MB`, `KB` unitateak onartzen ditu):

```yaml
id: 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c
perm: ro
rate: 80MB
```

`~/.config/sync0/users/bob.nt` (kargatzeko kideak mugatuak: `uploads/bob` eta `shared/common` direktorioetara soilik kargatzeko baimena):

```yaml
id: 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f
perm: rw
write_prefix:
  - uploads/bob
  - shared/common
```

`~/.config/sync0/users/charlie.nt` (Kideen karga lehenetsia: `write_prefix` baztertzen denean, partekatutako erroko direktoriora kargatzea onartzen da lehenespenez):

```yaml
id: 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
perm: rw
```

2. Konfiguratu erabiltzailearen direktorioa eta fitxategiak partekatzeko hainbat talde `~/.config/sync0/config.nt`-n:

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

Hasi zerbitzua partekatzen:

```bash
# Zuzeneko abiarazte (automatikoki kargatzen ditu .nt erabiltzaileen konfigurazio guztiak ~/.config/sync0/config.nt eta users_dir direktorioetan)
sync0 share --region SHA

# Edo bizkor argitaratu hainbat partekatze-talde eta baimendu taldekideei zuzenean komando-lerroaren bidez
sync0 share dataset:./data team-vault:./vault \
  --auth \
  --allow-ro 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c \
  --allow-rw 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f:uploads/bob,shared/common \
  --allow-rw 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
```

### 3. Atera eduki guztia edo hautatu hainbat fitxategi eta direktorio (`sync0 pull`)

Konektatu pareko edozein nodotara partekatzeko taldean eskuragarri dauden hazi-nodo guztiak automatikoki lortzeko eta atera hainbat hazi-nodotatik aldi berean:

```bash
# Eraman datu-multzoa partekatzeko talde osoa ./local_data lokalera
sync0 pull dataset:// ./local_data -p <nodoaren helbidea>

# Aukera anitzeko sinkronizazioa: aldi berean atera fitxategi bakarra docs/readme.md eta azpidirektorioak aktibo/ikonoak, src/core
sync0 pull dataset://docs/readme.md,assets/icons,src/core ./local_data -p <nodoaren helbidea>
```

### 4. Kargatu fitxategi edo direktorio bat partekatzeko talde batera (`sync0 push`)

Kargatzeko baimenak dituzten kideek (`rw` edo `admin`) tokiko fitxategiak edo direktorioak karga ditzakete urruneko partekatzeko taldeetara:

```bash
# Erro-direktorioa kargatzeko baimena duten kideak: zuzenean kargatu partekatutako erro-direktoriora
sync0 push team-vault:// ./new_release -p <nodoaren helbidea>

# Aurrizki jakin baterako baimendutako kideak soilik (adibidez, uploads/bob): kargatu baimendutako bide-aurrizkira
sync0 push team-vault://uploads/bob/report.pdf ./report.pdf -p <nodoaren helbidea>
```

### 5. Harpidetu sinkronizazio eta hazi automatikorako (`sync0 sub`)

Nodo jakin baten partekatze-taldean harpidetu ondoren, tokiko zerbitzaria bezero hartzailea eta hazi-zerbitzari berria da; iturburu-nodoaren fitxategia eguneratzen denean, automatikoki sinkronizatzen da eta beste nodo batzuen deskarga zatiak eskaintzen ditu:

```bash
# Harpidetu partekatzeko talde osora eta automatikoki hazia
sync0 azpidatu multzoa:// ./local_mirror -p <nodoaren helbidea>

# Aukeratu anitzeko soilik zehaztutako fitxategi eta azpidirektorioetara harpidetzeko eta hazteko
sync0 azpidatu multzoa://models/llama.onnx,configs ./local_mirror -p <nodoaren helbidea>
```

### 6. Makina anitzeko taldeen parekoen sinkronizazioa (`sync0 group`)

Makina bat baino gehiago partekatze talde berean sartzen dira, batak besteek automatikoki deskubritzen dituzte eskuragarri dauden hazi-nodoak eta bi norabideko sinkronizazioa mantentzen dute (sinkronizazio osoa edo fitxategi eta direktorioen aukeraketa anitz onartzen du):

```bash
# Erabat sartu taldean eta bi norabideko sinkronizazioa
sync0 group team-docs:// ./docs -p <nodoaren helbidea>

# Aukera anitzeko sinkronizazio taldean wal direktorioa eta meta.json fitxategia bakarrik
sync0 group team-docs://wal,meta.json ./docs -p <nodoaren helbidea>
```

### 7. Bi makinen arteko puntuz puntuko transmisio zuzena (`send` / `recv`)

Ez dago talde bat aurrez konfiguratu beharrik, eta aldi baterako fitxategiak edo direktorioak zuzenean transferi ditzakezu bi makinen artean:

```bash
# Hartzailearen jarraipena
sync0 recv 0.0.0.0:9000 ./backup

# Bidaltzen duen amaierak zuzenean transferitzen ditu fitxategiak edo direktorioak (aukerakoa --delete ispilua gehiegizko fitxategiak garbitzeko, --rate abiadura muga)
sync0 send ./project 192.168.1.10:9000
```

### 8. Hizkuntza anitzeko aldaketa

`sync0` 75 hizkuntza onartzen ditu, eta sistemaren hizkuntza-ingurunearen arabera automatikoki aldatzen da lehenespenez; eskuz ere zehaztu daiteke `SYNC0_LANG` bidez:

```bash
SYNC0_LANG=en sync0 --help
SYNC0_LANG=ja sync0 --help
SYNC0_LANG=de sync0 --help
```