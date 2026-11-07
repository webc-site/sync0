# sync0

`sync0` ass e Kommandozeil-Tool fir Dateien iwwer Netzwierker ze transferéieren, inkrementell Verzeechnes ze synchroniséieren, a gläichzäiteg Daten op verschidde Maschinnen ze verdeelen.

## Original Design Absicht

A Kräiz-Maschinn Zëmmeren, Länner oder Ëmfeld mat Netzverzögerungen a Paketverloscht, Tools wéi `rsync` an `scp` baséieren op enger eenzeger Verbindungssequenzbestätegung, wat d'Bandbreedungsverbrauch limitéiert; wann MÉI Maschinnen zéien Daten aus dem selwechten Server op der selwechter Zäit, der Quell Server Reseau Kaart kann einfach eng Flaschenhals ginn.

D'Zil vun `sync0` ass zwee Problemer ze léisen:

1. Loosst d'Datei an d'Verzeechnes Synchroniséierung tëscht den zwou Maschinnen voll kierperlech Bandbreed lafen, an nëmmen de geännerten Deel transferéieren wann d'Datei deelweis geännert gëtt.
2. Multiple Maschinnen kënnen Datenblocken openee transferéieren wann Dir dee selwechte Verzeechnes synchroniséiert, d'Laascht op de Quellserver reduzéieren an d'Gesamtsynchroniséierungszäit verkierzen.

## Geschwindegkeet Verglach mat rsync

Testëmfeld: Europäesch Node $\leftrightarrow$ Nordamerikaneschen Node, Ronn-Rees Verzögerung 86 ms, moosst déi komplett Zäitopwendeg abegraff Verzeechnesscannen, Deduplikatiounsvergleich, Netzwierkiwwerdroung an Diskplacementverifizéierung:

| Transmissioun Szenario | Donnéeën Gréisst | `sync0` | `rsync` | multiple |
| :--- | :---: | :---: | :---: | :---: |
| 10.000 kleng Dateien | 77.6 MB | 0.90 s (688 Mbps) | 4.86 s (128 Mbps) | 5.4x |
| Code an Archiv Packagen (`.tar`) | 512 MB | 1.94 s (2111 Mbps) | 9.36 s (438 Mbps) | 4.8x |
| Videodatei (`.mp4`) | 512 MB | 7.36 s (556 Mbps) | 12.11 s (338 Mbps) | 1.6x |
| Inkrementell Synchroniséierung (1% Inhalt änneren) | 512 MB | 0.12 s | 1.85 s | 15.4x |

## Leeschtung Verbesserung Methoden

- ** UDP gläichzäiteg Iwwerdroung **: Schéckt Datenblocken duerch UDP, a vermëttelt nëmme verluere Fragmenter fir ze vermeiden datt en eenzege Paketverloscht déi spéider Dateitransfere blockéiert.
- ** Inhaltssegmentéierung an Deduplikatioun **: Split d'Datei an Dateblocken no Inhalt a berechent de Scheckwäert. Dee selwechten Inhalt schonn um Empfang gëtt direkt lokal weiderbenotzt ouni d'Netz ze duplizéieren.
- ** Automatesch Kompressioun no Netzwierkgeschwindegkeet upassen **: Aktivéiert Kompressioun fir kompriméierbar Inhalter wéi Text a Code; sprangen automatesch Kompressioun fir Videoen, Biller, kompriméiert Packagen oder ënner High-Speed-LAN fir den CPU-Verbrauch ze reduzéieren.
- **Multi-Node Emgéigend zéien **: Wann verschidde Maschinnen dee selwechte Verzeechnes zéien, ginn Dateblocken gläichzäiteg vun erofgeluede Wirbelen an der selwechter Stad gezunn oder mat manner latency.

## Installéieren

### Method 1: Installéiert iwwer Cargo

```bash
cargo install sync0
```

### Method 2: Luet virkompiléiert Binären erof

