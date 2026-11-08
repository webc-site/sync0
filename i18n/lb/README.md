# sync0

`sync0` ass e Peer-to-Peer (P2P) Datei Sharing a Multi-Maschinn Synchroniséierungsinstrument dat mat Resilio Sync konkurréiert.

All Maschinn ass souwuel e Server wéi och e Client: all Node kann verschidde Dateiedeelungsgruppen publizéieren. Nodeems aner Noden sech bäitrieden, kënne se automatesch verfügbare Saumnoden vum Peer-Server kréien an se gläichzäiteg aus multiple Quellen zéien. Nodeems d'Synchroniséierung fäerdeg ass, ginn Somen automatesch erstallt fir méi Noden fir d'Downloads ze beschleunegen.

## Kär Highlights

- ** Jiddereen ass souwuel e Server wéi och e Client **: Vergläichbar mat Resilio Sync, keng zentral Server Ofhängegkeet. All Node kann MÉI Fichier Deele Gruppen schafen, a kann och aner Leit d'Deele Gruppen bäitrieden; Noden austauschen automatesch a kréien verfügbare Saumnoden fir gläichzäiteg Pull an Echtzäit Synchroniséierung vu verschidde Maschinnen z'erreechen.
- **IP-gratis Node-Adress**: Par défaut ginn den Node-Schlëssel an `base64url` kodéiert `iroh` Adress automatesch an `~/.config/sync0` generéiert ouni déi ëffentlech IP ze erënneren oder ze exponéieren.
- **Multi-Select Synchroniséierung op Ufro **: Ënnerstëtzt d'Synchroniséierung vun der ganzer Deelegrupp, an ënnerstëtzt och Synchroniséierung op Ufro andeems Dir verschidde Dateien oder Ënnerverzeechnungen zur selwechter Zäit auswielt.
- ** Grupp Member hierarchesch Permissiounen an eropluede Verzeechnes Kontroll **: Ënnerstëtzt nëmmen liesen (`ro`), liesen-schreiwen eropluede (`rw`), a Gestioun (`admin`) Permissiounen, a kann individuell de Wee Präfix spezifizéieren datt all Member erlaabt ass eropzelueden (par défaut, Eroplueden an de Root Verzeechnes vun der Share ass erlaabt).
- ** Vollgeschwindegkeet an inkrementell Synchroniséierung iwwer Netzwierker **: Gläichzäiteg Iwwerdroung vu grousse Dateien, Aggregatioun vu massive klengen Dateien an der Erënnerung, an nëmmen synchroniséiert Dateblocken wann Dateien deelweis geännert ginn.

## Geschwindegkeet Verglach mat rsync

### Transoceanic Netzwierk eenzeg Quell aktuell Messung (China → USA, Verzögerung 260 ms)

| Transmissioun Szenario | Donnéeën Gréisst | Modell | `sync0` | `rsync -az` | Erhéijung multiplier |
| :--- | :---: | :---: | :---: | :---: | :---: |
| 500 kleng Fichieren | 1.91 MB | Voll Synchroniséierung | 18.84 Mbps | 5.32 Mbps | 3.5x |
| 500 kleng Fichieren | 1.91 MB | Inkrementell Synchroniséierung | 27.15 Mbps | 3.83 Mbps | 7.1x |
| Code an Archiv Packagen (`.tar`) | 10.0 MB | Voll Synchroniséierung | 139.70 Mbps | 24.97 Mbps | 5.6x |
| Code an Archiv Packagen (`.tar`) | 10.0 MB | Inkrementell Synchroniséierung | 83.38 Mbps | 26.02 Mbps | 3.2x |
| Videodatei (`.mp4`) | 15.0 MB | Voll Synchroniséierung | 54.06 Mbps | 27.04 Mbps | 2.0x |
| Videodatei (`.mp4`) | 15.0 MB | Inkrementell Synchroniséierung | 216.83 Mbps | 21.51 Mbps | 10.1x |

### Transoceanic Network P2P Multi-Node Pull Test (China → USA Dual Nodes, Verzögerung 260 ms)

Gemoossene Verglach vu Multi-Source Som Node gesträifte concurrent Pull:

| Transmissioun Szenario | Donnéeën Gréisst | Single Node Séi | Zwee-Node P2P gläichzäiteg Aussaat | Erhéijung multiplier |
| :--- | :---: | :---: | :---: | :---: |
| 500 kleng Fichieren | 1.97 MB | 5.28 Mbps | 7.87 Mbps | 1.5x |
| Code an Archiv Packagen (`.tar`) | 30.0 MB | 56.35 Mbps | 83.17 Mbps | 1.5x |
| Videodatei (`.mp4`) | 30.0 MB | 27.00 Mbps | 39.46 Mbps | 1.5x |

