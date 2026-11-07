# sync0

`sync0` yra komandų eilutės įrankis, skirtas failams perkelti tinkluose, laipsniškai sinchronizuoti katalogus ir vienu metu paskirstyti duomenis keliems įrenginiams.

## Originalus dizaino tikslas

Įvairiose mašinose, šalyse ar aplinkose, kuriose yra tinklo vėlavimų ir paketų praradimo, tokie įrankiai kaip `rsync` ir `scp` yra pagrįsti vienos ryšio sekos patvirtinimu, kuris riboja pralaidumo naudojimą; kai keli įrenginiai vienu metu traukia duomenis iš to paties serverio, šaltinio serverio tinklo plokštė gali lengvai tapti kliūtimi.

`sync0` tikslas yra išspręsti dvi problemas:

1. Leiskite failų ir katalogų sinchronizavimui tarp dviejų įrenginių vykdyti visą fizinį pralaidumą, o pakeistą dalį perkelti tik tada, kai failas iš dalies pakeistas.
2. Keli įrenginiai gali perduoti duomenų blokus vienas kitam sinchronizuodami tą patį katalogą, sumažindami šaltinio serverio apkrovą ir sutrumpindami bendrą sinchronizavimo laiką.

## Greičio palyginimas su rsync

Bandymo aplinka: Europos mazgas $\leftrightarrow$ Šiaurės Amerikos mazgas, 86 ms uždelsimas pirmyn ir atgal, išmatuojant visą laiką, įskaitant katalogų nuskaitymą, dubliavimo panaikinimą, tinklo perdavimą ir disko vietos patikrinimą:

| Perdavimo scenarijus | Duomenų dydis | `sync0` | `rsync` | daugkartinis |
| :--- | :---: | :---: | :---: | :---: |
| 10 000 mažų failų | 77.6 MB | 0.90 s (688 Mbps) | 4.86 s (128 Mbps) | 5.4x |
| Kodo ir archyvo paketai (`.tar`) | 512 MB | 1.94 s (2111 Mbps) | 9.36 s (438 Mbps) | 4.8x |
| Vaizdo įrašo failas (`.mp4`) | 512 MB | 7.36 s (556 Mbps) | 12.11 s (338 Mbps) | 1.6x |
| Laipsniškas sinchronizavimas (keisti 1 % turinį) | 512 MB | 0.12 s | 1.85 s | 15.4x |

## Veiklos tobulinimo metodai

- **Vienu metu vykstantis UDP perdavimas**: siųskite duomenų blokus per UDP ir pakartotinai persiųskite tik prarastus fragmentus, kad išvengtumėte vieno paketo praradimo, blokuojančio tolesnius failų perkėlimus.
- **Turinio segmentavimas ir dubliavimo panaikinimas**: suskaidykite failą į duomenų blokus pagal turinį ir apskaičiuokite patikros reikšmę. Tas pats turinys, jau esantis gavėjo pusėje, pakartotinai naudojamas tiesiogiai vietoje, nedubliuojant tinklo.
- **Automatiškai koreguokite glaudinimą pagal tinklo greitį**: įjunkite suspaudžiamo turinio, pvz., teksto ir kodo, glaudinimą; automatiškai praleisti vaizdo įrašų, nuotraukų, suspaustų paketų arba didelės spartos LAN glaudinimą, kad sumažintumėte procesoriaus sąnaudas.
- **Kelių mazgų ištraukimas netoliese**: kai keli įrenginiai traukia tą patį katalogą, duomenų blokai bus ištraukiami vienu metu iš atsisiųstų mazgų tame pačiame mieste arba su mažesne delsa.

## Įdiegti

### 1 būdas: įdiekite per „Cargo“.

```bash
cargo install sync0
```

### 2 būdas: atsisiųskite iš anksto sukompiliuotus dvejetainius failus

