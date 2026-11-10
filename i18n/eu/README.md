# sync0

`sync0` `rsync` ordezkatzeko erabiltzen den P2P fitxategiak partekatzeko, denbora errealeko sinkronizazioa eta iturri anitzeko banaketa tresna deszentralizatua da - WAN transkontinentalaren neurtutako abiadura `rsync`ren **5,89×** (puntu bakarreko batez besteko abiadura geometrikoa **2,67×**, P2P batez besteko abiadura **-36) eta trans-16ko batez besteko abiadura **. latentzia handiko lotura P2P azelerazioa **7,68×** artekoa da (24 agertokiko lotura osoko batez besteko abiadura geometriko osoa **2,56×**).

Makina bakoitza zerbitzari bat eta bezero bat da: ez dago zerbitzari zentral bat edo sare publikoko helbiderik behar, zulo-zulaketa automatikoa sare banatutako gako publikoaren helbidean oinarritzen da, eta bi makinen arteko zuzeneko transmisioa onartzen du, partekatzeko talde anitzeko baimena ostatzea, azpidirektorioen aukeraketa anitz eskaeraren arabera, milisegundo-mailako denbora errealeko sinkronizazio inkrementala eta P2P iturri anitzeko azelerazioa.

## Instalatu

### 1. Metodoa: Karga instalatzea

```bash
cargo install sync0
```

### 2. metodoa: deskargatu aurrez konpilatutako bitarrak

