# sync0

`sync0` je orodje ukazne vrstice za prenos datotek po omrežjih, postopno sinhronizacijo imenikov in hkratno distribucijo podatkov na več strojev.

## Izvirni načrt oblikovanja

V sobah z različnimi stroji, državah ali okoljih z omrežnimi zamudami in izgubo paketov orodja, kot sta `rsync` in `scp`, temeljijo na potrditvi zaporedja ene same povezave, kar omejuje uporabo pasovne širine; ko več strojev hkrati črpa podatke iz istega strežnika, lahko omrežna kartica izvornega strežnika zlahka postane ozko grlo.

Cilj `sync0` je rešiti dva problema:

1. Naj sinhronizacija datotek in imenikov med obema napravama poteka s polno fizično pasovno širino in prenesite spremenjeni del le, ko je datoteka delno spremenjena.
2. Več strojev lahko med sinhronizacijo istega imenika med seboj prenaša podatkovne bloke, kar zmanjša obremenitev izvornega strežnika in skrajša celoten čas sinhronizacije.

## Primerjava hitrosti z rsync

Testno okolje: Evropsko vozlišče $\leftrightarrow$ Severnoameriško vozlišče, povratna zakasnitev 86 ms, merjenje celotnega časa, vključno s skeniranjem imenika, primerjavo deduplikacije, omrežnim prenosom in preverjanjem postavitve diska:

| Scenarij prenosa | Velikost podatkov | `sync0` | `rsync` | večkratno |
| :--- | :---: | :---: | :---: | :---: |
| 10.000 majhnih datotek | 77.6 MB | 0.90 s (688 Mbps) | 4.86 s (128 Mbps) | 5.4x |
| Kodni in arhivski paketi (`.tar`) | 512 MB | 1.94 s (2111 Mbps) | 9.36 s (438 Mbps) | 4.8x |
| Video datoteka (`.mp4`) | 512 MB | 7.36 s (556 Mbps) | 12.11 s (338 Mbps) | 1.6x |
| Prirastna sinhronizacija (spremeni 1 % vsebine) | 512 MB | 0.12 s | 1.85 s | 15.4x |

## Metode izboljšanja učinkovitosti

- **Hkratni prenos UDP**: Pošljite podatkovne bloke prek UDP in ponovno pošljite samo izgubljene fragmente, da preprečite, da bi izguba posameznega paketa blokirala nadaljnje prenose datotek.
- **Segmentacija vsebine in deduplikacija**: Razdelite datoteko v podatkovne bloke glede na vsebino in izračunajte kontrolno vrednost. Ista vsebina, ki je že na prejemnem koncu, se ponovno uporabi neposredno lokalno brez podvajanja omrežja.
- **Samodejno prilagodi stiskanje glede na hitrost omrežja**: Vklopite stiskanje za vsebino, ki jo je mogoče stisniti, kot sta besedilo in koda; samodejno preskoči stiskanje za videoposnetke, slike, stisnjene pakete ali pod hitrim LAN-om, da zmanjša porabo procesorja.
- **Vlečenje več vozlišč v bližini**: Ko več strojev vleče isti imenik, bodo podatkovni bloki potegnjeni sočasno iz prenesenih vozlišč v istem mestu ali z nižjo zakasnitvijo.

## Namestite

### 1. način: namestitev prek Cargo

```bash
cargo install sync0
```

### 2. način: Prenesite vnaprej prevedene binarne datoteke

