# sync0

`sync0` on käsurea tööriist failide ülekandmiseks võrkude vahel, kataloogide järkjärguliseks sünkroonimiseks ja andmete samaaegseks levitamiseks mitmele masinale.

## Algne disaini kavatsus

Masinatevahelistes ruumides, riikides või keskkondades, kus on võrguviivitused ja pakettakad, põhinevad sellised tööriistad nagu `rsync` ja `scp` ühe ühenduse jada kinnitusel, mis piirab ribalaiuse kasutamist; kui mitu masinat tõmbavad andmeid samast serverist korraga, võib lähteserveri võrgukaart kergesti muutuda kitsaskohaks.

Funktsiooni `sync0` eesmärk on lahendada kaks probleemi:

1. Laske failide ja kataloogide sünkroonimisel kahe masina vahel töötada kogu füüsilise ribalaiusega ja edastada muudetud osa ainult siis, kui faili on osaliselt muudetud.
2. Mitu masinat saavad sama kataloogi sünkroonimisel andmeplokke üksteisele edastada, vähendades lähteserveri koormust ja lühendades üldist sünkroonimisaega.

## Kiiruse võrdlus rsynciga

Testikeskkond: Euroopa sõlm $\leftrightarrow$ Põhja-Ameerika sõlm, edasi-tagasi viivitus 86 ms, kogu aeganõudva mõõtmine, sealhulgas kataloogide skannimine, dubleerimise võrdlus, võrguedastus ja ketta paigutuse kontrollimine:

| Edastamise stsenaarium | Andmemaht | `sync0` | `rsync` | mitmekordne |
| :--- | :---: | :---: | :---: | :---: |
| 10 000 väikest faili | 77.6 MB | 0.90 s (688 Mbps) | 4.86 s (128 Mbps) | 5.4x |
| Koodi- ja arhiivipaketid (`.tar`) | 512 MB | 1.94 s (2111 Mbps) | 9.36 s (438 Mbps) | 4.8x |
| Videofail (`.mp4`) | 512 MB | 7.36 s (556 Mbps) | 12.11 s (338 Mbps) | 1.6x |
| Järkjärguline sünkroonimine (muuda 1% sisu) | 512 MB | 0.12 s | 1.85 s | 15.4x |

## Tulemuslikkuse parandamise meetodid

- **UDP samaaegne edastamine**: saatke andmeplokid UDP kaudu ja edastage uuesti ainult kadunud fragmendid, et vältida ühe paketi kadu, mis blokeerib järgnevaid failiedastusi.
- **Sisu ​​segmenteerimine ja dubleerimine**: jagage fail vastavalt sisule andmeplokkideks ja arvutage kontrollväärtus. Sama sisu, mis on juba vastuvõtvas otsas, kasutatakse uuesti otse kohapeal, ilma võrku dubleerimata.
- **Reguleerige tihendamist automaatselt vastavalt võrgu kiirusele**: lülitage sisse tihendatava sisu (nt tekst ja kood) tihendamine; jätke automaatselt vahele videote, piltide, tihendatud pakettide või kiire LAN-i pakkimine, et vähendada protsessori tarbimist.
- **Mitme sõlme läheduses olev tõmbamine**: kui mitu masinat tõmbavad sama kataloogi, tõmmatakse andmeplokid samaaegselt allalaaditud sõlmedest samas linnas või väiksema latentsusega.

## Installige

### 1. meetod: paigaldage Cargo kaudu

```bash
cargo install sync0
```

### 2. meetod: laadige alla eelkompileeritud kahendfailid

