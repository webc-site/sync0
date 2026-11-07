# sync0

`sync0` er skipanalínutól til að flytja skrár yfir netkerfi, samstilla skrár í skrefum og dreifa gögnum samtímis á margar vélar.

## Upprunaleg hönnun ætlun

Í þversum vélaherbergjum, löndum eða umhverfi með nettafir og pakkatap, eru verkfæri eins og `rsync` og `scp` byggð á einni tengingaröð staðfestingu, sem takmarkar bandbreiddarnýtingu; þegar margar vélar draga gögn frá sama netþjóni á sama tíma getur netkort upprunaþjónsins auðveldlega orðið flöskuháls.

Markmið `sync0` er að leysa tvö vandamál:

1. Látið samstillingu skráar og möppu á milli vélanna tveggja keyra fulla líkamlega bandbreidd og flytjið aðeins breytta hlutann þegar skránni er breytt að hluta.
2. Margar vélar geta flutt gagnablokkir til hvors annars þegar samstillt er í sömu möppunni, minnkað álagið á frumþjóninn og stytt heildarsamstillingartímann.

## Hraðasamanburður við rsync

Prófunarumhverfi: Evrópskur hnútur $\leftrightarrow$ Norður-Ameríkuhnútur, seinkun fram og til baka 86 ms, mælir allan tímafreka þ.mt skráarskönnun, samanburð á aftvítekningu, netsendingu og staðfestingu á diskastaðsetningu:

| Sendingaratburðarás | Gagnastærð | `sync0` | `rsync` | margfaldur |
| :--- | :---: | :---: | :---: | :---: |
| 10.000 litlar skrár | 77.6 MB | 0.90 s (688 Mbps) | 4.86 s (128 Mbps) | 5.4x |
| Kóði og geymslupakkar (`.tar`) | 512 MB | 1.94 s (2111 Mbps) | 9.36 s (438 Mbps) | 4.8x |
| Myndbandsskrá (`.mp4`) | 512 MB | 7.36 s (556 Mbps) | 12.11 s (338 Mbps) | 1.6x |
| Stigvaxandi samstilling (breyta 1% innihaldi) | 512 MB | 0.12 s | 1.85 s | 15.4x |

## Aðferðir til að bæta árangur

- **UDP samhliða sending**: Sendu gagnablokkir í gegnum UDP og endursendu aðeins týnd brot til að koma í veg fyrir að eitt pakkatap hindri síðari skráaflutning.
- **Skilting efnis og aftvíföldun**: Skiptu skránni í gagnablokkir í samræmi við innihald og reiknaðu ávísunargildið. Sama efni sem þegar er á móttökuendanum er endurnýtt beint á staðnum án þess að afrita netið.
- **Stilltu þjöppun sjálfkrafa í samræmi við nethraða**: Kveiktu á þjöppun fyrir þjappanlegt efni eins og texta og kóða; sleppa sjálfkrafa þjöppun fyrir myndbönd, myndir, þjappaða pakka eða undir háhraða staðarneti til að draga úr örgjörvanotkun.
- **Tildráttur margra hnúta í nágrenninu**: Þegar margar vélar draga sömu möppuna, verða gagnablokkir teknar samtímis frá niðurhaluðum hnútum í sömu borg eða með minni leynd.

## Settu upp

### Aðferð 1: Settu upp í gegnum farm

```bash
cargo install sync0
```

### Aðferð 2: Hlaða niður forsamsettum tvíþættum