Eikite į [GitHub Releases](https://github.com/webc-site/sync0/releases), kad atsisiųstumėte `.tar.xz` suglaudintą atitinkamos sistemos paketą (palaiko Linux x86_64/ARM64, macOS Apple Silicon/Intel, Windows x64/ARM64, Android ARM64/x86_64, FreeBSD, NetBSD). Išpakavę įdėkite `sync0` į `PATH` katalogą:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Vartotojo vadovas

### 1. Tiesiogiai perkelkite failus ar katalogus tarp dviejų įrenginių

Pradėkite stebėjimą priimančiame įrenginyje ir nurodykite priėmimo ir išsaugojimo kelią:

```bash
sync0 recv 0.0.0.0:9000 ./backup
```

Siųsti vieną failą arba visą katalogą iš siuntimo įrenginio:

```bash
sync0 send ./project 192.168.1.10:9000
```

Dažniausiai naudojami parametrai:

```bash
# Ištrinkite perteklinius failus, kurie yra gavėjo pusėje, bet ne siuntimo pusėje (veidrodinis sinchronizavimas)
sync0 send ./project 192.168.1.10:9000 --delete

# Ribotas siuntimo greitis (vienetas: baitai per sekundę, pavyzdžiui, 50 MB/s)
sync0 send ./project 192.168.1.10:9000 --rate 50000000

# Tylus režimas (nerodoma eigos juosta)
sync0 send ./project 192.168.1.10:9000 -q
```

### 2. Bendrinamų katalogų publikavimas ir traukimas iš kelių mazgų

Paskelbkite bendrinamą katalogą (pavadintą `dataset`) kilmės serveryje:

```bash
sync0 share dataset ./data --region SHA
```

Ištraukite bendrinamą katalogą kituose įrenginiuose (galite vienu metu įrašyti kelių mazgų adresus, atskirtus kableliais, kad būtų traukiama vienu metu):

```bash
sync0 pull 10.0.0.1:9000,10.0.0.2:9000 dataset ./local_data
```

Ištraukite tik tam tikrą pakatalogį bendrame kataloge:

```bash
sync0 pull 10.0.0.1:9000 dataset ./local_data --subpath 2026/logs
```

Nusiųskite vietinius pakeitimus atgal į nuotolinį bendrinamą katalogą:

```bash
sync0 push 10.0.0.1:9000 dataset ./local_data
```

### 3. Prenumeruokite automatinius katalogo atnaujinimus

Prenumeruokite bendrinamą šaltinio mazgo katalogą. Kai failas įtraukiamas arba modifikuojamas šaltinio kataloge, jis automatiškai sinchronizuojamas su vietiniu kompiuteriu. Tuo pačiu metu vietinė mašina automatiškai veiks kaip mazgas, skirtas kitoms mašinoms traukti:

```bash
sync0 sub 10.0.0.1:9000 dataset ./local_mirror
```

### 4. Dvipusis kelių mašinų grupių sinchronizavimas

Keli įrenginiai prisijungia prie to paties grupės pavadinimo, o bet kurio mazgo katalogo pakeitimai bus automatiškai sinchronizuojami su kitais grupės mazgais:

```bash
sync0 group team-docs ./docs 10.0.0.1:9000,10.0.0.2:9000
```

### 5. Mazgo tapatybė ir asmeninė vizitinė kortelė

Kiekvienas mazgas palaiko vartotojo vardo, miesto, el. pašto, svetainės ir asmeninio parašo (daugiausia 256 simbolių) nustatymą ir perduoda vietinio privataus rakto parašo autentifikavimą:

```bash
# Peržiūrėkite vietinio viešojo rakto ID (nurodyti konfigūracijos katalogą, kad raktas liktų).
sync0 id -c ~/.config/sync0

# Generuokite ir rodykite mazgų vizitines korteles su asmeniniais parašais
sync0 profile -c ~/.config/sync0 \
  -u alice \
  --region SHA \
  --email alice@example.com \
  --url https://example.com \
  -b "Shanghai backup node"
```

`share`, `pull`, `sub`, `group` taip pat gali būti paleisti naudojant šiuos profilio parametrus:

```bash
sync0 share dataset ./data -c ~/.config/sync0 -u alice --region SHA -b "Shanghai backup node"
```

### 6. Kelių kalbų perjungimas

`sync0` Palaikomos 75 kalbos. Pagal numatytuosius nustatymus komandų eilutės žinynas ir raginimai atitinkama kalba rodomi automatiškai, atsižvelgiant į operacinės sistemos kalbos aplinką; jei sistemos kalbos nėra palaikomame sąraše, pagal numatytuosius nustatymus naudojama anglų kalba.

Norėdami rankiniu būdu nurodyti kalbą, nustatykite aplinkos kintamąjį `SYNC0_LANG`:

```bash
SYNC0_LANG=en sync0 --help
SYNC0_LANG=ja sync0 --help
SYNC0_LANG=de sync0 --help
```