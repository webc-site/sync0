# sync0

`sync0` ist ein Peer-to-Peer (P2P)-Dateifreigabe- und Synchronisierungstool für mehrere Maschinen, das mit Resilio Sync konkurriert.

Jeder Computer ist sowohl ein Server als auch ein Client: Jeder Knoten kann mehrere Dateifreigabegruppen veröffentlichen. Nachdem andere Knoten beigetreten sind, können sie automatisch verfügbare Seed-Knoten vom Peer-Server abrufen und diese gleichzeitig aus mehreren Quellen abrufen. Nach Abschluss der Synchronisierung werden automatisch Seeds für weitere Knoten erstellt, um Downloads zu beschleunigen.

## Kern-Highlights

- **Jeder ist sowohl ein Server als auch ein Client**: Vergleichbar mit Resilio Sync, keine zentrale Serverabhängigkeit. Jeder Knoten kann mehrere Dateifreigabegruppen erstellen und auch den Freigabegruppen anderer Personen beitreten. Knoten tauschen automatisch verfügbare Seed-Knoten aus und erhalten sie, um gleichzeitiges Ziehen und Echtzeitsynchronisierung mehrerer Maschinen zu erreichen.
- **IP-freie Knotenadresse**: Standardmäßig werden der Knotenschlüssel und die `base64url` codierte `iroh`-Adresse automatisch in `~/.config/sync0` generiert, ohne dass die öffentliche IP gespeichert oder offengelegt werden muss.
- **Mehrfachauswahl-Synchronisierung bei Bedarf**: Unterstützt die Synchronisierung der gesamten Freigabegruppe sowie die Synchronisierung bei Bedarf durch gleichzeitige Auswahl mehrerer Dateien oder Unterverzeichnisse.
- **Hierarchische Berechtigungen der Gruppenmitglieder und Steuerung des Upload-Verzeichnisses**: Unterstützt schreibgeschützte (`ro`), Lese-/Schreib-Upload- (`rw`) und Verwaltungsberechtigungen (`admin`) und kann das Pfadpräfix, das jedes Mitglied hochladen darf, individuell angeben (standardmäßig ist das Hochladen in das Stammverzeichnis der Freigabe zulässig).
- **Vollständige und inkrementelle Synchronisierung über Netzwerke hinweg**: Gleichzeitige Übertragung großer Dateien, Aggregation großer kleiner Dateien im Speicher und nur synchronisierte Datenblöcke, wenn Dateien teilweise geändert werden.

## Geschwindigkeitsvergleich mit rsync

### Tatsächliche Einzelquellenmessung im transozeanischen Netzwerk (China → USA, Verzögerung 260 ms)

| Übertragungsszenario | Datengröße | Modell | `sync0` | `rsync -az` | Multiplikator erhöhen |
| :--- | :---: | :---: | :---: | :---: | :---: |
| 500 kleine Dateien | 1.91 MB | Vollständige Synchronisierung | 18.84 Mbps | 5.32 Mbps | 3.5x |
| 500 kleine Dateien | 1.91 MB | Inkrementelle Synchronisierung | 27.15 Mbps | 3.83 Mbps | 7.1x |
| Code- und Archivpakete (`.tar`) | 10.0 MB | Vollständige Synchronisierung | 139.70 Mbps | 24.97 Mbps | 5.6x |
| Code- und Archivpakete (`.tar`) | 10.0 MB | Inkrementelle Synchronisierung | 83.38 Mbps | 26.02 Mbps | 3.2x |
| Videodatei (`.mp4`) | 15.0 MB | Vollständige Synchronisierung | 54.06 Mbps | 27.04 Mbps | 2.0x |
| Videodatei (`.mp4`) | 15.0 MB | Inkrementelle Synchronisierung | 216.83 Mbps | 21.51 Mbps | 10.1x |

### Transozeanischer Netzwerk-P2P-Pull-Test mit mehreren Knoten (China → US-Doppelknoten, Verzögerung 260 ms)

Gemessener Vergleich des gleichzeitigen Striping-Pulls mit Multi-Source-Seed-Knoten:

| Übertragungsszenario | Datengröße | Einzelknoten-Seeding | Gleichzeitiges P2P-Seeding mit zwei Knoten | Multiplikator erhöhen |
| :--- | :---: | :---: | :---: | :---: |
| 500 kleine Dateien | 1.97 MB | 5.28 Mbps | 7.87 Mbps | 1.5x |
| Code- und Archivpakete (`.tar`) | 30.0 MB | 56.35 Mbps | 83.17 Mbps | 1.5x |
| Videodatei (`.mp4`) | 30.0 MB | 27.00 Mbps | 39.46 Mbps | 1.5x |