Joan [GitHub Releases](https://github.com/webc-site/sync0/releases) atalera sistemari dagokion `.tar.xz` pakete konprimitua (Linux, macOS, Windows, Android, iOS, FreeBSD onartzen du), deskonprimitu eta `PATH`-n sartu:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Hasi azkar

### 1. Zero konfigurazio zuzeneko transmisioa bi makinen artean (`send` / `recv`)

Ez da partekatzeko talde bat konfiguratu beharrik, zuzenean transferitu fitxategiak edo direktorioak bi makinen artean abiadura handian:

```bash
# Hartzaileak portuan entzuten du eta ./backup-era idazten du
sync0 recv 0.0.0.0:9000 ./backup

# Igorlearen zuzeneko transmisioa (aukerakoa --ezabatu ispilua gehiegizko fitxategiak garbitzeko, --tasa 50 MBko abiadura muga)
sync0 send ./project 192.168.1.10:9000
```

### 2. Argitalpenak partekatzea eta eskaeraren arabera atera/kargatu (`share` / `pull` / `push`)

```bash
# Zerbitzaria: argitaratu partekatzeko taldea komando-lerro batekin (irteera IP gabeko iroh nodoaren helbidea abiarazi ondoren)
sync0 share dataset:./data

# Bezeroa: atera datu-multzoa partekatzeko talde osoa ./local_data lokalera
sync0 pull dataset:// ./local_data -p <nodoaren helbidea>

# Bezeroa: hautapen anitz atera eskaeraren arabera (sinkronizatu zehaztutako fitxategi eta azpidirektorio bakarrak soilik)
sync0 pull dataset://docs/readme.md,assets/icons,src/core ./local_data -p <nodoaren helbidea>

# Bezeroa: kargatu fitxategiak edo direktorioak baimendutako partekatze-taldeetara
sync0 push dataset://releases/v1 ./build_output -p <nodoaren helbidea>
```

### 3. Harpidetza denbora errealean hastea eta makina anitzeko bi norabideko sinkronizazioa (`sub` / `group`)

Konexio luze bat ezarri ondoren, fitxategiren bat **gehitu, aldatu, ezabatu, baimenak aldatu edo aldatzeko denbora** urruneko muturrean gertatzen bada, tokiko direktorioa milisegundotan sinkronikoki eguneratuko da denbora errealean (fitxategi handi berriek/aldatutakoek diferentzia-blokeak bakarrik aterako dituzte eta bere tokian eguneratuko dituzte, fitxategiak ezabatzeak automatikoki garbituko ditu tokiko fitxategiak eta direktorio hutsak, eta zuzenean datu-baimen-blokeak edo denbora-ztapatuak bakarrik aldatuko ditu):

- **`sync0 sub`(Noranzko bakarreko denbora errealeko ispilua + hazi automatikoa)**: tokiko denbora errealean partekatzeko amaierako gehiketak, ezabaketak eta aldaketak jarraitzen ditu eta automatikoki P2P hazi-iturri gisa balio du beste tira-nodo batzuei aldi berean azelerazioa emateko.
- **`sync0 group`(Makina anitzeko peer-to-peer bi norabideko denbora errealeko sinkronizazioa)**: fitxategiak gehitu, ezabatu edo aldatzen badira taldeko edozein makinatan, lineako gainerako nodo guztiak denbora errealean sinkronizatuko dira eta elkarren hazi gisa balioko dute.

```bash
# Harpidetu partekatzeko talde osora: urruneko gehiketak, ezabaketak eta aldaketak denbora errealean lokalean sinkronizatzen dira eta automatikoki hazten dira.
sync0 azpidatu multzoa:// ./local_mirror -p <nodoaren helbidea>

# Eskaera anitzeko harpidetza: zehaztutako fitxategi eta azpidirektorio bakarrak denbora errealean sinkronizatu eta hazi.
sync0 azpidatu multzoa://models/llama.onnx,configs ./local_mirror -p <nodoaren helbidea>

# Makina anitzeko pareko talde bi norabideko denbora errealeko sinkronizazioa (hautapen anitzeko bide osoa edo koma ere onartzen du)
sync0 group team-docs:// ./docs -p <nodoaren helbidea>
```

### 4. Taldearen partekatze anitzeko taldea eta kideen baimenen konfigurazioa (`.nt` konfigurazioa)

Lehenetsitako konfigurazio-direktorioa `~/.config/sync0` da (`-c <directory>` bidez alda daiteke), [NestedText](https://nestedtext.org) (`.nt`) formatu minimalista erabiliz:
- **Sare publikoaren irakurketa mugatzea onartzen du (baimendutako erabiltzaileek bakarrik irakur dezakete)**: partekatze-taldea `users:` kideen zerrenda batekin konfiguratuta badago (edo komando-lerroak `--allow-r` / `--allow-rw` / `--allow-w` / `--auth` zehazten badu), sistemak **automatikoki aktibatzen du sare pribatua eta publiko anonimoa (autentifikazioa) debekatuta. nodoen zenbaketa edo irakurketa, eta P2P hazi-nodoetara harpidetzeak zerrenda zuria ere heredatuko du aldi berean, baimendutako erabiltzaileek soilik irakur dezaten; `users`-rekin konfiguratuta ez dauden taldeak publikoak eta irakurtzeko soilik dira lehenespenez (publikoa eta irakurtzeko soilik izan nahi baduzu eta zehaztutako kideentzat soilik kargatzeko irekita, `auth: false` esplizituki ezar dezakezu).
- **Irakurketa-idazketa eta azpidirektorioa bidaltzeko baimen zehatzak**: kideak irakurtzeko soilik dira lehenespenez (`r` ez da esplizituki idatzi beharrik) eta `r` (irakurtzeko soilik), `w` (jarraipen-kutxan bakarrik idatzi, irakur daitekeena), `rw`" eta zuzeneko `rw`"> eta zuzeneko `rw` (irakurtzeko) kargatzeko baimena).

**Kideen konfigurazio adibidea (`~/.config/sync0/users/`):**

```yaml
# ~/.config/sync0/users/alice.nt (irakurtzeko soilik lehenetsia, abiadura muga 80 MB)
id: 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c
rate: 80MB
```

```yaml
# ~/.config/sync0/users/bob.nt (irakurtzeko eta idazteko baimenak, zehaztutako azpidirektoriora soilik kargatzeko baimena)
id: 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f
perm: rw
write_prefix:
  - uploads/bob
  - shared/common
```

```yaml
# ~/.config/sync0/users/charlie.nt (bakarrik idazteko baimena, irakurtzea debekatuta dago)
id: 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
perm: w
```

**Taldearen konfigurazio-adibidea partekatzeko (`~/.config/sync0/config.nt`):**

```yaml
users_dir: ./users

groups:
  # Konfiguratu gabeko erabiltzaileak: sare publiko lehenetsia irakurtzeko soilik
  dataset:
    path: ./data
    rate: 50MB

  # Konfiguratutako erabiltzaileak: sare publikoko irakurketa automatikoki mugatu (alicek eta bob-ek bakarrik irakur dezakete, charlie-k entregatu eta kargatu bakarrik egin dezake)
  team-vault:
    path: ./vault
    users:
      - alice
      - bob
      - charlie
```

Hasi zerbitzua (kargatu konfigurazio fitxategia edo zuzenean erabili komando-lerroko parametroak sare publikoaren irakurketa mugatzeko eta kideak baimentzeko):

```bash
# Kargatu ~/.config/sync0/config.nt hasteko
sync0 share --region SHA

# Edo argitaratu partekatze-talde pribatu bat komando-lerro hutsaren bidez (zehaztu --allow-* sare publikoaren irakurketa automatikoki mugatzeko eta baimendutako erabiltzaileek soilik atzi dezakete).
sync0 share team-vault:./vault \
  --allow-r 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c \
  --allow-rw 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f:uploads/bob,shared/common \
  --allow-w 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
```

### 5. Nodoaren helbidea, bisita-txartela eta hizkuntza anitzekoa

```bash
# Ikusi IPrik gabeko tokiko nodoaren helbidea (gakoa automatikoki sortzen da lehen exekuziorako)
sync0 id

# Sortu Ed25519 norberak sinatutako nodoaren bisita-txartela (ezizena, hiri kodea, posta elektronikoa, webgunea, aurkezpena)
sync0 profile -u alice --region SHA --email alice@example.com -b "Shanghai node"

# 75 hizkuntza integratu daude, sistemaren hizkuntza lehenespenez jarraitzen dutenak eta ingurune-aldagaien bidez alda daitezkeenak.
SYNC0_LANG=en sync0 --help
```

## Errendimendua eta oinarrizko ezaugarriak

### WAN kontinental arteko benetako neurketa (Konparazioa `rsync`)

Benetako neurketa ingurunea: Estatu Batuak `c1` → Europa `n1` (RTT 65 ms), datu-multzoa Linux kernel `linux-7.3.3` da:

#### 1) Kartako modua vs `rsync`

| Datu multzoa | tamaina | `sync0` Puntu bakarra<br>(Mbps) | `rsync`<br>(Mbps) | Bizkortzea (`sync0` / `rsync`) |
| :--- | ---: | ---: | ---: | ---: |
| `linux-7.3.3.tar.xz`(ez da konprimigarria) | 155 MB | **236.48** | 108.74 | **2,17×** |
| `linux-7.3.3.tar`(konprimagarria) | 1.6 GB | **843.83** | 385.06 | **2,19×** |
| `linux-7.3.3.tar`(tokiko aldaketaren gehikuntza) | 1.6 GB | **8,014.15** | 1,361.72 | **5,89×** |
| `linux-7.3.3/` (95.979 fitxategi txiki) | 1.5 GB | **627.58** | 347.80 | **1,80×** |
| **batez besteko geometrikoa** | — | **1,000.91** | 375.26 | **2,67×** |

#### 2) P2P iturri bikoitzeko modua vs `rsync`

| Datu multzoa | tamaina | `sync0` P2P iturri bikoitza<br>(Mbps) | `rsync`<br>(Mbps) | Bizkortzea (`sync0` / `rsync`) |
| :--- | ---: | ---: | ---: | ---: |
| `linux-7.3.3.tar.xz`(ez da konprimigarria) | 155 MB | **330.09** | 108.74 | **3,04×** |
| `linux-7.3.3.tar`(konprimagarria) | 1.6 GB | **1,551.86** | 385.06 | **4,03×** |
| `linux-7.3.3.tar`(tokiko aldaketaren gehikuntza) | 1.6 GB | **5,474.17** | 1,361.72 | **4,02×** |
| `linux-7.3.3/` (95.979 fitxategi txiki) | 1.5 GB | **706.56** | 347.80 | **2,03×** |
| **batez besteko geometrikoa** | — | **1,186.42** | 375.26 | **3,16×** |

> Oharra: P2P iturburu bikoitzeko moduan, Europako nodo `s1` elkarlanean aritzen dira hazietarako, eta kontinente gurutzatuaren bizkarrezurra RTT 65 ms-koa da.

### Produktuaren oinarrizko ezaugarriak

- **Gako publikoa zulaketa automatikoa zuzentzen duena**: Ez da zerbitzari zentralaren eta sare publikoaren helbiderik behar, NAT zuloen zulaketa automatikoa sare banatutako gako publikoaren helbidean oinarrituta.
- **Milisegundo-mailako denbora errealeko sinkronizazioa eta metadatuen kontzientzia**: jatorrizko fitxategien gertaeren monitorizazioan (`FSEvents` / `inotify` / `ReadDirectoryChangesW`), fitxategien gehikuntzak, ezabatzeak, POSIX baimenak (`mode`) eta milisegundo-mailako aldaketen denbora (`mtime`) denbora errealean oinarrituta; zero datu-blokearen transmisioa baimenak edo ordua bakarrik aldatzen direnean.
- **BT estiloko aldibereko deskarga eta igoera eta eskualdeko afinitate-programazioa**: deskargatzeko nodoak ez du itxaron behar zenbateko osoa deskargatu arte. 128 biteko hash egiaztapena gainditu duten bloke-tarte guztiak (`Op::Have`) berehala blokeatzen dira memoria/aldi baterako multzoan beste nodo batzuk aldi berean atera ditzaten. PEX nodoen truke dinamikoarekin eta nodo motelaren erdi-prebentzioarekin batera (Lana-lapurtzea), kluster-eko transmisioa maximizatzen da.
- **Maltzurren aurkako nodoen manipulazioa eta klusterren pozoitzearen immunitatea**: iturri zerrenda autoritarioa `root_hash` Blokeo koherentea + hardware-azelerazio derrigorrezkoa `gxhash128` digestio egiaztapena bloke bakoitza diskoan jarri eta kanpoan hazi aurretik; manipulatutako bloke bat aurkitzen bada, milisegundoko etengailuak nodo gaiztoa itzaliko du eta iturri zintzotik automatikoki berreskuratuko du.
- **FastCDC in-situ gehikuntza eta fitxategien agregazio txikia**: bloke diferentzialak soilik transferitzen dira fitxategi handien aldaketa partzialetarako eta diskoan jartzen dira; Fitxategi txikien memoria masiboak eta aurrizkiaren konpresio diferentziala ezabatzen dute fitxategiz fitxategi joan-etorriko gastuak.
- **Auto-sendapena energia eten ondoren eta berrabiarazi ondoren berrabiarazi**: RDB + AOF formatu hibridoa (`.s0_subs.bin`) erredis-antzeko erabiltzen da hazi-egoera eta fitxategi-zerrenda mantentzeko, eguneroko aldaketak mikrosegundotan gehitzen dira eta automatikoki berridatzi eta aldian-aldian antolatzen dira. Zerbitzaria berrabiarazi ondoren, lineaz kanpoko aldaketak automatikoki hazi-nodora eramaten dira.

## Arkitektura teknikoa eta dokumentazioa

- **Plano bikoitzeko desakoplazio protokoloa**:
  - **Kontrol eta denbora errealeko aldaketa gainazala (`iroh` QUIC / TCP, lehen bytearen `PROTO_VERSION` bertsioaren esku-ematea) **: Ed25519 bisita-txartelaren egiaztapenaren arduraduna, baimenen egiaztapenaren, Swarm hazien aurkikuntzaren eta `u64::MAX` maila-aldaketa masiboaren erreprodukzio-multzoaren laguntza eta `mzstd` id="b-time push" (`mzstd` id="5">b-denbora errealean konprimitu) `FsOp::Remove` / `FsOp::Meta` Hiru primitibo).
  - **Datuen banaketa gainazala (UDP + `compio` I/O asinkronoa) **: 20B goiburu trinkoa, 18B TACK feedback periodikoa, BBRv3/Copa pilaketa hibridoaren kontrola, RACK-TLP pakete-galera berreskuratzea, Shannon lehen blokeko entropia moldagarria Zstd konpresioaren saihesbidea eta <b id-="2"> amaierako egiaztapen-blokea.
- **Lortu informazio gehiago**:
  - [Errendimendua neurtzeko txostena eta erreprodukzio-gidoia](https://github.com/webc-site/sync0/blob/main/doc/zh/benchmark.md)
  - [Sistemaren arkitekturaren diseinua](https://github.com/webc-site/sync0/blob/main/doc/zh/design_v4.md)
  - [Hari bitarren protokoloa eta iraunkortasunaren zehaztapena](https://github.com/webc-site/sync0/blob/main/doc/zh/proto.md)
  - [Identitatearen autentifikazioa, autoritate kontrola eta osotasuna egiaztatzea](https://github.com/webc-site/sync0/blob/main/doc/zh/crypto.md)