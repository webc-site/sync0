# sync0

`sync0` ass en dezentraliséierte P2P Datei Sharing, Echtzäit Synchroniséierung a Multi-Source Verdeelungsinstrument dat benotzt gëtt fir `rsync` ze ersetzen - déi gemoossene Geschwindegkeet vum transkontinentale WAN ass bis zu `rsync`'s **5.89×** (Eenpunkt geometresch Duerchschnëttsgeschwindegkeet **2.67×-** ** Source Dual Geschwindegkeet **, P2P. transoceanic High-latency Link P2P Beschleunegung ass bis zu **7.68×** (Volllink 24-Szenario ëmfaassend geometresch Mëttelgeschwindegkeet **2.56×**).

All Maschinn ass souwuel e Server wéi och e Client: et gëtt kee Besoin fir en zentrale Server oder eng ëffentlech Netzwierkadress, automatesch Lachpunching baséiert op der verdeelt Netzwierk ëffentlech Schlësseladress, an et ënnerstëtzt direkt Iwwerdroung tëscht zwou Maschinnen, Multi-Sharing Group Permission Hosting, Multi-Sharing Group Permission Hosting, Multi-Sharing Group Erlaabnis Hosting, Multi-Sharing Group Subdirectories on Demand, Millisekonnen-Niveau Echtzäit inkrementell Synchroniséierung a P2P Multi-Source Beschleunegung.

## Installéieren

### Method 1: Cargo Installatioun

```bash
cargo install sync0
```

### Method 2: Luet virkompiléiert Binären erof