Pojdite na [GitHub Releases](https://github.com/webc-site/sync0/releases), da prenesete `.tar.xz` stisnjen paket za ustrezen sistem (podpira Linux x86_64/ARM64, macOS Apple Silicon/Intel, Windows x64/ARM64, Android ARM64/x86_64, FreeBSD, NetBSD). Po razpakiranju postavite `sync0` v imenik `PATH`:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Uporabniški priročnik

### 1. Neposredno prenesite datoteke ali imenike med dvema napravama

Zaženite nadzor na sprejemni napravi in ​​določite pot sprejema in shranjevanja:

```bash
sync0 recv 0.0.0.0:9000 ./backup
```

Pošljite posamezno datoteko ali celoten imenik iz naprave za pošiljanje:

```bash
sync0 send ./project 192.168.1.10:9000
```

Pogosto uporabljeni parametri:

```bash
# Izbrišite odvečne datoteke, ki obstajajo na prejemnem koncu, ne pa tudi na pošiljatelju (zrcalna sinhronizacija)
sync0 send ./project 192.168.1.10:9000 --delete

# Omejite hitrost pošiljanja (enota: bajti/sekundo, na primer 50 MB/s)
sync0 send ./project 192.168.1.10:9000 --rate 50000000

# Tihi način (ni prikazane vrstice napredka)
sync0 send ./project 192.168.1.10:9000 -q
```

### 2. Objavljanje skupnih imenikov in vlečenje iz več vozlišč

Objavite imenik v skupni rabi (z imenom `dataset`) na izvornem strežniku:

```bash
sync0 share dataset ./data --region SHA
```

Povlecite imenik v skupni rabi na drugih napravah (hkrati lahko izpolnite več naslovov vozlišč, ločenih z vejicami za sočasno vlečenje):

```bash
sync0 pull 10.0.0.1:9000,10.0.0.2:9000 dataset ./local_data
```

Potegnite samo določen podimenik v imeniku v skupni rabi:

```bash
sync0 pull 10.0.0.1:9000 dataset ./local_data --subpath 2026/logs
```

Potisnite lokalne spremembe nazaj v oddaljeni imenik v skupni rabi:

```bash
sync0 push 10.0.0.1:9000 dataset ./local_data
```

### 3. Naročite se na samodejne posodobitve kataloga

Naročite se na skupni imenik izvornega vozlišča. Ko je datoteka dodana ali spremenjena v izvornem imeniku, bo samodejno sinhronizirana z lokalnim računalnikom. Istočasno bo lokalni stroj samodejno služil kot vozlišče za druge stroje, ki bodo vlekli:

```bash
sync0 sub 10.0.0.1:9000 dataset ./local_mirror
```

### 4. Dvosmerna sinhronizacija večstrojnih skupin

Več strojev se pridruži istemu imenu skupine in spremembe v katerem koli imeniku vozlišč bodo samodejno sinhronizirane z drugimi vozlišči v skupini:

```bash
sync0 group team-docs ./docs 10.0.0.1:9000,10.0.0.2:9000
```

### 5. Identiteta vozlišča in osebna vizitka

Vsako vozlišče podpira nastavitev uporabniškega imena, mesta, e-pošte, spletnega mesta in osebnega podpisa (največ 256 znakov) ter opravi preverjanje pristnosti podpisa lokalnega zasebnega ključa:

```bash
# Ogled ID-ja lokalnega javnega ključa (določite konfiguracijski imenik, da ohranite ključ)
sync0 id -c ~/.config/sync0

# Ustvarite in prikažite vizitke vozlišča s prilagojenimi podpisi
sync0 profile -c ~/.config/sync0 \
  -u alice \
  --region SHA \
  --email alice@example.com \
  --url https://example.com \
  -b "Shanghai backup node"
```

`share`, `pull`, `sub`, `group` je mogoče zagnati tudi s temi parametri profila:

```bash
sync0 share dataset ./data -c ~/.config/sync0 -u alice --region SHA -b "Shanghai backup node"
```

### 6. Preklapljanje med več jeziki

`sync0` Podpira 75 jezikov. Privzeto se pomoč in pozivi v ukazni vrstici v ustreznem jeziku samodejno prikažejo glede na jezikovno okolje operacijskega sistema; če sistemskega jezika ni na podprtem seznamu, se privzeto uporablja angleščina.

Če želite jezik določiti ročno, nastavite spremenljivko okolja `SYNC0_LANG`:

```bash
SYNC0_LANG=en sync0 --help
SYNC0_LANG=ja sync0 --help
SYNC0_LANG=de sync0 --help
```