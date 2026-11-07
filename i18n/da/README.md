# sync0

`sync0` er et kommandolinjeværktøj til overførsel af filer på tværs af netværk, trinvis synkronisering af mapper og samtidig distribution af data til flere maskiner.

## Original design hensigt

I maskinrum, lande eller miljøer med netværksforsinkelser og pakketab er værktøjer som `rsync` og `scp` baseret på bekræftelse af en enkelt forbindelsessekvens, hvilket begrænser båndbreddeudnyttelsen; når flere maskiner trækker data fra den samme server på samme tid, kan kildeservernetværkskortet nemt blive en flaskehals.

Målet med `sync0` er at løse to problemer:

1. Lad fil- og bibliotekssynkroniseringen mellem de to maskiner køre fuld fysisk båndbredde, og overfør kun den ændrede del, når filen er delvist ændret.
2. Flere maskiner kan overføre datablokke til hinanden, når de synkroniserer den samme mappe, hvilket reducerer belastningen på kildeserveren og forkorter den samlede synkroniseringstid.

## Hastighedssammenligning med rsync

Testmiljø: Europæisk node $\leftrightarrow$ Nordamerikansk node, tur-retur forsinkelse 86 ms, måling af den fuldstændige tidskrævende, inklusive biblioteksscanning, deduplikeringssammenligning, netværkstransmission og verifikation af diskplacering:

| Transmissionsscenarie | Datastørrelse | `sync0` | `rsync` | flere |
| :--- | :---: | :---: | :---: | :---: |
| 10.000 små filer | 77.6 MB | 0.90 s (688 Mbps) | 4.86 s (128 Mbps) | 5.4x |
| Kode- og arkivpakker (`.tar`) | 512 MB | 1.94 s (2111 Mbps) | 9.36 s (438 Mbps) | 4.8x |
| Videofil (`.mp4`) | 512 MB | 7.36 s (556 Mbps) | 12.11 s (338 Mbps) | 1.6x |
| Inkrementel synkronisering (rediger 1 % indhold) | 512 MB | 0.12 s | 1.85 s | 15.4x |

## Metoder til forbedring af ydeevnen

- **UDP samtidig transmission**: Send datablokke gennem UDP, og gentransmitter kun tabte fragmenter for at undgå, at et enkelt pakketab blokerer efterfølgende filoverførsler.
- **Indholdssegmentering og deduplikering**: Opdel filen i datablokke efter indhold og beregn checkværdien. Det samme indhold, der allerede er på den modtagende ende, genbruges direkte lokalt uden at duplikere netværket.
- **Juster automatisk komprimering i henhold til netværkshastighed**: Slå komprimering til for komprimerbart indhold såsom tekst og kode; spring automatisk komprimering over for videoer, billeder, komprimerede pakker eller under højhastigheds-LAN for at reducere CPU-forbruget.
- **Multi-node i nærheden trækker**: Når flere maskiner trækker den samme mappe, vil datablokke blive trukket samtidigt fra downloadede noder i den samme by eller med lavere latenstid.

## Installere

### Metode 1: Installer via last

```bash
cargo install sync0
```

### Metode 2: Download prækompilerede binære filer

Gå til [GitHub Releases](https://github.com/webc-site/sync0/releases) for at downloade den `.tar.xz` komprimerede pakke til det tilsvarende system (understøtter Linux x86_64/ARM64, macOS Apple Silicon/Intel, Windows x64/ARM64, Android ARM64/x86_64, FreeBSD, NetBSD). Efter udpakning skal du placere `sync0` i `PATH` mappen:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Brugervejledning

### 1. Overfør direkte filer eller mapper mellem to maskiner

Start overvågning på den modtagende maskine og angiv modtage- og gemmestien:

```bash
sync0 recv 0.0.0.0:9000 ./backup
```

Send en enkelt fil eller en hel mappe fra afsendermaskinen:

```bash
sync0 send ./project 192.168.1.10:9000
```

Ofte brugte parametre:

```bash
# Slet redundante filer, der findes i modtagerenden, men ikke i afsenderenden (spejlsynkronisering)
sync0 send ./project 192.168.1.10:9000 --delete

# Begræns afsendelseshastighed (enhed: bytes/sekund, f.eks. 50 MB/s)
sync0 send ./project 192.168.1.10:9000 --rate 50000000

# Lydløs tilstand (ingen statuslinje vist)
sync0 send ./project 192.168.1.10:9000 -q
```

### 2. Udgivelse af delte mapper og træk fra flere noder

Udgiv en delt mappe (navngivet `dataset`) på oprindelsesserveren:

```bash
sync0 share dataset ./data --region SHA
```

Træk den delte mappe på andre maskiner (du kan udfylde flere nodeadresser på samme tid, adskilt af kommaer for at trække samtidigt):

```bash
sync0 pull 10.0.0.1:9000,10.0.0.2:9000 dataset ./local_data
```

Træk kun en bestemt undermappe i den delte mappe:

```bash
sync0 pull 10.0.0.1:9000 dataset ./local_data --subpath 2026/logs
```

Skub lokale ændringer tilbage til den eksterne delte mappe:

```bash
sync0 push 10.0.0.1:9000 dataset ./local_data
```

### 3. Abonner på katalogets automatiske opdateringer

Abonner på den delte mappe for kildenoden. Når en fil tilføjes eller ændres i kildebiblioteket, vil den automatisk blive synkroniseret til den lokale computer. Samtidig vil den lokale maskine automatisk tjene som en node for andre maskiner at trække:

```bash
sync0 sub 10.0.0.1:9000 dataset ./local_mirror
```

### 4. To-vejs synkronisering af multi-maskine grupper

Flere maskiner slutter sig til det samme gruppenavn, og ændringer i enhver nodemappe vil automatisk blive synkroniseret med andre noder i gruppen:

```bash
sync0 group team-docs ./docs 10.0.0.1:9000,10.0.0.2:9000
```

### 5. Node-identitet og personligt visitkort

Hver node understøtter indstilling af brugernavn, by, e-mail, websted og en personlig signatur (maks. 256 tegn) og videregiver den lokale private nøglesignaturgodkendelse:

```bash
# Se det lokale offentlige nøgle-id (angiv konfigurationsmappen for at bevare nøglen)
sync0 id -c ~/.config/sync0

# Generer og vis nodevisitkort med personlige signaturer
sync0 profile -c ~/.config/sync0 \
  -u alice \
  --region SHA \
  --email alice@example.com \
  --url https://example.com \
  -b "Shanghai backup node"
```

`share`, `pull`, `sub`, `group` kan også startes med disse profilparametre:

```bash
sync0 share dataset ./data -c ~/.config/sync0 -u alice --region SHA -b "Shanghai backup node"
```

### 6. Skift på flere sprog

`sync0` Understøtter 75 sprog. Som standard vises kommandolinjehjælp og prompter på det tilsvarende sprog automatisk i henhold til operativsystemets sprogmiljø; hvis systemsproget ikke er på den understøttede liste, bruges engelsk som standard.

For at angive sproget manuelt skal du indstille miljøvariablen `SYNC0_LANG`:

```bash
SYNC0_LANG=en sync0 --help
SYNC0_LANG=ja sync0 --help
SYNC0_LANG=de sync0 --help
```