Farðu í [GitHub útgáfur](https://github.com/webc-site/sync0/releases) til að hlaða niður `.tar.xz` þjappaða pakkanum fyrir samsvarandi kerfi (styður Linux x86_64/ARM64, macOS Apple Silicon/Intel, Windows x64/ARM64, Android ARM64/x86_64, FreeBSD, NetBSD). Eftir að hafa pakkað upp skaltu setja `sync0` í `PATH` möppuna:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Notendahandbók

### 1. Flyttu skrár eða möppur beint á milli tveggja véla

Byrjaðu að fylgjast með móttökuvélinni og tilgreindu móttöku- og vistunarleiðina:

```bash
sync0 recv 0.0.0.0:9000 ./backup
```

Sendu eina skrá eða heila skrá frá sendivélinni:

```bash
sync0 send ./project 192.168.1.10:9000
```

Algengar færibreytur:

```bash
# Eyða óþarfa skrám sem eru til á móttökuendanum en ekki á sendandanum (spegill samstilling)
sync0 send ./project 192.168.1.10:9000 --delete

# Takmarka sendingarhraða (eining: bæti/sekúndu, til dæmis 50 MB/s)
sync0 send ./project 192.168.1.10:9000 --rate 50000000

# Hljóðlaus stilling (engin framvindustika sýnd)
sync0 send ./project 192.168.1.10:9000 -q
```

### 2. Gefa út sameiginlegar möppur og draga úr mörgum hnútum

Birtu sameiginlega skrá (sem heitir `dataset`) á upprunaþjóninum:

```bash
sync0 share dataset ./data --region SHA
```

Dragðu samnýttu möppuna á öðrum vélum (þú getur fyllt út mörg hnútavistföng á sama tíma, aðskilin með kommum til að draga samtímis):

```bash
sync0 pull 10.0.0.1:9000,10.0.0.2:9000 dataset ./local_data
```

Dragðu aðeins tiltekna undirskrá í samnýttu möppunni:

```bash
sync0 pull 10.0.0.1:9000 dataset ./local_data --subpath 2026/logs
```

Ýttu staðbundnum breytingum aftur í ytri samnýttu möppuna:

```bash
sync0 push 10.0.0.1:9000 dataset ./local_data
```

### 3. Gerast áskrifandi að sjálfvirkum uppfærslum vörulista

Gerast áskrifandi að samnýttu möppu upprunahnútsins. Þegar skrá er bætt við eða henni breytt í upprunaskránni verður hún sjálfkrafa samstillt við staðbundna tölvuna. Á sama tíma mun staðbundin vél sjálfkrafa þjóna sem hnút fyrir aðrar vélar til að draga:

```bash
sync0 sub 10.0.0.1:9000 dataset ./local_mirror
```

### 4. Tvíhliða samstilling fjölvélahópa

Margar vélar sameinast sama hópnafni og breytingar á hvaða hnútaskrá sem er verða sjálfkrafa samstilltar við aðra hnúta í hópnum:

```bash
sync0 group team-docs ./docs 10.0.0.1:9000,10.0.0.2:9000
```

### 5. Hnútaauðkenni og persónulegt nafnspjald

Hver hnútur styður stillingu notandanafns, borg, tölvupósts, vefsíðu og sérsniðinnar undirskriftar (hámark 256 stafir) og framhjá staðbundinni auðkenningu undirskriftar einkalykils:

```bash
# Skoða staðbundið auðkenni almenningslykils (tilgreindu stillingarskrána til að halda lykilnum áfram)
sync0 id -c ~/.config/sync0

# Búðu til og sýndu hnúta nafnspjöld með sérsniðnum undirskriftum
sync0 profile -c ~/.config/sync0 \
  -u alice \
  --region SHA \
  --email alice@example.com \
  --url https://example.com \
  -b "Shanghai backup node"
```

`share`, `pull`, `sub`, `group` er einnig hægt að ræsa með þessum prófílfæribreytum:

```bash
sync0 share dataset ./data -c ~/.config/sync0 -u alice --region SHA -b "Shanghai backup node"
```

### 6. Að skipta á mörgum tungumálum

`sync0` Styður 75 tungumál. Sjálfgefið er að skipanalínuhjálp og kvaðningar á samsvarandi tungumáli birtast sjálfkrafa í samræmi við tungumálaumhverfi stýrikerfisins; ef kerfismálið er ekki á studdum listanum er enska sjálfgefið notuð.

Til að tilgreina tungumálið handvirkt skaltu stilla umhverfisbreytuna `SYNC0_LANG`:

```bash
SYNC0_LANG=en sync0 --help
SYNC0_LANG=ja sync0 --help
SYNC0_LANG=de sync0 --help
```