Gitt op [GitHub Releases](https://github.com/webc-site/sync0/releases) fir de `.tar.xz` kompriméierte Package fir de entspriechende System erofzelueden (ënnerstëtzt Linux x86_64/ARM64, macOS Apple Silicon/Intel, Windows x64/ARM64, Android ARM64/x86_64, FreeBSD, NetBSD). Nom Auszippen, setzt `sync0` an de `PATH` Verzeichnis:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## User Guide

### 1. Transfert direkt Dateien oder Verzeichnisser tëscht zwou Maschinnen

Start Iwwerwaachung op der Empfangsmaschinn a spezifizéiert den Empfangs- a Spuerwee:

```bash
sync0 recv 0.0.0.0:9000 ./backup
```

Schéckt eng eenzeg Datei oder e ganze Verzeechnes vun der Sendermaschinn:

```bash
sync0 send ./project 192.168.1.10:9000
```

Allgemeng benotzt Parameteren:

```bash
# Redundante Dateien läschen déi um Empfangsend existéieren awer net um Sendend (Spigelsynchroniséierung)
sync0 send ./project 192.168.1.10:9000 --delete

# Limitéiert d'Sendrate (Eenheet: Bytes / Sekonn, zum Beispill 50 MB / s)
sync0 send ./project 192.168.1.10:9000 --rate 50000000

# Stille Modus (keng Fortschrëttbar gewisen)
sync0 send ./project 192.168.1.10:9000 -q
```

### 2. Verëffentlechen gedeelt Verzeichnisser an zitt vu multiple Wirbelen

Verëffentlechen e gemeinsame Verzeechnes (genannt `dataset`) um Origin Server:

```bash
sync0 share dataset ./data --region SHA
```

Pull de gemeinsame Verzeechnes op anere Maschinnen (Dir kënnt verschidde Node Adressen zur selwechter Zäit ausfëllen, getrennt vu Komma fir gläichzäiteg ze zéien):

```bash
sync0 pull 10.0.0.1:9000,10.0.0.2:9000 dataset ./local_data
```

Pull nëmmen e bestëmmten Ënnerverzeechnes am gemeinsame Verzeechnes:

```bash
sync0 pull 10.0.0.1:9000 dataset ./local_data --subpath 2026/logs
```

Dréckt lokal Ännerungen zréck an de Remote Shared Verzeechnes:

```bash
sync0 push 10.0.0.1:9000 dataset ./local_data
```

### 3. Abonnéiert Iech op Katalog automatesch Aktualiséierungen

Abonnéiert Iech op de gemeinsame Verzeechnes vum Quellknot. Wann eng Datei am Quellverzeichnis bäigefüügt oder geännert gëtt, gëtt se automatesch op de lokale Computer synchroniséiert. Zur selwechter Zäit déngt déi lokal Maschinn automatesch als Node fir aner Maschinnen ze zéien:

```bash
sync0 sub 10.0.0.1:9000 dataset ./local_mirror
```

### 4. Zwee-Manéier Synchroniséierung vun Multi-Maschinn Gruppen

Multiple Maschinnen bäitrieden dee selwechte Gruppnumm, an Ännerungen an all Node Verzeechnes ginn automatesch op aner Noden an der Grupp synchroniséiert:

```bash
sync0 group team-docs ./docs 10.0.0.1:9000,10.0.0.2:9000
```

### 5. Node Identitéit a perséinlech Visittekaart

All Node ënnerstëtzt d'Astellung vum Benotzernumm, Stad, E-Mail, Websäit an eng personaliséiert Ënnerschrëft (maximal 256 Zeechen), a passéiert déi lokal privat Schlëssel Ënnerschrëft Authentifikatioun:

```bash
# Kuckt déi lokal ëffentlech Schlëssel ID (spezifizéiert de Konfiguratiounsverzeechnes fir de Schlëssel ze bestoen)
sync0 id -c ~/.config/sync0

# Generéiere a weist Node Visittekaarte mat personaliséierten Ënnerschrëften
sync0 profile -c ~/.config/sync0 \
  -u alice \
  --region SHA \
  --email alice@example.com \
  --url https://example.com \
  -b "Shanghai backup node"
```

`share`, `pull`, `sub`, `group` kënnen och mat dëse Profilparameter gestart ginn:

```bash
sync0 share dataset ./data -c ~/.config/sync0 -u alice --region SHA -b "Shanghai backup node"
```

### 6. Multi-Sprooch schalt

`sync0` Ënnerstëtzt 75 Sproochen. Par défaut ginn d'Hëllef vun der Kommandozeil an d'Ufroen an der entspriechender Sprooch automatesch ugewisen no der Sproochëmfeld vum Betribssystem; wann d'Systemsprooch net an der ënnerstëtzter Lëscht ass, gëtt Englesch als Standard benotzt.

Fir d'Sprooch manuell ze spezifizéieren, setzt d'Ëmfeldvariabel `SYNC0_LANG`:

```bash
SYNC0_LANG=en sync0 --help
SYNC0_LANG=ja sync0 --help
SYNC0_LANG=de sync0 --help
```