Gitt op [GitHub Releases](https://github.com/webc-site/sync0/releases) fir de `.tar.xz` kompriméierte Package erofzelueden deen dem System entsprécht (ënnerstëtzt Linux, macOS, Windows, Android, iOS, FreeBSD), unzip an `PATH` setzen:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Start séier

### 1. Nullkonfiguratioun direkt Iwwerdroung tëscht zwou Maschinnen (`send` / `recv`)

Kee Grond fir eng Deelegrupp ze konfiguréieren, direkt Dateien oder Verzeichnisser tëscht zwou Maschinnen mat héijer Geschwindegkeet ze transferéieren:

```bash
# De Empfangsend lauschtert um Hafen a schreift op ./Backup
sync0 recv 0.0.0.0:9000 ./backup

# Direkt Iwwerdroung vum Sender (optional --Spigel läschen fir iwwerschësseg Dateien ze botzen, --rate 50MB Geschwindegkeetslimit)
sync0 send ./project 192.168.1.10:9000
```

### 2. Postdeelen an on-demand Pull/Upload (`share` / `pull` / `push`)

```bash
# Server: Verëffentlecht den Deelegrupp mat enger Kommandozeil (ausgab déi IP-fräi iroh Node Adress nom Startup)
sync0 share dataset:./data

# Client: Pull de ganzen Datesetsdeelegrupp op déi lokal ./local_data
sync0 pull dataset:// ./local_data -p <Node address>

# Client: Pull Multiple Selektiounen op Ufro (synchroniséiert nëmmen spezifizéiert eenzel Dateien an Ënnerverzeechnungen)
sync0 pull dataset://docs/readme.md,assets/icons,src/core ./local_data -p <Node address>

# Client: Eroplueden Dateien oder Verzeichnisser op autoriséiert Deelegruppen
sync0 push dataset://releases/v1 ./build_output -p <Node address>
```

### 3. Echtzäit Abonnement Seeding a Multi-Maschinn Zwee-Wee Synchroniséierung (`sub` / `group`)

Nodeems Dir eng laang Verbindung gegrënnt hutt, wann iergendeen Datei **Addéieren, Änneren, Läschen, Ännere vun Permissiounen oder Ännerungszäit** um Remote Enn geschitt, gëtt de lokalen Verzeechnes synchron an Millisekonnen an Echtzäit aktualiséiert (nei / geännert grouss Dateie zéien nëmmen d'Differenzblocken an aktualiséieren se op der Plaz, läschen Dateien botzen automatesch lokal Dateien an eidel Verzeichnisser op, an ännert nëmmen amp Permissiounen direkt):

- **`sync0 sub`(One-Manéier Echtzäit Spigelen + automatesch Aussaat)**: Déi lokal Echtzäit follegt d'Ergänzunge, Läschen an Ännerunge vum Deeleend, an déngt automatesch als P2P Somquell fir gläichzäiteg Beschleunegung fir aner Pullknäppchen ze bidden.
- **`sync0 group`(Multi-Maschinn peer-to-peer zwee-Wee Echtzäit Synchroniséierung) **: Wann Dateien op enger Maschinn an der Grupp bäigefüügt, geläscht oder geännert ginn, ginn all aner Online-Noden synchron an Echtzäit aktualiséiert an déngen als Somen fir all aner.

```bash
# Abonnéiert Iech op de ganzen Deelegrupp: Fern Ergänzunge, Läschen an Ännerungen ginn lokal an Echtzäit synchroniséiert an automatesch gesaat
sync0 sub dataset:// ./local_mirror -p <Node address>

# On-Demand Multi-Select Abonnement: nëmmen synchroniséiert a spezifizéiert eenzel Dateien an Ënnerverzeechnungen an Echtzäit
sync0 sub dataset://models/llama.onnx,configs ./local_mirror -p <Node address>

# Multi-Maschinn Peer Group zwee-Wee Echtzäit Synchroniséierung (ënnerstëtzt och voll oder Comma Multi-Select Weeër)
sync0 group team-docs:// ./docs -p <Node address>
```

### 4. Team Multi-Sharing Grupp a Member Erlaabnis Konfiguratioun (`.nt` Konfiguratioun)

De Standard Konfiguratiounsverzeichnis ass `~/.config/sync0` (kann iwwer `-c <Verzeechnes>` gewiesselt ginn), mam minimalistesche [NestedText](https://nestedtext.org) (`.nt`) Format:
- **Ënnerstëtzt d'Liesen vum ëffentlechen Netzwierk (nëmmen autoriséierte Benotzer kënne liesen)**: Soulaang wéi d'Deelgrupp mat enger `users:` Memberslëscht konfiguréiert ass (oder d'Kommandozeil spezifizéiert `--allow-r` / `--allow-rw` / `--allow-w` / `--auth`), schalt de System **automatesch op anonyme (<)private authentesch (<) ** anonyme Reseau (<) ** ëffentlech authentesch Netzwierker. Node Enumeratioun oder Liesen, a P2P Abonnéieren op Seeding Noden wäert och d'Whitelist gläichzäiteg ierwen, wat nëmmen autoriséiert Benotzer erlaabt ze liesen; Deelegruppen, déi net mat `users` konfiguréiert sinn, sinn ëffentlech a liesbar par défaut (wann Dir wëllt ëffentlech an nëmme liesen an op fir Eroplueden nëmme fir spezifizéierte Memberen opmaachen, kënnt Dir explizit `auth: false` setzen).
- **Fin-grained Lies-Schreiwen an Ënnerverzeechnes Liwwerung Permissiounen **: Membere sinn liesbar par défaut (`r` muss net explizit geschriwwe ginn), an Ënnerstëtzung `r` (nëmmen liesen), `w` (nëmmen an d'Dropbox schreiwen, net liesbar), `rw` an `rw` (Lies-writ e-Direktioun fir d'Ënnerstëtzung fir d'Liesen ze limitéieren) eropluede).

**Member Konfiguratioun Beispill (`~/.config/sync0/users/`):**

```yaml
# ~/.config/sync0/users/alice.nt (Standard liesbar, Geschwindegkeetslimit 80MB)
id: 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c
rate: 80MB
```

```yaml
# ~/.config/sync0/users/bob.nt (Lies- a Schreifrechter, nëmmen erlaabt fir an de spezifizéierte Ënnerverzeechnes eropzelueden)
id: 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f
perm: rw
write_prefix:
  - uploads/bob
  - shared/common
```

```yaml
# ~/.config/sync0/users/charlie.nt (nëmmen d'Liwwerungserlaabnes schreiwen, liesen ass verbueden)
id: 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
perm: w
```

** Deele Grupp Konfiguratioun Beispill (`~/.config/sync0/config.nt`):**

```yaml
users_dir: ./users

groups:
  # Net konfiguréiert Benotzer: Standard ëffentlechen Netzwierk nëmme liesen
  dataset:
    path: ./data
    rate: 50MB

  # Konfiguréiert Benotzer: limitéiert automatesch ëffentlech Netzwierk Liesen (nëmmen Alice a Bob kënne liesen, Charlie kann nëmmen liwweren an eroplueden)
  team-vault:
    path: ./vault
    users:
      - alice
      - bob
      - charlie
```

Start de Service (Luet d'Konfiguratiounsdatei oder benotzt direkt Kommandozeilparameter fir d'ëffentlech Netzwierk Liesen ze beschränken an d'Memberen ze autoriséieren):

```bash
# Lued ~/.config/sync0/config.nt fir unzefänken
sync0 share --region SHA

# Oder publizéieren eng privat Deelegrupp duerch reng Kommandozeil (spezifizéieren --allow-* fir automatesch d'ëffentlech Netzwierk Liesen ze beschränken an nëmmen autoriséiert Benotzer kënnen et zougräifen)
sync0 share team-vault:./vault \
  --allow-r 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c \
  --allow-rw 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f:uploads/bob,shared/common \
  --allow-w 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
```

### 5. Node Adress, Visittekaart a Multi-Sprooch

```bash
# Kuckt déi lokal IP-gratis Node Adress (Schlëssel gëtt automatesch fir den éischte Run generéiert)
sync0 id

# Generéiere Ed25519 selwer ënnerschriwwen Node Visittekaart (Spëtznumm, Stadcode, E-Mail, Websäit, Aféierung)
sync0 profile -u alice --region SHA --email alice@example.com -b "Shanghai node"

# Et gi 75 agebaute Sproochen, déi d'Systemsprooch als Standard verfollegen a kënnen duerch Ëmfeldvariablen geschalt ginn.
SYNC0_LANG=en sync0 --help
```

## Leeschtung a Kär Fonctiounen

### Cross-continental WAN aktuell Messung (Vergläich `rsync`)

Tatsächlech Miessëmfeld: USA `c1` → Europa `n1` (RTT 65 ms), den Dateset ass Linux Kernel `linux-7.3.3`:

#### 1) A la Carte Modus vs `rsync`

| Datenset | Gréisst | `sync0` Eenzelpunkt<br>(Mbps) | `rsync`<br>(Mbps) | Speedup (`sync0` / `rsync`) |
| :--- | ---: | ---: | ---: | ---: |
| `linux-7.3.3.tar.xz`(net kompriméierbar) | 155 MB | **236.48** | 108.74 | **2.17×** |
| `linux-7.3.3.tar`(kompressibel) | 1.6 GB | **843.83** | 385.06 | **2.19×** |
| `linux-7.3.3.tar`(lokal Ännerung Inkrement) | 1.6 GB | **8,014.15** | 1,361.72 | **5.89×** |
| `linux-7.3.3/` (95.979 kleng Dateien) | 1.5 GB | **627.58** | 347.80 | **1.80×** |
| **geometresch Mëttel** | - | **1,000.91** | 375.26 | **2.67×** |

#### 2) P2P Dual-Source Modus vs `rsync`

| Datenset | Gréisst | `sync0` P2P Dual Source<br>(Mbps) | `rsync`<br>(Mbps) | Speedup (`sync0` / `rsync`) |
| :--- | ---: | ---: | ---: | ---: |
| `linux-7.3.3.tar.xz`(net kompriméierbar) | 155 MB | **330.09** | 108.74 | **3.04×** |
| `linux-7.3.3.tar`(kompressibel) | 1.6 GB | **1,551.86** | 385.06 | **4.03×** |
| `linux-7.3.3.tar`(lokal Ännerung Inkrement) | 1.6 GB | **5,474.17** | 1,361.72 | **4.02×** |
| `linux-7.3.3/` (95.979 kleng Dateien) | 1.5 GB | **706.56** | 347.80 | **2.03×** |
| **geometresch Mëttel** | - | **1,186.42** | 375.26 | **3.16×** |

> Notiz: Am P2P Dual-Source Modus kollaboréieren europäesch Noden `s1` fir Aussaat, an de cross-kontinentale Réckgrat RTT ass 65 ms.

### Kär Produit Fonctiounen

- ** Ëffentlech Schlëssel Adress automatesch Lach Punching **: Kee Besoin fir en zentrale Server an ëffentlech Netzwierk Adress, automatesch NAT Lach Punching baséiert op der verdeelt Reseau ëffentlech Schlëssel Adress.
- **Millisecond-Niveau Echtzäit Synchroniséierung a Metadatenbewosstsinn**: Baséierend op gebierteg Datei Event Iwwerwaachung (`FSEvents` / `inotify` / `ReadDirectoryChangesW`), Echtzäit Push vun Dateiergänzungen, Läschen, POSIX Permissiounen (`mode`) a Millisekonne-Modifikatioun Zäit="5"> (`ReadDirectoryChangesW`); Null Datenblockiwwerdroung wann nëmmen Permissiounen oder Zäit geännert ginn.
- ** BT-Stil gläichzäiteg Eroflueden an Eroplueden a regional Affinitéitsplang **: Den Erofluede Node brauch net op de ganze Betrag ze waarden fir erofzelueden. All d'Blockintervaller (`Op::Have`), déi den 128-Bit Hashcheck passéiert hunn, ginn direkt an der Erënnerung / temporäre Pool gespaart fir aner Noden gläichzäiteg ze zéien. Zesumme mat dynamesche PEX Nodeaustausch a luesen Node hallef Viraussetzung (Work-Stealing), gëtt de Cluster-Duerchgang maximéiert.
- **Anti-béiswëlleg Node-Tampering a Clustervergëftungsimmunitéit**: autoritär Quelllëscht `root_hash` Selbstkonsistent Sperrung + obligatoresch Hardwarebeschleunigung `gxhash128` Verdauungsverifizéierung ier all Block op der Disk plazéiert an extern gesaat gëtt; wann e tamperéierte Block fonnt gëtt, wäert de Millisekonne-Niveau Circuit Breaker de béisaarteg Node blackout an automatesch vun der éierlecher Quell erholen.
- ** FastCDC in-situ Inkrement a kleng Dateiaggregatioun **: Nëmmen Differentialblocken gi fir deelweis Ännerung vu grousse Dateien transferéiert an op Disk gesat; massive klenge Fichier Erënnerung Verpakung a Präfix differentiell Kompressioun eliminéiert Fichier-vun-Datei Ronn-Rees Overhead.
- **Selbstheilung nom Stroumausbroch a Widderhuelung nom Neistart **: Redis-ähnlech RDB + AOF Hybridformat (`.s0_subs.bin`) gëtt benotzt fir de Seedstatus an d'Dateilëscht ze bestoe bleiwen, alldeeglech Ännerungen ginn a Mikrosekonne bäigefüügt, a ginn automatesch nei geschriwwe a regelméisseg organiséiert. Nodeems de Server nei gestart ass, ginn offline Ännerungen automatesch op de Seedknuet gedréckt.

## Technesch Architektur an Dokumentatioun

- ** Dual Plane Decoupling Protokoll**:
  - **Kontroll an Echtzäit änneren Uewerfläch (`iroh` QUIC / TCP, éischt Byte `PROTO_VERSION` Versioun Handshake) **: Verantwortlech fir Ed25519 Visittekaart Verifikatioun, Erlaabnis Verifizéierung, Swarm Seed Entdeckung, an Ënnerstëtzung fir `u64::MAX` Niveau Mass Changement Streaming Batching an `mzstd` Echtzäit Pushing an `mzstd` id="6"> / `FsOp::Remove` / `FsOp::Meta` Dräi Primitiv).
  - **Datenverdeelungsfläch (gebierteg UDP + `compio` asynchron I/O) **: 20B kompakt Header, 18B TACK periodesch Feedback, BBRv3/Copa Hybrid Stau Kontroll, RACK-TLP Paketverloscht Erhuelung, Shannon Entropie adaptiven Zstd Kompressioun Contournement vum éischte Block, an <b id ="-2"> Blockéierungsend = "-2"> Blockéierung.
- **Léier méi**:
  - [Leeschtung Mooss Rapport a Reproduktioun Schrëft](https://github.com/webc-site/sync0/blob/main/doc/zh/benchmark.md)
  - [Systemarchitektur Design](https://github.com/webc-site/sync0/blob/main/doc/zh/design_v4.md)
  - [Binär Drot Protokoll a Persistenz Spezifizéierung](https://github.com/webc-site/sync0/blob/main/doc/zh/proto.md)
  - [Identitéit Authentifikatioun, Autoritéit Kontroll an Integritéit Verifizéierung](https://github.com/webc-site/sync0/blob/main/doc/zh/crypto.md)