# sync0

`sync0` is een opdrachtregelprogramma voor het overbrengen van bestanden over netwerken, het stapsgewijs synchroniseren van mappen en het gelijktijdig distribueren van gegevens naar meerdere machines.

## Oorspronkelijke ontwerpintentie

In machinekamers, landen of omgevingen met netwerkvertragingen en pakketverlies zijn tools zoals `rsync` en `scp` gebaseerd op bevestiging van een enkele verbindingsreeks, wat het bandbreedtegebruik beperkt; wanneer meerdere machines tegelijkertijd gegevens van dezelfde server ophalen, kan de netwerkkaart van de bronserver gemakkelijk een knelpunt worden.

Het doel van `sync0` is om twee problemen op te lossen:

1. Laat de bestands- en mapsynchronisatie tussen de twee machines de volledige fysieke bandbreedte gebruiken, en breng het gewijzigde deel alleen over als het bestand gedeeltelijk is gewijzigd.
2. Meerdere machines kunnen datablokken naar elkaar overbrengen bij het synchroniseren van dezelfde directory, waardoor de belasting op de bronserver wordt verminderd en de algehele synchronisatietijd wordt verkort.

## Snelheidsvergelijking met rsync

Testomgeving: Europees knooppunt $\leftrightarrow$ Noord-Amerikaans knooppunt, retourvertraging 86 ms, meten van de volledige tijdrovend inclusief directoryscannen, deduplicatievergelijking, netwerktransmissie en verificatie van schijfplaatsing:

| Transmissiescenario | Gegevensgrootte | `sync0` | `rsync` | meerdere |
| :--- | :---: | :---: | :---: | :---: |
| 10.000 kleine bestanden | 77.6 MB | 0.90 s (688 Mbps) | 4.86 s (128 Mbps) | 5.4x |
| Codeer- en archiefpakketten (`.tar`) | 512 MB | 1.94 s (2111 Mbps) | 9.36 s (438 Mbps) | 4.8x |
| Videobestand (`.mp4`) | 512 MB | 7.36 s (556 Mbps) | 12.11 s (338 Mbps) | 1.6x |
| Incrementele synchronisatie (wijzig 1% inhoud) | 512 MB | 0.12 s | 1.85 s | 15.4x |

## Methoden voor prestatieverbetering

- **UDP gelijktijdige verzending**: Verzend datablokken via UDP en verzend alleen verloren fragmenten opnieuw om te voorkomen dat een enkel pakketverlies de volgende bestandsoverdrachten blokkeert.
- **Contentsegmentatie en deduplicatie**: Splits het bestand op in datablokken op basis van de inhoud en bereken de controlewaarde. Dezelfde inhoud die zich al aan de ontvangende kant bevindt, wordt direct lokaal hergebruikt zonder het netwerk te dupliceren.
- **Pas de compressie automatisch aan op basis van de netwerksnelheid**: schakel compressie in voor comprimeerbare inhoud zoals tekst en code; sla automatisch de compressie over voor video's, afbeeldingen, gecomprimeerde pakketten of onder high-speed LAN om het CPU-verbruik te verminderen.
- **Ophalen van meerdere knooppunten in de buurt**: wanneer meerdere machines dezelfde map ophalen, worden gegevensblokken gelijktijdig opgehaald van gedownloade knooppunten in dezelfde stad of met een lagere latentie.

## Installeren

### Methode 1: Installeren via Cargo

```bash
cargo install sync0
```

### Methode 2: Voorgecompileerde binaire bestanden downloaden

