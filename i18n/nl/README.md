# sync0

`sync0` is een peer-to-peer (P2P) tool voor het delen van bestanden en synchronisatie tussen meerdere machines die concurreert met Resilio Sync.

Elke machine is zowel een server als een client: elk knooppunt kan meerdere groepen voor het delen van bestanden publiceren. Nadat andere knooppunten zich hebben aangesloten, kunnen ze automatisch beschikbare zaadknooppunten van de peerserver verkrijgen en deze gelijktijdig uit meerdere bronnen halen. Nadat de synchronisatie is voltooid, worden automatisch zaden aangemaakt voor meer knooppunten om downloads te versnellen.

## Kern hoogtepunten

- **Iedereen is zowel een server als een client**: Vergelijkbaar met Resilio Sync, geen centrale serverafhankelijkheid. Elk knooppunt kan meerdere groepen voor het delen van bestanden maken, en kan zich ook aansluiten bij de deelgroepen van andere mensen; knooppunten wisselen automatisch beschikbare zaadknooppunten uit en verkrijgen deze om gelijktijdige pull- en realtime synchronisatie van meerdere machines te realiseren.
- **IP-vrij knooppuntadres**: standaard worden de knooppuntsleutel en het `base64url` gecodeerde `iroh` adres automatisch gegenereerd in `~/.config/sync0` zonder dat u het openbare IP-adres hoeft te onthouden of openbaar te maken.
- **Multi-select synchronisatie op aanvraag**: Ondersteunt synchronisatie van de gehele deelgroep, en ondersteunt ook synchronisatie op aanvraag door meerdere bestanden of submappen tegelijkertijd te selecteren.
- **Hiërarchische machtigingen voor groepsleden en beheer van uploaddirectory**: Ondersteunt alleen-lezen (`ro`), lees-schrijf-upload (`rw`) en beheermachtigingen (`admin`), en kan individueel het padvoorvoegsel specificeren dat elk lid mag uploaden (standaard is uploaden naar de hoofdmap van de share toegestaan).
- **Volledige en incrementele synchronisatie tussen netwerken**: Gelijktijdige overdracht van grote bestanden, aggregatie van enorme kleine bestanden in het geheugen en alleen gesynchroniseerde datablokken wanneer bestanden gedeeltelijk zijn gewijzigd.

## Snelheidsvergelijking met rsync

### Transoceanisch netwerk, feitelijke meting uit één bron (China → Verenigde Staten, vertraging 260 ms)

| Transmissiescenario | Gegevensgrootte | model | `sync0` | `rsync -az` | Verhoog de vermenigvuldiger |
| :--- | :---: | :---: | :---: | :---: | :---: |
| 500 kleine bestanden | 1.91 MB | Volledige synchronisatie | 18.84 Mbps | 5.32 Mbps | 3.5x |
| 500 kleine bestanden | 1.91 MB | Incrementele synchronisatie | 27.15 Mbps | 3.83 Mbps | 7.1x |
| Codeer- en archiefpakketten (`.tar`) | 10.0 MB | Volledige synchronisatie | 139.70 Mbps | 24.97 Mbps | 5.6x |
| Codeer- en archiefpakketten (`.tar`) | 10.0 MB | Incrementele synchronisatie | 83.38 Mbps | 26.02 Mbps | 3.2x |
| Videobestand (`.mp4`) | 15.0 MB | Volledige synchronisatie | 54.06 Mbps | 27.04 Mbps | 2.0x |
| Videobestand (`.mp4`) | 15.0 MB | Incrementele synchronisatie | 216.83 Mbps | 21.51 Mbps | 10.1x |

### Transoceanisch netwerk P2P multi-node pull-test (China → Verenigde Staten dubbele knooppunten, vertraging 260 ms)

Gemeten vergelijking van gestreepte gelijktijdige pull met meerdere bronnen:

| Transmissiescenario | Gegevensgrootte | Zaden met één knooppunt | P2P-gelijktijdig zaaien met twee knooppunten | Verhoog de vermenigvuldiger |
| :--- | :---: | :---: | :---: | :---: |
| 500 kleine bestanden | 1.97 MB | 5.28 Mbps | 7.87 Mbps | 1.5x |
| Codeer- en archiefpakketten (`.tar`) | 30.0 MB | 56.35 Mbps | 83.17 Mbps | 1.5x |
| Videobestand (`.mp4`) | 30.0 MB | 27.00 Mbps | 39.46 Mbps | 1.5x |

