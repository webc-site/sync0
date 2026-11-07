# sync0

`sync0` er et kommandolinjeverktøy for overføring av filer på tvers av nettverk, trinnvis synkronisering av kataloger og samtidig distribusjon av data til flere maskiner.

## Original design intensjon

I rom på tvers av maskin, land eller miljøer med nettverksforsinkelser og pakketap, er verktøy som `rsync` og `scp` basert på bekreftelse av enkelt tilkoblingssekvens, som begrenser båndbreddeutnyttelsen; når flere maskiner trekker data fra samme server samtidig, kan kildeservernettverkskortet lett bli en flaskehals.

Målet med `sync0` er å løse to problemer:

1. La fil- og katalogsynkroniseringen mellom de to maskinene kjøre full fysisk båndbredde, og overfør kun den endrede delen når filen er delvis modifisert.
2. Flere maskiner kan overføre datablokker til hverandre når de synkroniserer samme katalog, reduserer belastningen på kildeserveren og forkorter den totale synkroniseringstiden.

## Hastighetssammenligning med rsync

Testmiljø: Europeisk node $\leftrightarrow$ Nordamerikansk node, tur-retur-forsinkelse 86 ms, måling av hele tidkrevende inkludert katalogskanning, dedupliseringssammenligning, nettverksoverføring og verifisering av diskplassering:

| Overføringsscenario | Datastørrelse | `sync0` | `rsync` | flere |
| :--- | :---: | :---: | :---: | :---: |
| 10 000 små filer | 77.6 MB | 0.90 s (688 Mbps) | 4.86 s (128 Mbps) | 5.4x |
| Kode og arkivpakker (`.tar`) | 512 MB | 1.94 s (2111 Mbps) | 9.36 s (438 Mbps) | 4.8x |
| Videofil (`.mp4`) | 512 MB | 7.36 s (556 Mbps) | 12.11 s (338 Mbps) | 1.6x |
| Inkrementell synkronisering (endre 1 % innhold) | 512 MB | 0.12 s | 1.85 s | 15.4x |

## Metoder for ytelsesforbedring

- **UDP samtidig overføring**: Send datablokker gjennom UDP, og send bare tapte fragmenter på nytt for å unngå at ett enkelt pakketap blokkerer påfølgende filoverføringer.
- **Innholdssegmentering og deduplisering**: Del opp filen i datablokker i henhold til innhold og beregn sjekkverdien. Det samme innholdet som allerede er på mottakersiden gjenbrukes direkte lokalt uten å duplisere nettverket.
- **Juster automatisk komprimering i henhold til nettverkshastighet**: Slå på komprimering for komprimerbart innhold som tekst og kode; automatisk hoppe over komprimering for videoer, bilder, komprimerte pakker eller under høyhastighets LAN for å redusere CPU-forbruket.
- **Truk av flere noder i nærheten**: Når flere maskiner trekker den samme katalogen, vil datablokker trekkes samtidig fra nedlastede noder i samme by eller med lavere ventetid.

## Installer

### Metode 1: Installer via Cargo

```bash
cargo install sync0
```

### Metode 2: Last ned forhåndskompilerte binærfiler

Gå til [GitHub Releases](https://github.com/webc-site/sync0/releases) for å laste ned den `.tar.xz` komprimerte pakken for det tilsvarende systemet (støtter Linux x86_64/ARM64, macOS Apple Silicon/Intel, Windows x64/ARM64, Android ARM64/x86_64, FreeBSD, NetBSD). Etter utpakning legger du `sync0` inn i `PATH`-katalogen:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Brukerveiledning

### 1. Overfør filer eller kataloger direkte mellom to maskiner

Start overvåking på mottaksmaskinen og spesifiser mottaks- og lagringsbanen:

```bash
sync0 recv 0.0.0.0:9000 ./backup
```

Send en enkelt fil eller en hel katalog fra avsendermaskinen:

```bash
sync0 send ./project 192.168.1.10:9000
```

Vanlig brukte parametere:

```bash
# Slett redundante filer som finnes på mottakersiden, men ikke på avsenderenden (speilsynkronisering)
sync0 send ./project 192.168.1.10:9000 --delete

# Begrens sendehastighet (enhet: byte/sekund, for eksempel 50 MB/s)
sync0 send ./project 192.168.1.10:9000 --rate 50000000

# Stille modus (ingen fremdriftslinje vist)
sync0 send ./project 192.168.1.10:9000 -q
```

### 2. Publiserer delte kataloger og henter fra flere noder

Publiser en delt katalog (kalt `dataset`) på opprinnelsesserveren:

```bash
sync0 share dataset ./data --region SHA
```

Trekk den delte katalogen på andre maskiner (du kan fylle ut flere nodeadresser samtidig, atskilt med kommaer for å trekke samtidig):

```bash
sync0 pull 10.0.0.1:9000,10.0.0.2:9000 dataset ./local_data
```

Trekk bare en bestemt underkatalog i den delte katalogen:

```bash
sync0 pull 10.0.0.1:9000 dataset ./local_data --subpath 2026/logs
```

Skyv lokale endringer tilbake til den eksterne delte katalogen:

```bash
sync0 push 10.0.0.1:9000 dataset ./local_data
```

### 3. Abonner på automatiske katalogoppdateringer

Abonner på den delte katalogen til kildenoden. Når en fil legges til eller endres i kildekatalogen, vil den automatisk bli synkronisert til den lokale datamaskinen. Samtidig vil den lokale maskinen automatisk tjene som en node for andre maskiner å trekke:

```bash
sync0 sub 10.0.0.1:9000 dataset ./local_mirror
```

### 4. Toveis synkronisering av multimaskingrupper

Flere maskiner slutter seg til samme gruppenavn, og endringer i enhver nodekatalog vil automatisk synkroniseres med andre noder i gruppen:

```bash
sync0 group team-docs ./docs 10.0.0.1:9000,10.0.0.2:9000
```

### 5. Node-identitet og personlig visittkort

Hver node støtter innstilling av brukernavn, by, e-post, nettside og en personlig signatur (maksimalt 256 tegn), og passerer den lokale private nøkkelsignaturautentiseringen:

```bash
# Se den lokale offentlige nøkkel-IDen (spesifiser konfigurasjonskatalogen for å opprettholde nøkkelen)
sync0 id -c ~/.config/sync0

# Generer og vis node visittkort med personlige signaturer
sync0 profile -c ~/.config/sync0 \
  -u alice \
  --region SHA \
  --email alice@example.com \
  --url https://example.com \
  -b "Shanghai backup node"
```

`share`, `pull`, `sub`, `group` kan også startes med disse profilparametrene:

```bash
sync0 share dataset ./data -c ~/.config/sync0 -u alice --region SHA -b "Shanghai backup node"
```

### 6. Bytting av flere språk

`sync0` Støtter 75 språk. Som standard vises kommandolinjehjelp og ledetekster på det tilsvarende språket automatisk i henhold til språkmiljøet til operativsystemet; hvis systemspråket ikke er på den støttede listen, brukes engelsk som standard.

For å spesifisere språket manuelt, sett miljøvariabelen `SYNC0_LANG`:

```bash
SYNC0_LANG=en sync0 --help
SYNC0_LANG=ja sync0 --help
SYNC0_LANG=de sync0 --help
```