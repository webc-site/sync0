# sync0

`sync0` is 'n opdragreëlhulpmiddel vir die oordrag van lêers oor netwerke, inkrementele sinchronisering van dopgehou, en gelyktydige verspreiding van data na veelvuldige masjiene.

## Oorspronklike ontwerp voorneme

In kruismasjienkamers, lande of omgewings met netwerkvertragings en pakkieverlies, is gereedskap soos `rsync` en `scp` gebaseer op enkelverbindingsvolgordebevestiging, wat bandwydtebenutting beperk; wanneer verskeie masjiene data van dieselfde bediener op dieselfde tyd trek, kan die bronbedienernetwerkkaart maklik 'n bottelnek word.

Die doel van `sync0` is om twee probleme op te los:

1. Laat die lêer- en gidssinchronisasie tussen die twee masjiene volle fisiese bandwydte loop, en dra slegs die veranderde deel oor wanneer die lêer gedeeltelik gewysig is.
2. Veelvuldige masjiene kan datablokke na mekaar oordra wanneer dieselfde gids gesinchroniseer word, wat die las op die bronbediener verminder en die algehele sinchronisasietyd verkort.

## Spoedvergelyking met rsync

Toetsomgewing: Europese nodus $\leftrightarrow$ Noord-Amerikaanse nodus, retoervertraging 86 ms, meet die volledige tydrowende insluitend gidsskandering, dedupliseringsvergelyking, netwerkoordrag en skyfplasingverifikasie:

| Transmissie scenario | Data grootte | `sync0` | `rsync` | veelvuldig |
| :--- | :---: | :---: | :---: | :---: |
| 10 000 klein lêers | 77.6 MB | 0.90 s (688 Mbps) | 4.86 s (128 Mbps) | 5.4x |
| Kode en argief pakkette (`.tar`) | 512 MB | 1.94 s (2111 Mbps) | 9.36 s (438 Mbps) | 4.8x |
| Videolêer (`.mp4`) | 512 MB | 7.36 s (556 Mbps) | 12.11 s (338 Mbps) | 1.6x |
| Inkrementele sinchronisasie (verander 1% inhoud) | 512 MB | 0.12 s | 1.85 s | 15.4x |

## Metodes vir prestasieverbetering

- **UDP gelyktydige oordrag**: Stuur datablokke deur UDP, en versend slegs verlore fragmente weer om te verhoed dat 'n enkele pakkieverlies daaropvolgende lêeroordragte blokkeer.
- **Inhoudsegmentering en deduplisering**: Verdeel die lêer in datablokke volgens inhoud en bereken die tjekwaarde. Dieselfde inhoud wat reeds aan die ontvangkant is, word direk plaaslik hergebruik sonder om die netwerk te dupliseer.
- ** Pas kompressie outomaties aan volgens netwerkspoed**: Skakel kompressie aan vir saamdrukbare inhoud soos teks en kode; slaan outomaties kompressie oor vir video's, prente, saamgeperste pakkette of onder hoëspoed-LAN om SVE-verbruik te verminder.
- **Multi-node nabygeleë trek**: Wanneer veelvuldige masjiene dieselfde gids trek, sal datablokke gelyktydig van afgelaaide nodusse in dieselfde stad of met laer latency getrek word.

## Installeer

### Metode 1: Installeer via Cargo

```bash
cargo install sync0
```

### Metode 2: Laai vooraf saamgestelde binaries af