Ga naar [GitHub Releases](https://github.com/webc-site/sync0/releases) om het `.tar.xz` gecomprimeerde pakket voor het overeenkomstige systeem te downloaden (ondersteunt Linux x86_64/ARM64, macOS Apple Silicon/Intel, Windows x64/ARM64, Android ARM64/x86_64, FreeBSD, NetBSD). Na het uitpakken plaats je `sync0` in de map `PATH`:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Gebruikershandleiding

### 1. Breng bestanden of mappen rechtstreeks over tussen twee machines

Begin met monitoren op de ontvangende machine en specificeer het ontvangst- en opslagpad:

```bash
sync0 recv 0.0.0.0:9000 ./backup
```

Verzend een enkel bestand of een hele map vanaf de verzendende machine:

```bash
sync0 send ./project 192.168.1.10:9000
```

Veelgebruikte parameters:

```bash
# Verwijder overtollige bestanden die aan de ontvangende kant bestaan, maar niet aan de verzendende kant (spiegelsynchronisatie)
sync0 send ./project 192.168.1.10:9000 --delete

# Verzendsnelheid beperken (eenheid: bytes/seconde, bijvoorbeeld 50 MB/s)
sync0 send ./project 192.168.1.10:9000 --rate 50000000

# Stille modus (geen voortgangsbalk weergegeven)
sync0 send ./project 192.168.1.10:9000 -q
```

### 2. Gedeelde mappen publiceren en ophalen uit meerdere knooppunten

Publiceer een gedeelde map (genaamd `dataset`) op de oorspronkelijke server:

```bash
sync0 share dataset ./data --region SHA
```

Haal de gedeelde map op andere machines op (u kunt meerdere knooppuntadressen tegelijk invullen, gescheiden door komma's om gelijktijdig op te halen):

```bash
sync0 pull 10.0.0.1:9000,10.0.0.2:9000 dataset ./local_data
```

Trek alleen een bepaalde submap in de gedeelde map:

```bash
sync0 pull 10.0.0.1:9000 dataset ./local_data --subpath 2026/logs
```

Push lokale wijzigingen terug naar de externe gedeelde map:

```bash
sync0 push 10.0.0.1:9000 dataset ./local_data
```

### 3. Abonneer u op automatische updates van de catalogus

Abonneer u op de gedeelde map van het bronknooppunt. Wanneer een bestand wordt toegevoegd of gewijzigd in de bronmap, wordt het automatisch gesynchroniseerd met de lokale computer. Tegelijkertijd zal de lokale machine automatisch dienen als knooppunt voor andere machines:

```bash
sync0 sub 10.0.0.1:9000 dataset ./local_mirror
```

### 4. Tweerichtingssynchronisatie van groepen met meerdere machines

Meerdere machines sluiten zich aan bij dezelfde groepsnaam en wijzigingen in elke knooppuntdirectory worden automatisch gesynchroniseerd met andere knooppunten in de groep:

```bash
sync0 group team-docs ./docs 10.0.0.1:9000,10.0.0.2:9000
```

### 5. Knooppuntidentiteit en persoonlijk visitekaartje

Elk knooppunt ondersteunt het instellen van de gebruikersnaam, stad, e-mail, website en een persoonlijke handtekening (maximaal 256 tekens), en passeert de lokale authenticatie van de privésleutel:

```bash
# Bekijk de lokale openbare sleutel-ID (geef de configuratiemap op om de sleutel te behouden)
sync0 id -c ~/.config/sync0

# Genereer en toon knooppunt-visitekaartjes met gepersonaliseerde handtekeningen
sync0 profile -c ~/.config/sync0 \
  -u alice \
  --region SHA \
  --email alice@example.com \
  --url https://example.com \
  -b "Shanghai backup node"
```

`share`, `pull`, `sub`, `group` kunnen ook worden gestart met deze profielparameters:

```bash
sync0 share dataset ./data -c ~/.config/sync0 -u alice --region SHA -b "Shanghai backup node"
```

### 6. Meertalig schakelen

`sync0` Ondersteunt 75 talen. Standaard worden opdrachtregelhulp en aanwijzingen in de overeenkomstige taal automatisch weergegeven, afhankelijk van de taalomgeving van het besturingssysteem; als de systeemtaal niet in de ondersteunde lijst staat, wordt standaard Engels gebruikt.

Om de taal handmatig op te geven, stelt u de omgevingsvariabele `SYNC0_LANG` in:

```bash
SYNC0_LANG=en sync0 --help
SYNC0_LANG=ja sync0 --help
SYNC0_LANG=de sync0 --help
```