### Werkelijke meting van een transcontinentale speciale hogesnelheidslijn (Europa → Noord-Amerika, vertraging 86 ms)

| Transmissiescenario | Gegevensgrootte | `sync0` | `rsync` | Verhoog de vermenigvuldiger |
| :--- | :---: | :---: | :---: | :---: |
| 10.000 kleine bestanden | 77.6 MB | 688 Mbps | 128 Mbps | 5.4x |
| Codeer- en archiefpakketten (`.tar`) | 512 MB | 2111 Mbps | 438 Mbps | 4.8x |
| Videobestand (`.mp4`) | 512 MB | 556 Mbps | 338 Mbps | 1.6x |
| Incrementele synchronisatie (wijzig 1% inhoud) | 512 MB | 34133 Mbps | 2214 Mbps | 15.4x |

## Installeren

### Methode 1: Installeren via Cargo

```bash
cargo install sync0
```

### Methode 2: Voorgecompileerde binaire bestanden downloaden

Ga naar [GitHub Releases](https://github.com/webc-site/sync0/releases) om het `.tar.xz` gecomprimeerde pakket voor het overeenkomstige systeem te downloaden (ondersteunt Linux, macOS, Windows, Android, FreeBSD, NetBSD), pak het uit en plaats het in de `PATH` map:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Ga snel aan de slag

`sync0` gebruikt standaard `~/.config/sync0` als de configuratiemap (als u naar andere mappen moet overschakelen, kunt u `-c <map>` toevoegen).

### 1. Bekijk het lokale knooppuntadres en het instellingsvisitekaartje

```bash
# Bekijk het lokale knooppuntadres (de sleutel wordt voor de eerste keer automatisch gegenereerd in ~/.config/sync0)
sync0 id

# Genereer een persoonlijk knooppunt-visitekaartje (ondersteunt gebruikersnaam, stad, e-mailadres, website en handtekening binnen 256 woorden)
sync0 profile -u alice --region SHA --email alice@example.com --url https://example.com -b "Shanghai node"
```

### 2. Publiceer meerdere groepen voor het delen van bestanden en stel machtigingen voor groepsleden in (`.nt` Configuratie)

`sync0` is geconfigureerd in de indeling [NestedText](https://nestedtext.org) (`.nt`). Elke gebruiker heeft een afzonderlijk `.nt` configuratiebestand. U kunt de gebruikersprofielmap opgeven via `users_dir` in `config.nt` (waarin automatisch alle `.nt` bestanden in deze map worden opgenomen). Zowel de gebruiker als het padvoorvoegsel dat mag worden geüpload, gebruiken een ongeordende lijst (`-`). **Als het voorvoegsel van het uploadpad niet is ingevuld, is uploaden naar de hoofdmap van de share standaard toegestaan**.

1. Maak afzonderlijke `.nt` configuratiebestanden voor elk lid in de map `~/.config/sync0/users/`:

`~/.config/sync0/users/alice.nt` (alleen-lezen lid, tarief beperkt `80MB`, `rate` ondersteunt `GB`, `MB`, `KB` eenheden):

```yaml
id: 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c
perm: ro
rate: 80MB
```

`~/.config/sync0/users/bob.nt` (beperkte uploadleden: mag alleen uploaden naar de mappen `uploads/bob` en `shared/common`):

```yaml
id: 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f
perm: rw
write_prefix:
  - uploads/bob
  - shared/common
```

`~/.config/sync0/users/charlie.nt` (Standaard uploaden van leden: wanneer `write_prefix` wordt weggelaten, is uploaden naar de hoofdmap van de share standaard toegestaan):

```yaml
id: 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
perm: rw
```

2. Configureer de gebruikersmap en meerdere groepen voor het delen van bestanden in `~/.config/sync0/config.nt`:

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

Deelservice starten:

```bash
# Direct opstarten (laadt automatisch alle .nt-gebruikersconfiguraties in de mappen ~/.config/sync0/config.nt en users_dir)
sync0 share --region SHA

# Of publiceer snel meerdere deelgroepen en autoriseer groepsleden rechtstreeks via de opdrachtregel
sync0 share dataset:./data team-vault:./vault \
  --auth \
  --allow-ro 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c \
  --allow-rw 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f:uploads/bob,shared/common \
  --allow-rw 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
```

### 3. Haal alle inhoud op of selecteer meerdere bestanden en mappen (`sync0 pull`)

Maak verbinding met elk peer-knooppunt om automatisch alle beschikbare zaadknooppunten in de deelgroep te verkrijgen en tegelijkertijd uit meerdere zaadknooppunten te halen:

```bash
# Trek de volledige groep voor het delen van gegevenssets naar het lokale ./local_data
sync0 pull dataset:// ./local_data -p <knooppuntadres>

# Synchronisatie met meerdere selecties: haal tegelijkertijd het afzonderlijke bestand docs/readme.md en submappen assets/icons, src/core op
sync0 pull dataset://docs/readme.md,assets/icons,src/core ./local_data -p <knooppuntadres>
```

### 4. Upload een bestand of map naar een deelgroep (`sync0 push`)

Leden met uploadrechten (`rw` of `admin`) kunnen lokale bestanden of mappen uploaden naar groepen voor extern delen:

```bash
# Leden met uploadrechten in de hoofdmap: Upload rechtstreeks naar de gedeelde hoofdmap
sync0 push team-vault:// ./new_release -p <knooppuntadres>

# Alleen leden die geautoriseerd zijn voor een specifiek voorvoegsel (zoals uploads/bob): uploaden naar het toegestane padvoorvoegsel
sync0 push team-vault://uploads/bob/report.pdf ./report.pdf -p <knooppuntadres>
```

### 5. Abonneer u op automatische synchronisatie en plaatsing (`sync0 sub`)

Nadat u zich heeft geabonneerd op de deelgroep van een bepaald knooppunt, is de lokale server zowel een ontvangende client als een nieuwe zaaiserver; wanneer het bronknooppuntbestand wordt bijgewerkt, wordt het automatisch gesynchroniseerd en biedt het gefragmenteerde downloads voor andere knooppunten:

```bash
# Abonneer u op de gehele deelgroep en zaai automatisch
sync0 subdataset:// ./local_mirror -p <knooppuntadres>

# Alleen multi-selecteren om u te abonneren op specifieke bestanden en submappen en deze te seeden
sync0 subdataset://models/llama.onnx,configs ./local_mirror -p <knooppuntadres>
```

### 6. Peersynchronisatie met meerdere machines (`sync0 group`)

Meerdere machines sluiten zich aan bij dezelfde deelgroep, elkaar ontdekt automatisch beschikbare zaadknooppunten en onderhoudt tweerichtingssynchronisatie (ondersteunt volledige synchronisatie of meervoudige selectie van bestanden en mappen):

```bash
# Sluit u volledig aan bij de groep en tweerichtingssynchronisatie
sync0 group team-docs:// ./docs -p <knooppuntadres>

# Alleen de wal-map en het meta.json-bestand in de synchronisatiegroep met meerdere selecties
sync0 groep team-docs://wal,meta.json ./docs -p <knooppuntadres>
```

### 7. Directe point-to-point-overdracht tussen twee machines (`send` / `recv`)

Het is niet nodig om een ​​groep vooraf te configureren en u kunt bestanden of mappen tijdelijk rechtstreeks tussen twee machines overbrengen:

```bash
# Bewaking van de ontvanger
sync0 recv 0.0.0.0:9000 ./backup

# De verzendende kant draagt ​​rechtstreeks bestanden of mappen over (optioneel --delete mirror om overtollige bestanden op te ruimen, --rate snelheidslimiet)
sync0 send ./project 192.168.1.10:9000
```

### 8. Meertalig schakelen

`sync0` ondersteunt 75 talen en schakelt standaard automatisch over op basis van de systeemtaalomgeving; het kan ook handmatig worden opgegeven via `SYNC0_LANG`:

```bash
SYNC0_LANG=en sync0 --help
SYNC0_LANG=ja sync0 --help
SYNC0_LANG=de sync0 --help
```