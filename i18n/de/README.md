# sync0

`sync0` ist ein Befehlszeilentool zum Übertragen von Dateien über Netzwerke, zum schrittweisen Synchronisieren von Verzeichnissen und zum gleichzeitigen Verteilen von Daten auf mehrere Computer.

## Ursprüngliche Designabsicht

In maschinenübergreifenden Räumen, Ländern oder Umgebungen mit Netzwerkverzögerungen und Paketverlust basieren Tools wie `rsync` und `scp` auf der Bestätigung einer einzelnen Verbindungssequenz, wodurch die Bandbreitennutzung begrenzt wird. Wenn mehrere Maschinen gleichzeitig Daten vom selben Server abrufen, kann die Netzwerkkarte des Quellservers leicht zu einem Engpass werden.

Das Ziel von `sync0` ist die Lösung zweier Probleme:

1. Lassen Sie die Datei- und Verzeichnissynchronisierung zwischen den beiden Maschinen mit voller physischer Bandbreite laufen und übertragen Sie den geänderten Teil nur, wenn die Datei teilweise geändert wurde.
2. Mehrere Maschinen können bei der Synchronisierung desselben Verzeichnisses Datenblöcke untereinander übertragen, wodurch die Belastung des Quellservers verringert und die Gesamtsynchronisierungszeit verkürzt wird.

## Geschwindigkeitsvergleich mit rsync

Testumgebung: Europäischer Knoten $\leftrightarrow$ Nordamerikanischer Knoten, Umlaufverzögerung 86 ms, Messung des gesamten Zeitaufwands, einschließlich Verzeichnisscan, Deduplizierungsvergleich, Netzwerkübertragung und Überprüfung der Festplattenplatzierung:

| Übertragungsszenario | Datengröße | `sync0` | `rsync` | mehrere |
| :--- | :---: | :---: | :---: | :---: |
| 10.000 kleine Dateien | 77.6 MB | 0.90 s (688 Mbps) | 4.86 s (128 Mbps) | 5.4x |
| Code- und Archivpakete (`.tar`) | 512 MB | 1.94 s (2111 Mbps) | 9.36 s (438 Mbps) | 4.8x |
| Videodatei (`.mp4`) | 512 MB | 7.36 s (556 Mbps) | 12.11 s (338 Mbps) | 1.6x |
| Inkrementelle Synchronisierung (1 % Inhalt ändern) | 512 MB | 0.12 s | 1.85 s | 15.4x |

## Methoden zur Leistungsverbesserung

- ** Gleichzeitige UDP-Übertragung **: Senden Sie Datenblöcke über UDP und übertragen Sie nur verlorene Fragmente erneut, um zu vermeiden, dass ein einzelner Paketverlust nachfolgende Dateiübertragungen blockiert.
- **Inhaltssegmentierung und Deduplizierung**: Teilen Sie die Datei entsprechend dem Inhalt in Datenblöcke auf und berechnen Sie den Prüfwert. Derselbe Inhalt, der sich bereits auf der Empfängerseite befindet, wird direkt lokal wiederverwendet, ohne dass das Netzwerk dupliziert wird.
- **Komprimierung automatisch an die Netzwerkgeschwindigkeit anpassen**: Aktivieren Sie die Komprimierung für komprimierbare Inhalte wie Text und Code; Überspringen Sie automatisch die Komprimierung für Videos, Bilder, komprimierte Pakete oder unter Hochgeschwindigkeits-LAN, um den CPU-Verbrauch zu reduzieren.
- **Multi-Knoten-Pulling in der Nähe**: Wenn mehrere Maschinen dasselbe Verzeichnis abrufen, werden Datenblöcke gleichzeitig von heruntergeladenen Knoten in derselben Stadt oder mit geringerer Latenz abgerufen.

## Installieren

### Methode 1: Installation über Cargo

```bash
cargo install sync0
```

### Methode 2: Vorkompilierte Binärdateien herunterladen