### Tatsächliche Messung der transkontinentalen Hochgeschwindigkeits-Standleitung (Europa → Nordamerika, Verzögerung 86 ms)

| Übertragungsszenario | Datengröße | `sync0` | `rsync` | Multiplikator erhöhen |
| :--- | :---: | :---: | :---: | :---: |
| 10.000 kleine Dateien | 77.6 MB | 688 Mbps | 128 Mbps | 5.4x |
| Code- und Archivpakete (`.tar`) | 512 MB | 2111 Mbps | 438 Mbps | 4.8x |
| Videodatei (`.mp4`) | 512 MB | 556 Mbps | 338 Mbps | 1.6x |
| Inkrementelle Synchronisierung (1 % Inhalt ändern) | 512 MB | 34133 Mbps | 2214 Mbps | 15.4x |

## Installieren

### Methode 1: Installation über Cargo

```bash
cargo install sync0
```

### Methode 2: Vorkompilierte Binärdateien herunterladen

Gehen Sie zu [GitHub Releases](https://github.com/webc-site/sync0/releases), um das `.tar.xz` komprimierte Paket für das entsprechende System herunterzuladen (unterstützt Linux, macOS, Windows, Android, FreeBSD, NetBSD), entpacken Sie es und legen Sie es im Verzeichnis `PATH` ab:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Legen Sie schnell los

`sync0` verwendet standardmäßig `~/.config/sync0` als Konfigurationsverzeichnis (wenn Sie zu anderen Verzeichnissen wechseln müssen, können Sie `-c <directory>` hinzufügen).

### 1. Sehen Sie sich die lokale Knotenadresse und die Visitenkarte an

```bash
# Zeigen Sie die lokale Knotenadresse an (der Schlüssel wird zum ersten Mal automatisch in ~/.config/sync0 generiert).
sync0 id

# Generieren Sie eine persönliche Knoten-Visitenkarte (unterstützt Benutzername, Stadt, E-Mail, Website und Signatur innerhalb von 256 Wörtern)
sync0 profile -u alice --region SHA --email alice@example.com --url https://example.com -b "Shanghai node"
```

### 2. Veröffentlichen Sie mehrere Dateifreigabegruppen und legen Sie Gruppenmitgliederberechtigungen fest (`.nt` Konfiguration)

`sync0` ist im Format [NestedText](https://nestedtext.org) (`.nt`) konfiguriert. Jeder Benutzer verfügt über eine separate `.nt`-Konfigurationsdatei. Sie können das Benutzerprofilverzeichnis über `users_dir` in `config.nt` angeben (wodurch automatisch alle `.nt`-Dateien in diesem Verzeichnis eingeschlossen werden). Sowohl der Benutzer als auch das Pfadpräfix, das hochgeladen werden darf, verwenden eine ungeordnete Liste (`-`). **Wenn das Präfix des Upload-Pfads nicht ausgefüllt ist, ist das Hochladen in das Stammverzeichnis der Freigabe standardmäßig zulässig**.

1. Erstellen Sie separate `.nt`-Konfigurationsdateien für jedes Mitglied im Verzeichnis `~/.config/sync0/users/`:

`~/.config/sync0/users/alice.nt` (schreibgeschütztes Mitglied, Tarif begrenzt `80MB`, `rate` unterstützt `GB`, `MB`, `KB` Einheiten):

```yaml
id: 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c
perm: ro
rate: 80MB
```

`~/.config/sync0/users/bob.nt` (Eingeschränkte Upload-Mitglieder: Hochladen nur in die Verzeichnisse `uploads/bob` und `shared/common` erlaubt):

```yaml
id: 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f
perm: rw
write_prefix:
  - uploads/bob
  - shared/common
```

`~/.config/sync0/users/charlie.nt` (Standard-Upload für Mitglieder: Wenn `write_prefix` weggelassen wird, ist das Hochladen in das Stammverzeichnis der Freigabe standardmäßig zulässig):

```yaml
id: 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
perm: rw
```

2. Konfigurieren Sie das Benutzerverzeichnis und mehrere Dateifreigabegruppen in `~/.config/sync0/config.nt`:

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

Freigabedienst starten:

```bash
# Direkter Start (lädt automatisch alle .nt-Benutzerkonfigurationen in den Verzeichnissen ~/.config/sync0/config.nt und users_dir)
sync0 share --region SHA

# Oder veröffentlichen Sie schnell mehrere Freigabegruppen und autorisieren Sie Gruppenmitglieder direkt über die Befehlszeile
sync0 share dataset:./data team-vault:./vault \
  --auth \
  --allow-ro 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c \
  --allow-rw 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f:uploads/bob,shared/common \
  --allow-rw 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
```

### 3. Ziehen Sie den gesamten Inhalt ab oder wählen Sie mehrere Dateien und Verzeichnisse aus (`sync0 pull`)

Stellen Sie eine Verbindung zu einem beliebigen Peer-Knoten her, um automatisch alle verfügbaren Seed-Knoten in der Freigabegruppe abzurufen und von mehreren Seed-Knoten gleichzeitig abzurufen:

```bash
# Ziehen Sie die gesamte Datensatzfreigabegruppe in die lokale Datei ./local_data
sync0 pull dataset:// ./local_data -p <Knotenadresse>

# Mehrfachauswahl-Synchronisierung: Ziehen Sie gleichzeitig die einzelne Datei docs/readme.md und die Unterverzeichnisse asset/icons, src/core
sync0 pull dataset://docs/readme.md,assets/icons,src/core ./local_data -p <Knotenadresse>
```

### 4. Laden Sie eine Datei oder ein Verzeichnis in eine Freigabegruppe hoch (`sync0 push`)

Mitglieder mit Upload-Berechtigungen (`rw` oder `admin`) können lokale Dateien oder Verzeichnisse in Remote-Freigabegruppen hochladen:

```bash
# Mitglieder mit der Berechtigung zum Hochladen des Stammverzeichnisses: Direkt in das freigegebene Stammverzeichnis hochladen
sync0 push team-vault:// ./new_release -p <Knotenadresse>

# Nur Mitglieder, die für ein bestimmtes Präfix berechtigt sind (z. B. uploads/bob): Hochladen auf das zulässige Pfadpräfix
sync0 push team-vault://uploads/bob/report.pdf ./report.pdf -p <Knotenadresse>
```

### 5. Abonnieren Sie die automatische Synchronisierung und das Seeding (`sync0 sub`)

Nach dem Abonnieren der Freigabegruppe eines bestimmten Knotens ist der lokale Server sowohl ein empfangender Client als auch ein neuer Seeding-Server. Wenn die Quellknotendatei aktualisiert wird, wird sie automatisch synchronisiert und stellt geblockte Downloads für andere Knoten bereit:

```bash
# Abonnieren Sie die gesamte Sharing-Gruppe und starten Sie automatisch das Seeding
sync0 sub dataset:// ./local_mirror -p <Knotenadresse>

# Wählen Sie nur die Mehrfachauswahl aus, um bestimmte Dateien und Unterverzeichnisse zu abonnieren und ihnen ein Seeding zuzuordnen
sync0 sub dataset://models/llama.onnx,configs ./local_mirror -p <Knotenadresse>
```

### 6. Peer-Synchronisierung mehrerer Maschinengruppen (`sync0 group`)

Mehrere Maschinen treten derselben Freigabegruppe bei, jeder erkennt automatisch verfügbare Seed-Knoten und sorgt für eine bidirektionale Synchronisierung (unterstützt vollständige Synchronisierung oder Mehrfachauswahl von Dateien und Verzeichnissen):

```bash
# Vollständiger Beitritt zur Gruppe und bidirektionale Synchronisierung
sync0-Gruppe team-docs:// ./docs -p <Knotenadresse>

# Nur das Wal-Verzeichnis und die Datei meta.json in der Mehrfachauswahl-Synchronisierungsgruppe
sync0-Gruppe team-docs://wal,meta.json ./docs -p <Knotenadresse>
```

### 7. Punkt-zu-Punkt-Direktübertragung zwischen zwei Maschinen (`send` / `recv`)

Es ist nicht erforderlich, eine Gruppe vorab zu konfigurieren, und Sie können Dateien oder Verzeichnisse vorübergehend direkt zwischen zwei Computern übertragen:

```bash
# Empfängerüberwachung
sync0 recv 0.0.0.0:9000 ./backup

# Die sendende Seite überträgt Dateien oder Verzeichnisse direkt (optional --delete Mirror zum Bereinigen überschüssiger Dateien, --rate Geschwindigkeitsbegrenzung)
sync0 send ./project 192.168.1.10:9000
```

### 8. Mehrsprachige Umschaltung

`sync0` unterstützt 75 Sprachen und wechselt standardmäßig automatisch entsprechend der Systemsprachenumgebung; es kann auch manuell über `SYNC0_LANG` angegeben werden:

```bash
SYNC0_LANG=en sync0 --help
SYNC0_LANG=ja sync0 --help
SYNC0_LANG=de sync0 --help
```