Gaan na [GitHub Releases](https://github.com/webc-site/sync0/releases) om die `.tar.xz` saamgeperste pakket vir die ooreenstemmende stelsel af te laai (ondersteun Linux x86_64/ARM64, macOS Apple Silicon/Intel, Windows x64/ARM64, Android ARM64/x86_64, FreeBSD, NetBSD). Nadat u dit uitgepak het, plaas `sync0` in die `PATH` gids:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Gebruikersgids

### 1. Dra lêers of gidse direk oor tussen twee masjiene

Begin monitering op die ontvangsmasjien en spesifiseer die ontvangs- en stoorpad:

```bash
sync0 recv 0.0.0.0:9000 ./backup
```

Stuur 'n enkele lêer of 'n hele gids vanaf die stuurmasjien:

```bash
sync0 send ./project 192.168.1.10:9000
```

Algemene gebruikte parameters:

```bash
# Vee oortollige lêers uit wat aan die ontvangkant bestaan, maar nie aan die stuurkant nie (spieëlsinchronisasie)
sync0 send ./project 192.168.1.10:9000 --delete

# Beperk stuurtempo (eenheid: grepe/sekonde, byvoorbeeld 50 MB/s)
sync0 send ./project 192.168.1.10:9000 --rate 50000000

# Stil modus (geen vorderingsbalk gewys nie)
sync0 send ./project 192.168.1.10:9000 -q
```

### 2. Publiseer gedeelde dopgehou en trek van verskeie nodusse af

Publiseer 'n gedeelde gids (met die naam `dataset`) op die oorsprongbediener:

```bash
sync0 share dataset ./data --region SHA
```

Trek die gedeelde gids op ander masjiene (jy kan verskeie nodus-adresse op dieselfde tyd invul, geskei deur kommas om gelyktydig te trek):

```bash
sync0 pull 10.0.0.1:9000,10.0.0.2:9000 dataset ./local_data
```

Trek slegs 'n sekere subgids in die gedeelde gids:

```bash
sync0 pull 10.0.0.1:9000 dataset ./local_data --subpath 2026/logs
```

Druk plaaslike veranderinge terug na die afgeleë gedeelde gids:

```bash
sync0 push 10.0.0.1:9000 dataset ./local_data
```

### 3. Teken in op katalogus outomatiese opdaterings

Teken in op die gedeelde gids van die bronnodus. Wanneer 'n lêer bygevoeg of gewysig word in die brongids, sal dit outomaties met die plaaslike rekenaar gesinchroniseer word. Terselfdertyd sal die plaaslike masjien outomaties dien as 'n nodus vir ander masjiene om te trek:

```bash
sync0 sub 10.0.0.1:9000 dataset ./local_mirror
```

### 4. Tweerigtingsinchronisasie van multimasjiengroepe

Veelvuldige masjiene sluit by dieselfde groepnaam aan, en veranderinge in enige nodusgids sal outomaties met ander nodusse in die groep gesinchroniseer word:

```bash
sync0 group team-docs ./docs 10.0.0.1:9000,10.0.0.2:9000
```

### 5. Node-identiteit en persoonlike besigheidskaartjie

Elke nodus ondersteun die instelling van die gebruikersnaam, stad, e-pos, webwerf en 'n gepersonaliseerde handtekening (maksimum 256 karakters), en slaag die plaaslike private sleutel handtekening verifikasie:

```bash
# Bekyk die plaaslike publieke sleutel-ID (spesifiseer die konfigurasiegids om die sleutel te behou)
sync0 id -c ~/.config/sync0

# Genereer en vertoon node-besigheidskaartjies met gepersonaliseerde handtekeninge
sync0 profile -c ~/.config/sync0 \
  -u alice \
  --region SHA \
  --email alice@example.com \
  --url https://example.com \
  -b "Shanghai backup node"
```

`share`, `pull`, `sub`, `group` kan ook met hierdie profielparameters bekendgestel word:

```bash
sync0 share dataset ./data -c ~/.config/sync0 -u alice --region SHA -b "Shanghai backup node"
```

### 6. Multi-taal skakeling

`sync0` Ondersteun 75 tale. By verstek word opdragreëlhulp en aanwysings in die ooreenstemmende taal outomaties vertoon volgens die taalomgewing van die bedryfstelsel; as die stelseltaal nie in die ondersteunde lys is nie, word Engels by verstek gebruik.

Om die taal handmatig te spesifiseer, stel die omgewingsveranderlike `SYNC0_LANG`:

```bash
SYNC0_LANG=en sync0 --help
SYNC0_LANG=ja sync0 --help
SYNC0_LANG=de sync0 --help
```