Gehen Sie zu [GitHub Releases](https://github.com/webc-site/sync0/releases), um das `.tar.xz` komprimierte Paket für das entsprechende System herunterzuladen (unterstützt Linux x86_64/ARM64, macOS Apple Silicon/Intel, Windows x64/ARM64, Android ARM64/x86_64, FreeBSD, NetBSD). Platzieren Sie `sync0` nach dem Entpacken im Verzeichnis `PATH`:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Benutzerhandbuch

### 1. Übertragen Sie Dateien oder Verzeichnisse direkt zwischen zwei Computern

Starten Sie die Überwachung auf dem empfangenden Computer und geben Sie den Empfangs- und Speicherpfad an:

```bash
sync0 recv 0.0.0.0:9000 ./backup
```

Senden Sie eine einzelne Datei oder ein ganzes Verzeichnis vom sendenden Computer:

```bash
sync0 send ./project 192.168.1.10:9000
```

Häufig verwendete Parameter:

```bash
# Löschen Sie redundante Dateien, die auf der Empfangsseite, aber nicht auf der Sendeseite vorhanden sind (Spiegelsynchronisation).
sync0 send ./project 192.168.1.10:9000 --delete

# Senderate begrenzen (Einheit: Bytes/Sekunde, zum Beispiel 50 MB/s)
sync0 send ./project 192.168.1.10:9000 --rate 50000000

# Stiller Modus (kein Fortschrittsbalken angezeigt)
sync0 send ./project 192.168.1.10:9000 -q
```

### 2. Freigegebene Verzeichnisse veröffentlichen und von mehreren Knoten abrufen

Veröffentlichen Sie ein freigegebenes Verzeichnis (mit dem Namen `dataset`) auf dem Ursprungsserver:

```bash
sync0 share dataset ./data --region SHA
```

Rufen Sie das freigegebene Verzeichnis auf anderen Computern ab (Sie können mehrere Knotenadressen gleichzeitig eingeben, durch Kommas getrennt, um es gleichzeitig abzurufen):

```bash
sync0 pull 10.0.0.1:9000,10.0.0.2:9000 dataset ./local_data
```

Ziehen Sie nur ein bestimmtes Unterverzeichnis im freigegebenen Verzeichnis:

```bash
sync0 pull 10.0.0.1:9000 dataset ./local_data --subpath 2026/logs
```

Lokale Änderungen zurück in das Remote-Freigabeverzeichnis übertragen:

```bash
sync0 push 10.0.0.1:9000 dataset ./local_data
```

### 3. Abonnieren Sie die automatischen Aktualisierungen des Katalogs

Abonnieren Sie das freigegebene Verzeichnis des Quellknotens. Wenn eine Datei im Quellverzeichnis hinzugefügt oder geändert wird, wird sie automatisch mit dem lokalen Computer synchronisiert. Gleichzeitig dient der lokale Computer automatisch als Knoten für andere Computer zum Abrufen von:

```bash
sync0 sub 10.0.0.1:9000 dataset ./local_mirror
```

### 4. Zwei-Wege-Synchronisierung von Gruppen mit mehreren Maschinen

Mehrere Maschinen treten demselben Gruppennamen bei und Änderungen in jedem Knotenverzeichnis werden automatisch mit anderen Knoten in der Gruppe synchronisiert:

```bash
sync0 group team-docs ./docs 10.0.0.1:9000,10.0.0.2:9000
```

### 5. Knotenidentität und persönliche Visitenkarte

Jeder Knoten unterstützt das Festlegen des Benutzernamens, der Stadt, der E-Mail-Adresse, der Website und einer personalisierten Signatur (maximal 256 Zeichen) und übergibt die lokale Signaturauthentifizierung mit privatem Schlüssel:

```bash
# Zeigen Sie die ID des lokalen öffentlichen Schlüssels an (geben Sie das Konfigurationsverzeichnis an, um den Schlüssel beizubehalten).
sync0 id -c ~/.config/sync0

# Erstellen und zeigen Sie Knoten-Visitenkarten mit personalisierten Signaturen an
sync0 profile -c ~/.config/sync0 \
  -u alice \
  --region SHA \
  --email alice@example.com \
  --url https://example.com \
  -b "Shanghai backup node"
```

`share`, `pull`, `sub`, `group` können auch mit diesen Profilparametern gestartet werden:

```bash
sync0 share dataset ./data -c ~/.config/sync0 -u alice --region SHA -b "Shanghai backup node"
```

### 6. Mehrsprachige Umschaltung

`sync0` Unterstützt 75 Sprachen. Standardmäßig werden Befehlszeilenhilfe und Eingabeaufforderungen automatisch in der entsprechenden Sprache entsprechend der Sprachumgebung des Betriebssystems angezeigt; Wenn die Systemsprache nicht in der unterstützten Liste enthalten ist, wird standardmäßig Englisch verwendet.

Um die Sprache manuell anzugeben, legen Sie die Umgebungsvariable `SYNC0_LANG` fest:

```bash
SYNC0_LANG=en sync0 --help
SYNC0_LANG=ja sync0 --help
SYNC0_LANG=de sync0 --help
```