Minge jaotisesse [GitHub Releases](https://github.com/webc-site/sync0/releases), et laadida alla `.tar.xz` tihendatud pakett vastava süsteemi jaoks (toetab Linux x86_64/ARM64, macOS Apple Silicon/Intel, Windows x64/ARM64, Android ARM64/x86_64, FreeBSD, NetBSD). Pärast lahtipakkimist asetage `sync0` kataloogi `PATH`:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Kasutusjuhend

### 1. Otse failide või kataloogide ülekandmine kahe masina vahel

Alustage vastuvõtvas masinas jälgimist ja määrake vastuvõtu- ja salvestamistee:

```bash
sync0 recv 0.0.0.0:9000 ./backup
```

Saatke saatvast masinast üks fail või terve kataloog:

```bash
sync0 send ./project 192.168.1.10:9000
```

Tavaliselt kasutatavad parameetrid:

```bash
# Kustutage üleliigsed failid, mis on vastuvõtvas, kuid mitte saatvas otsas (peegelsünkroonimine)
sync0 send ./project 192.168.1.10:9000 --delete

# Piiratud saatmiskiirus (ühik: baiti sekundis, näiteks 50 MB/s)
sync0 send ./project 192.168.1.10:9000 --rate 50000000

# Vaikne režiim (edenemisriba ei kuvata)
sync0 send ./project 192.168.1.10:9000 -q
```

### 2. Jagatud kataloogide avaldamine ja mitmest sõlmest tõmbamine

Avalda jagatud kataloog (nimega `dataset`) lähteserveris:

```bash
sync0 share dataset ./data --region SHA
```

Tõmmake jagatud kataloog teistes masinates (saate samaaegseks tõmbamiseks sisestada mitu sõlme aadressi, eraldades need komadega):

```bash
sync0 pull 10.0.0.1:9000,10.0.0.2:9000 dataset ./local_data
```

Tõmmake jagatud kataloogis ainult teatud alamkataloog:

```bash
sync0 pull 10.0.0.1:9000 dataset ./local_data --subpath 2026/logs
```

Lükake kohalikud muudatused tagasi jagatud kaugkataloogi:

```bash
sync0 push 10.0.0.1:9000 dataset ./local_data
```

### 3. Tellige kataloogi automaatsed värskendused

Tellige lähtesõlme jagatud kataloog. Kui fail lisatakse või seda muudetakse lähtekataloogi, sünkroonitakse see automaatselt kohaliku arvutiga. Samal ajal toimib kohalik masin automaatselt sõlmena teistele masinatele, mida saab tõmmata:

```bash
sync0 sub 10.0.0.1:9000 dataset ./local_mirror
```

### 4. Mitme masinaga rühmade kahesuunaline sünkroonimine

Sama rühmanimega liituvad mitu masinat ja muudatused mis tahes sõlme kataloogis sünkroonitakse automaatselt rühma teiste sõlmedega:

```bash
sync0 group team-docs ./docs 10.0.0.1:9000,10.0.0.2:9000
```

### 5. Sõlme identiteet ja isiklik visiitkaart

Iga sõlm toetab kasutajanime, linna, e-posti, veebisaidi ja isikupärastatud allkirja (maksimaalselt 256 tähemärki) seadistamist ning edastab kohaliku privaatvõtme allkirja autentimise:

```bash
# Vaadake kohaliku avaliku võtme ID-d (võtme säilitamiseks määrake konfiguratsioonikataloog)
sync0 id -c ~/.config/sync0

# Looge ja kuvage isikupärastatud allkirjadega sõlme visiitkaarte
sync0 profile -c ~/.config/sync0 \
  -u alice \
  --region SHA \
  --email alice@example.com \
  --url https://example.com \
  -b "Shanghai backup node"
```

`share`, `pull`, `sub`, `group` saab käivitada ka järgmiste profiiliparameetritega:

```bash
sync0 share dataset ./data -c ~/.config/sync0 -u alice --region SHA -b "Shanghai backup node"
```

### 6. Mitme keele vahetamine

`sync0` Toetab 75 keelt. Vaikimisi kuvatakse vastavalt operatsioonisüsteemi keelekeskkonnale automaatselt vastavas keeles käsurea abi ja viipasid; kui süsteemi keelt ei ole toetatud loendis, kasutatakse vaikimisi inglise keelt.

Keele käsitsi määramiseks määrake keskkonnamuutuja `SYNC0_LANG`:

```bash
SYNC0_LANG=en sync0 --help
SYNC0_LANG=ja sync0 --help
SYNC0_LANG=de sync0 --help
```