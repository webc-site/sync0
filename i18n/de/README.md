# sync0

`sync0` ist ein dezentrales P2P-Dateifreigabe-, Echtzeit-Synchronisierungs- und Multi-Source-Verteilungstool, das `rsync` ersetzt. Die gemessene Geschwindigkeit des transkontinentalen WAN beträgt bis zu `rsync`s **5,89×** (einpunktige geometrische Durchschnittsgeschwindigkeit **2,67×**, P2P-Dual-Source-geometrische Durchschnittsgeschwindigkeit **3,16×**) und transozeanische P2P-Beschleunigung mit hoher Latenz beträgt bis zu **7,68×** (Full-Link 24-Szenario umfassende geometrische Durchschnittsgeschwindigkeit **2,56×**).

Jede Maschine ist sowohl ein Server als auch ein Client: Es ist kein zentraler Server oder eine öffentliche Netzwerkadresse erforderlich. Das automatische Hole Punching basiert auf der öffentlichen Schlüsseladresse des verteilten Netzwerks und unterstützt die direkte Übertragung zwischen zwei Maschinen, das Hosten von Berechtigungen für mehrere Freigabegruppen, die Mehrfachauswahl von Unterverzeichnissen bei Bedarf, die inkrementelle Echtzeitsynchronisierung auf Millisekundenebene und die gleichzeitige P2P-Mehrquellenbeschleunigung.

## Installieren

### Methode 1: Frachtinstallation

```bash
cargo install sync0
```

### Methode 2: Vorkompilierte Binärdateien herunterladen