### Tatsächlech Messung vun der transkontinentaler Héich-Vitesse engagéierten Linn (Europa → Nordamerika, Verzögerung 86 ms)

| Transmissioun Szenario | Donnéeën Gréisst | `sync0` | `rsync` | Erhéijung multiplier |
| :--- | :---: | :---: | :---: | :---: |
| 10.000 kleng Dateien | 77.6 MB | 688 Mbps | 128 Mbps | 5.4x |
| Code an Archiv Packagen (`.tar`) | 512 MB | 2111 Mbps | 438 Mbps | 4.8x |
| Videodatei (`.mp4`) | 512 MB | 556 Mbps | 338 Mbps | 1.6x |
| Inkrementell Synchroniséierung (1% Inhalt änneren) | 512 MB | 34133 Mbps | 2214 Mbps | 15.4x |

## Installéieren

### Method 1: Installéiert iwwer Cargo

```bash
cargo install sync0
```

### Method 2: Luet virkompiléiert Binären erof

Gitt op [GitHub Releases](https://github.com/webc-site/sync0/releases) fir de `.tar.xz` kompriméierte Package fir de entspriechende System erofzelueden (ënnerstëtzt Linux, macOS, Windows, Android, FreeBSD, NetBSD), unzip a setzt en an den `PATH` Verzeechnes:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Start séier

`sync0` benotzt `~/.config/sync0` als Konfiguratiounsverzeichnis par défaut (wann Dir op aner Verzeichnisser wiessele musst, kënnt Dir `-c <Verzeechnes>` derbäisetzen).

### 1. Kuckt d'lokal Node Adress an Astellung Visittekaart

```bash
# Kuckt déi lokal Node Adress (de Schlëssel gëtt automatesch an ~/.config/sync0 fir d'éischte Kéier generéiert)
sync0 id

# Generéiere eng perséinlech Node Visittekaart (ënnerstëtzt Benotzernumm, Stad, E-Mail, Websäit an Ënnerschrëft bannent 256 Wierder)
sync0 profile -u alice --region SHA --email alice@example.com --url https://example.com -b "Shanghai node"
```

### 2. Verëffentlecht verschidde Dateiedeelungsgruppen a setzt Gruppemember Permissiounen (`.nt` Konfiguratioun)

`sync0` ass am Format [NestedText](https://nestedtext.org) (`.nt`) konfiguréiert. All Benotzer huet eng separat `.nt` Konfiguratiounsdatei. Dir kënnt de Benotzerprofilverzeechnes duerch `users_dir` an `config.nt` spezifizéieren (wat automatesch all `.nt` Dateien an dësem Verzeichnis enthält). Souwuel de Benotzer wéi och de Wee Präfix erlaabt ze eropgelueden benotzen eng ongeuerdnet Lëscht (`-`). ** Wann den Eropluedswee Präfix net ausgefëllt ass, ass den Eroplueden an de Root Verzeechnes vum Share par défaut erlaabt **.

1. Erstellt separat `.nt` Konfiguratiounsdateien fir all Member am `~/.config/sync0/users/` Verzeichnis:

`~/.config/sync0/users/alice.nt` (read-nëmme Member, Taux limitéiert `80MB`, `rate` ënnerstëtzt `GB`, `MB`, `KB` Eenheeten):

```yaml
id: 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c
perm: ro
rate: 80MB
```

`~/.config/sync0/users/bob.nt` (beschränkt Eroplueden Memberen: erlaabt nëmmen eropzelueden an `uploads/bob` an `shared/common` Verzeichnisser):

```yaml
id: 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f
perm: rw
write_prefix:
  - uploads/bob
  - shared/common
```

`~/.config/sync0/users/charlie.nt` (Standard Member eropluede: Wann `write_prefix` ewechgelooss gëtt, ass den Eroplueden an de Root-Verzeichnis vum Share par défaut erlaabt):

```yaml
id: 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
perm: rw
```

2. Konfiguréiert de Benotzerverzeichnis a verschidde Dateiedeelungsgruppen an `~/.config/sync0/config.nt`:

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

Start Deele Service:

```bash
# Direkte Startup (Luet automatesch all .nt Benotzerkonfiguratiounen an ~/.config/sync0/config.nt an users_dir Verzeichnisser)
sync0 share --region SHA

# Oder publizéieren séier verschidde Deelegruppen an autoriséiert Gruppememberen direkt iwwer d'Kommandozeil
sync0 share dataset:./data team-vault:./vault \
  --auth \
  --allow-ro 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c \
  --allow-rw 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f:uploads/bob,shared/common \
  --allow-rw 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
```

### 3. Pull all Inhalt oder wielt verschidde Dateien a Verzeichnisser (`sync0 pull`)

Connectéiert mat all Peer Node fir automatesch all verfügbare Saumknäppchen an der Deelegrupp ze kréien, an zitt gläichzäiteg vu multiple Saumnoden:

```bash
# Pull de ganzen Datesaz Deelegrupp op déi lokal ./local_data
sync0 pull dataset:// ./local_data -p <Node address>

# Multi-Select Synchroniséierung: zéien gläichzäiteg déi eenzeg Datei docs/readme.md an Ënnerverzeechnes Assets/Icons, src/core
sync0 pull dataset://docs/readme.md,assets/icons,src/core ./local_data -p <Node address>
```

### 4. Eroplueden e Fichier oder Verzeechnes an eng Deelegrupp (`sync0 push`)

Membere mat Eropluedsrechter (`rw` oder `admin`) kënne lokal Dateien oder Verzeichnisser op Ferndeelungsgruppen eroplueden:

```bash
# Membere mat Root Verzeechnes Erlaabnis Erlaabnis: Eroplueden direkt an de gemeinsame Root Verzeechnes
sync0 dréckt Team-Vault:// ./new_release -p <Node Adress>

# Nëmmen Memberen déi zu engem spezifesche Präfix autoriséiert sinn (wéi Eroplueden / Bob): eropluede fir de erlaabte Wee Präfix
sync0 push team-vault://uploads/bob/report.pdf ./report.pdf -p <Node address>
```

### 5. Abonnéiert Iech op automatesch Synchroniséierung a Seeding (`sync0 sub`)

Nodeems Dir Iech op d'Sharing-Grupp vun engem bestëmmte Node abonnéiert hutt, ass de lokale Server souwuel e Empfangsclient an en neie Seed-Server; Wann d'Quellnodedatei aktualiséiert gëtt, gëtt se automatesch synchroniséiert a stellt chunked Downloads fir aner Wirbelen:

```bash
# Abonnéiert Iech op de ganzen Deelegrupp a seed automatesch
sync0 sub dataset:// ./local_mirror -p <Node address>

# Nëmme Multi-Select fir op spezifizéierte Dateien an Ënnerverzeechnungen ze abonnéieren an se ze säen
sync0 sub dataset://models/llama.onnx,configs ./local_mirror -p <Node address>
```

### 6. Multi-Maschinn Grupp Peer Synchroniséierung (`sync0 group`)

Multiple Maschinnen bäitrieden déiselwecht Deelegrupp, géigesäiteg entdeckt automatesch verfügbare Seednoden an ënnerhält zwee-Wee Synchroniséierung (ënnerstëtzt voll Synchroniséierung oder multiple Selektioun vu Dateien an Verzeichnisser):

```bash
# Ganz an der Grupp an zwee-Manéier Synchroniséierung
sync0 group team-docs:// ./docs -p <Node address>

# Nëmmen de wal Verzeechnes an meta.json Datei an der Multi-Select Synchroniséierungsgrupp
sync0 group team-docs://wal,meta.json ./docs -p <Node address>
```

### 7. Punkt-zu-Punkt direkt Iwwerdroung tëscht zwou Maschinnen (`send` / `recv`)

Et ass net néideg eng Grupp virzestellen, an Dir kënnt temporär Dateien oder Verzeichnisser direkt tëscht zwou Maschinnen transferéieren:

```bash
# Empfänger Iwwerwachung
sync0 recv 0.0.0.0:9000 ./backup

# D'Sendend Enn transferéiert direkt Dateien oder Verzeichnisser (optional --delete Spigel fir iwwerschësseg Dateien ze botzen, --rate Geschwindegkeetslimit)
sync0 send ./project 192.168.1.10:9000
```

### 8. Multi-Sprooch schalt

`sync0` ënnerstëtzt 75 Sproochen, a schalt automatesch no dem Systemsproochëmfeld par défaut; et kann och manuell duerch `SYNC0_LANG` spezifizéiert ginn:

```bash
SYNC0_LANG=en sync0 --help
SYNC0_LANG=ja sync0 --help
SYNC0_LANG=de sync0 --help
```