Gehen Sie zu [GitHub Releases](https://github.com/webc-site/sync0/releases), um das `.tar.xz` komprimierte Paket herunterzuladen, das dem System entspricht (unterstützt Linux, macOS, Windows, Android, iOS, FreeBSD), entpacken Sie es und legen Sie es in `PATH` ab:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Legen Sie schnell los

### 1. Direkte Übertragung ohne Konfiguration zwischen zwei Maschinen (`send` / `recv`)

Sie müssen keine Freigabegruppe konfigurieren, sondern können Dateien oder Verzeichnisse direkt und mit hoher Geschwindigkeit zwischen zwei Computern übertragen:

```bash
# Der Empfänger lauscht am Port und schreibt in ./backup
sync0 recv 0.0.0.0:9000 ./backup

# Direkte Übertragung vom Absender (optional --delete Mirror zum Bereinigen überschüssiger Dateien, --rate 50 MB Geschwindigkeitsbegrenzung)
sync0 send ./project 192.168.1.10:9000
```

### 2. Beitragsfreigabe und On-Demand-Pull/Upload (`share` / `pull` / `push`)

```bash
# Server: Veröffentlichen Sie die Freigabegruppe mit einer Befehlszeile (geben Sie die IP-freie Iroh-Knotenadresse nach dem Start aus).
sync0 share dataset:./data

# Client: Ziehen Sie die gesamte Datensatzfreigabegruppe in die lokale Datei ./local_data
sync0 pull dataset:// ./local_data -p <Knotenadresse>

# Client: Bei Bedarf mehrere Auswahlen durchführen (nur bestimmte Einzeldateien und Unterverzeichnisse synchronisieren)
sync0 pull dataset://docs/readme.md,assets/icons,src/core ./local_data -p <Knotenadresse>

# Client: Dateien oder Verzeichnisse in autorisierte Freigabegruppen hochladen
sync0 push dataset://releases/v1 ./build_output -p <Knotenadresse>
```

### 3. Abonnement-Seeding in Echtzeit und bidirektionale Synchronisierung mehrerer Maschinen (`sub` / `group`)

Wenn nach dem Aufbau einer langen Verbindung auf der Gegenseite eine Datei **hinzugefügt, geändert, gelöscht, Berechtigungen oder Änderungszeit geändert wird**, wird das lokale Verzeichnis synchron in Millisekunden in Echtzeit aktualisiert (bei neuen/geänderten großen Dateien werden nur die Differenzblöcke abgerufen und an Ort und Stelle aktualisiert, beim Löschen von Dateien werden lokale Dateien und leere Verzeichnisse automatisch bereinigt und nur Berechtigungen geändert oder Null-Datenblöcke direkt mit einem Zeitstempel versehen):

- **`sync0 sub`(Einseitige Echtzeitspiegelung + automatisches Seeding)**: Die lokale Echtzeit folgt den Hinzufügungen, Löschungen und Änderungen des Sharing-Endes und dient automatisch als P2P-Seed-Quelle, um gleichzeitige Beschleunigung für andere Pull-Knoten bereitzustellen.
- **`sync0 group`(Peer-to-Peer-Zwei-Wege-Echtzeitsynchronisation mit mehreren Maschinen)**: Wenn Dateien auf einer beliebigen Maschine in der Gruppe hinzugefügt, gelöscht oder geändert werden, werden alle anderen Online-Knoten synchron in Echtzeit aktualisiert und dienen als Startpunkte füreinander.

```bash
# Abonnieren Sie die gesamte Freigabegruppe: Remote-Hinzufügungen, -Löschungen und -Änderungen werden lokal in Echtzeit synchronisiert und automatisch geseedet
sync0 sub dataset:// ./local_mirror -p <Knotenadresse>

# On-Demand-Mehrfachauswahl-Abonnement: Synchronisieren und Seeding nur bestimmter einzelner Dateien und Unterverzeichnisse in Echtzeit
sync0 sub dataset://models/llama.onnx,configs ./local_mirror -p <Knotenadresse>

# Zwei-Wege-Echtzeit-Synchronisierung mehrerer Maschinen-Peergruppen (unterstützt auch vollständige oder Komma-Mehrfachauswahlpfade)
sync0-Gruppe team-docs:// ./docs -p <Knotenadresse>
```

### 4. Team-Mehrfachfreigabegruppen- und Mitgliederberechtigungskonfiguration (`.nt`-Konfiguration)

Das Standardkonfigurationsverzeichnis ist `~/.config/sync0` (kann über `-c <directory>` umgeschaltet werden) und verwendet das minimalistische Format [NestedText](https://nestedtext.org) (`.nt`):
- **Unterstützt die Einschränkung des Lesens im öffentlichen Netzwerk (nur autorisierte Benutzer können lesen)**: Solange die Freigabegruppe mit einer `users:`-Mitgliederliste konfiguriert ist (oder die Befehlszeile `--allow-r` / `--allow-rw` / `--allow-w` / `--auth` angibt), aktiviert das System automatisch die private Authentifizierung (`auth: true`)** und verbietet die Aufzählung oder das Lesen anonymer Knoten im öffentlichen Netzwerk sowie P2P Wenn Sie Seeding-Knoten abonnieren, wird gleichzeitig auch die Whitelist geerbt, sodass nur autorisierte Benutzer lesen können. Freigabegruppen, die nicht mit `users` konfiguriert sind, sind standardmäßig öffentlich und schreibgeschützt (wenn Sie öffentlich und schreibgeschützt sein und nur für bestimmte Mitglieder zum Hochladen geöffnet sein möchten, können Sie `auth: false` explizit festlegen).
- **Fein abgestufte Lese-/Schreibberechtigungen und Zustellungsberechtigungen für Unterverzeichnisse**: Mitglieder sind standardmäßig schreibgeschützt (`r` muss nicht explizit geschrieben werden) und unterstützen `r` (schreibgeschützt), `w` (nur in die Dropbox schreiben, nicht lesbar), `rw` (Lese-/Schreibzugriff) und `write_prefix` (beschränken Sie das zum Hochladen zulässige Unterverzeichnispräfix).

**Beispiel für eine Mitgliederkonfiguration (`~/.config/sync0/users/`):**

```yaml
# ~/.config/sync0/users/alice.nt (Standard schreibgeschützt, Geschwindigkeitsbegrenzung 80 MB)
id: 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c
rate: 80MB
```

```yaml
# ~/.config/sync0/users/bob.nt (Lese- und Schreibberechtigungen, nur zum Hochladen in das angegebene Unterverzeichnis erlaubt)
id: 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f
perm: rw
write_prefix:
  - uploads/bob
  - shared/common
```

```yaml
# ~/.config/sync0/users/charlie.nt (nur Schreibzustellungsberechtigung, Lesen ist verboten)
id: 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
perm: w
```

**Beispiel für die Konfiguration einer Freigabegruppe (`~/.config/sync0/config.nt`):**

```yaml
users_dir: ./users

groups:
  # Nicht konfigurierte Benutzer: Standardmäßiges öffentliches Netzwerk schreibgeschützt
  dataset:
    path: ./data
    rate: 50MB

  # Konfigurierte Benutzer: Das Lesen im öffentlichen Netzwerk automatisch einschränken (nur Alice und Bob können lesen, Charlie kann nur liefern und hochladen)
  team-vault:
    path: ./vault
    users:
      - alice
      - bob
      - charlie
```

Starten Sie den Dienst (laden Sie die Konfigurationsdatei oder verwenden Sie direkt Befehlszeilenparameter, um das Lesen im öffentlichen Netzwerk einzuschränken und Mitglieder zu autorisieren):

```bash
# Laden Sie ~/.config/sync0/config.nt, um zu starten
sync0 share --region SHA

# Oder veröffentlichen Sie eine private Freigabegruppe über die reine Befehlszeile (geben Sie --allow-* an, um das Lesen im öffentlichen Netzwerk automatisch einzuschränken und nur autorisierte Benutzer darauf zuzugreifen).
sync0 share team-vault:./vault \
  --allow-r 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c \
  --allow-rw 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f:uploads/bob,shared/common \
  --allow-w 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
```

### 5. Knotenadresse, Visitenkarte und Mehrsprachigkeit

```bash
# Sehen Sie sich die lokale IP-freie Knotenadresse an (Schlüssel wird beim ersten Durchlauf automatisch generiert)
sync0 id

# Generieren Sie eine selbstsignierte Node-Visitenkarte von Ed25519 (Spitzname, Stadtcode, E-Mail, Website, Einführung).
sync0 profile -u alice --region SHA --email alice@example.com -b "Shanghai node"

# Es gibt 75 integrierte Sprachen, die standardmäßig der Systemsprache folgen und über Umgebungsvariablen umgeschaltet werden können.
SYNC0_LANG=en sync0 --help
```

## Leistung und Kernfunktionen

### Tatsächliche kontinentalübergreifende WAN-Messung (Vergleich `rsync`)

Tatsächliche Messumgebung: Vereinigte Staaten `c1` → Europa `n1` (RTT 65 ms), der Datensatz ist der Linux-Kernel `linux-7.3.3`:

#### 1) A-la-carte-Modus vs. `rsync`

| Datensatz | Größe | `sync0`Einzelpunkt<br>(Mbps) | `rsync`<br>(Mbps) | Beschleunigung (`sync0` / `rsync`) |
| :--- | ---: | ---: | ---: | ---: |
| `linux-7.3.3.tar.xz`(nicht komprimierbar) | 155 MB | **236.48** | 108.74 | **2,17×** |
| `linux-7.3.3.tar`(komprimierbar) | 1.6 GB | **843.83** | 385.06 | **2,19×** |
| `linux-7.3.3.tar`(lokales Änderungsinkrement) | 1.6 GB | **8,014.15** | 1,361.72 | **5,89×** |
| `linux-7.3.3/` (95.979 kleine Dateien) | 1.5 GB | **627.58** | 347.80 | **1,80×** |
| **geometrisches Mittel** | — | **1,000.91** | 375.26 | **2,67×** |

#### 2) P2P-Dual-Source-Modus vs. `rsync`

| Datensatz | Größe | `sync0`P2P Dual Source<br>(Mbps) | `rsync`<br>(Mbps) | Beschleunigung (`sync0` / `rsync`) |
| :--- | ---: | ---: | ---: | ---: |
| `linux-7.3.3.tar.xz`(nicht komprimierbar) | 155 MB | **330.09** | 108.74 | **3,04×** |
| `linux-7.3.3.tar`(komprimierbar) | 1.6 GB | **1,551.86** | 385.06 | **4,03×** |
| `linux-7.3.3.tar`(lokales Änderungsinkrement) | 1.6 GB | **5,474.17** | 1,361.72 | **4,02×** |
| `linux-7.3.3/` (95.979 kleine Dateien) | 1.5 GB | **706.56** | 347.80 | **2,03×** |
| **geometrisches Mittel** | — | **1,186.42** | 375.26 | **3,16×** |

> Hinweis: Im P2P-Dual-Source-Modus arbeiten europäische Knoten `s1` beim Seeding zusammen und die kontinentalübergreifende Backbone-RTT beträgt 65 ms.

### Kernproduktfunktionen

- **Automatische Lochung der öffentlichen Schlüsseladressierung**: Kein zentraler Server und keine öffentliche Netzwerkadresse erforderlich, automatische NAT-Lochlochung basierend auf der öffentlichen Schlüsseladresse des verteilten Netzwerks.
- **Echtzeitsynchronisierung und Metadatenerkennung auf Millisekundenebene**: Basierend auf nativer Dateiereignisüberwachung (`FSEvents` / `inotify` / `ReadDirectoryChangesW`), Echtzeit-Push von Dateihinzufügungen, -löschungen, POSIX-Berechtigungen (`mode`) und Änderungszeit auf Millisekundenebene (`mtime`); Keine Datenblockübertragung, wenn nur Berechtigungen oder Zeit geändert werden.
- **Gleichzeitiger Download und Upload im BT-Stil und regionale Affinitätsplanung**: Der Download-Knoten muss nicht warten, bis die gesamte Menge heruntergeladen ist. Alle Blockintervalle (`Op::Have`), die die 128-Bit-Hash-Prüfung bestanden haben, werden sofort im Speicher/temporären Pool gesperrt, damit andere Knoten sie gleichzeitig abrufen können. Zusammen mit dem dynamischen PEX-Knotenaustausch und der langsamen Knoten-Half-Preemption (Work-Stealing) wird der Cluster-Durchsatz maximiert.
- **Anti-Malicious-Node-Manipulation und Cluster-Poisoning-Immunität**: Autoritätsquellenliste `root_hash` Selbstkonsistente Sperrung + obligatorische Hardwarebeschleunigung `gxhash128` Digest-Überprüfung, bevor jeder Block auf der Festplatte platziert und extern gesetzt wird; Wenn ein manipulierter Block gefunden wird, schaltet der Schutzschalter auf Millisekundenebene den bösartigen Knoten aus und stellt ihn automatisch von der ehrlichen Quelle wieder her.
- **FastCDC In-situ-Inkrementierung und Aggregation kleiner Dateien**: Für die teilweise Änderung großer Dateien werden nur Differenzblöcke übertragen und auf der Festplatte abgelegt; Die massive Speicherpaketierung für kleine Dateien und die differenzielle Präfixkomprimierung eliminieren den Overhead für den Datei-für-Datei-Roundtrip.
- **Selbstheilung nach Stromausfall und Wiederaufnahme nach Neustart**: Das Redis-ähnliche RDB + AOF-Hybridformat (`.s0_subs.bin`) wird verwendet, um den Seeding-Status und die Dateiliste beizubehalten. Tägliche Änderungen werden in Mikrosekunden angehängt und regelmäßig automatisch neu geschrieben und organisiert. Nach dem Neustart des Servers werden Offline-Änderungen automatisch an den Seeding-Knoten übertragen.

## Technische Architektur und Dokumentation

- **Dual-Plane-Entkopplungsprotokoll**:
  - **Steuerung und Echtzeit-Änderungsoberfläche (`iroh` QUIC/TCP, erstes Byte `PROTO_VERSION` Versions-Handshake) **: Verantwortlich für Ed25519-Visitenkartenüberprüfung, Berechtigungsüberprüfung, Swarm-Seed-Erkennung und Unterstützung für Massenänderungs-Streaming-Batching auf `u64::MAX`-Ebene und `mzstd` komprimierten `SubPush` Echtzeit-Push (`FsOp::Upsert` / `FsOp::Remove` / `FsOp::Meta` Drei Grundelemente).
  - **Datenverteilungsoberfläche (natives UDP + `compio` asynchrone E/A) **: 20B kompakter Header, 18B TACK periodisches Feedback, BBRv3/Copa-Hybrid-Überlastungskontrolle, RACK-TLP-Paketverlustwiederherstellung, Shannon-Entropie-adaptive Zstd-Komprimierungsumgehung im ersten Block und `gxhash128`-Block-End-to-End-Verifizierung.
- **Weitere Informationen**:
  - [Leistungsmessbericht und Reproduktionsskript](https://github.com/webc-site/sync0/blob/main/doc/zh/benchmark.md)
  - [Design der Systemarchitektur](https://github.com/webc-site/sync0/blob/main/doc/zh/design_v4.md)
  - [Spezifikation des Binärdrahtprotokolls und der Persistenz](https://github.com/webc-site/sync0/blob/main/doc/zh/proto.md)
  - [Identitätsauthentifizierung, Autoritätskontrolle und Integritätsüberprüfung](https://github.com/webc-site/sync0/blob/main/doc/zh/crypto.md)