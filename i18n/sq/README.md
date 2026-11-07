# sync0

`sync0` është një mjet i linjës së komandës për transferimin e skedarëve nëpër rrjete, për sinkronizimin në rritje të drejtorive dhe për shpërndarjen e njëkohshme të të dhënave në makina të shumta.

## Synimi origjinal i dizajnit

Në dhomat, vendet ose mjediset ndër-makinare me vonesa në rrjet dhe humbje të paketave, mjete të tilla si `rsync` dhe `scp` bazohen në konfirmimin e një sekuence të vetme lidhjeje, e cila kufizon përdorimin e gjerësisë së brezit; kur makina të shumta tërheqin të dhëna nga i njëjti server në të njëjtën kohë, karta e rrjetit të serverit burim mund të bëhet lehtësisht një pengesë.

Qëllimi i `sync0` është të zgjidhë dy probleme:

1. Lëreni sinkronizimin e skedarit dhe drejtorisë midis dy makinave të funksionojë me gjerësi të plotë fizike dhe transferoni pjesën e ndryshuar vetëm kur skedari është modifikuar pjesërisht.
2. Makinat e shumta mund të transferojnë blloqe të dhënash tek njëri-tjetri kur sinkronizojnë të njëjtin drejtori, duke reduktuar ngarkesën në serverin burimor dhe duke shkurtuar kohën e përgjithshme të sinkronizimit.

## Krahasimi i shpejtësisë me rsync

Mjedisi i testimit: Nyja evropiane $\leftright-arrow$ Nyja e Amerikës së Veriut, vonesë vajtje-ardhje 86 ms, duke matur kohën e plotë, duke përfshirë skanimin e direktoriumit, krahasimin e dyfishimit, transmetimin e rrjetit dhe verifikimin e vendosjes së diskut:

| Skenari i transmetimit | Madhësia e të dhënave | `sync0` | `rsync` | të shumëfishta |
| :--- | :---: | :---: | :---: | :---: |
| 10,000 skedarë të vegjël | 77.6 MB | 0.90 s (688 Mbps) | 4.86 s (128 Mbps) | 5.4x |
| Paketat e kodit dhe arkivave (`.tar`) | 512 MB | 1.94 s (2111 Mbps) | 9.36 s (438 Mbps) | 4.8x |
| Skedar video (`.mp4`) | 512 MB | 7.36 s (556 Mbps) | 12.11 s (338 Mbps) | 1.6x |
| Sinkronizimi në rritje (modifiko përmbajtjen 1%) | 512 MB | 0.12 s | 1.85 s | 15.4x |

## Metodat e përmirësimit të performancës

- **Transmetimi i njëkohshëm i UDP**: Dërgoni blloqe të dhënash përmes UDP dhe ritransmetoni vetëm fragmente të humbura për të shmangur një humbje të vetme pakete që bllokon transferimet e mëvonshme të skedarëve.
- **Segmentimi i përmbajtjes dhe deduplikimi**: Ndani skedarin në blloqe të dhënash sipas përmbajtjes dhe llogaritni vlerën e kontrollit. E njëjta përmbajtje tashmë në fundin marrës ripërdoret drejtpërdrejt në nivel lokal pa dublikuar rrjetin.
- **Rregulloni automatikisht kompresimin sipas shpejtësisë së rrjetit**: Aktivizoni ngjeshjen për përmbajtje të kompresueshme si teksti dhe kodi; kapërce automatikisht ngjeshjen për video, foto, paketa të kompresuara ose nën LAN me shpejtësi të lartë për të reduktuar konsumin e CPU-së.
- **Tërheqja me shumë nyje në afërsi**: Kur makina të shumta tërheqin të njëjtin drejtori, blloqet e të dhënave do të tërhiqen njëkohësisht nga nyjet e shkarkuara në të njëjtin qytet ose me vonesë më të ulët.

## Instaloni

### Metoda 1: Instaloni përmes ngarkesave

```bash
cargo install sync0
```

### Metoda 2: Shkarkoni binarët e parakompiluar

Shkoni te [GitHub Releases](https://github.com/webc-site/sync0/releases) për të shkarkuar paketën e ngjeshur `.tar.xz` për sistemin përkatës (mbështet Linux x86_64/ARM64, macOS Apple Silicon/Intel, Windows x64/ARM64, Android ARM64/x86_64, FreeBSD, NetBSD). Pas shkyçjes, vendoseni `sync0` në drejtorinë `PATH`:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Udhëzues përdorimi

### 1. Transferoni direkt skedarë ose direktori midis dy makinave

Filloni monitorimin në makinën marrëse dhe specifikoni rrugën e marrjes dhe ruajtjes:

```bash
sync0 recv 0.0.0.0:9000 ./backup
```

Dërgoni një skedar të vetëm ose një drejtori të tërë nga makina dërguese:

```bash
sync0 send ./project 192.168.1.10:9000
```

Parametrat e përdorur zakonisht:

```bash
# Fshi skedarët e tepërt që ekzistojnë në fundin marrës, por jo në fundin e dërgimit (sinkronizimi i pasqyrës)
sync0 send ./project 192.168.1.10:9000 --delete

# Kufizoni shpejtësinë e dërgimit (njësi: bajt/sekondë, për shembull 50 MB/s)
sync0 send ./project 192.168.1.10:9000 --rate 50000000

# Modaliteti i heshtur (nuk shfaqet shiriti i përparimit)
sync0 send ./project 192.168.1.10:9000 -q
```

### 2. Publikimi i drejtorive të përbashkëta dhe tërheqja nga nyjet e shumta

Publikoni një drejtori të përbashkët (me emrin `dataset`) në serverin e origjinës:

```bash
sync0 share dataset ./data --region SHA
```

Tërhiqeni drejtorinë e përbashkët në makina të tjera (mund të plotësoni adresa të shumta nyje në të njëjtën kohë, të ndara me presje për t'u tërhequr njëkohësisht):

```bash
sync0 pull 10.0.0.1:9000,10.0.0.2:9000 dataset ./local_data
```

Tërhiqni vetëm një nëndrejtori të caktuar në drejtorinë e përbashkët:

```bash
sync0 pull 10.0.0.1:9000 dataset ./local_data --subpath 2026/logs
```

Shtyni ndryshimet lokale përsëri në drejtorinë e përbashkët të largët:

```bash
sync0 push 10.0.0.1:9000 dataset ./local_data
```

### 3. Regjistrohu për përditësimet automatike të katalogut

Abonohu ​​në drejtorinë e përbashkët të nyjës burimore. Kur një skedar shtohet ose modifikohet në drejtorinë burimore, ai do të sinkronizohet automatikisht me kompjuterin lokal. Në të njëjtën kohë, makina lokale do të shërbejë automatikisht si një nyje për makineritë e tjera për t'u tërhequr:

```bash
sync0 sub 10.0.0.1:9000 dataset ./local_mirror
```

### 4. Sinkronizimi i dyanshëm i grupeve me shumë makina

Makinat e shumta bashkohen me të njëjtin emër grupi dhe ndryshimet në çdo drejtori të nyjeve do të sinkronizohen automatikisht me nyjet e tjera në grup:

```bash
sync0 group team-docs ./docs 10.0.0.1:9000,10.0.0.2:9000
```

### 5. Identiteti i nyjës dhe karta e biznesit personal

Çdo nyje mbështet vendosjen e emrit të përdoruesit, qytetit, emailit, faqes së internetit dhe një nënshkrimi të personalizuar (maksimumi 256 karaktere) dhe kalon vërtetimin e nënshkrimit të çelësit privat lokal:

```bash
# Shikoni ID-në e çelësit publik lokal (specifikoni drejtorinë e konfigurimit për të vazhduar çelësin)
sync0 id -c ~/.config/sync0

# Gjeneroni dhe shfaqni kartat e biznesit të nyjeve me nënshkrime të personalizuara
sync0 profile -c ~/.config/sync0 \
  -u alice \
  --region SHA \
  --email alice@example.com \
  --url https://example.com \
  -b "Shanghai backup node"
```

`share`, `pull`, `sub`, `group` gjithashtu mund të hapen me këto parametra të profilit:

```bash
sync0 share dataset ./data -c ~/.config/sync0 -u alice --region SHA -b "Shanghai backup node"
```

### 6. Ndërrimi në shumë gjuhë

`sync0` Mbështet 75 gjuhë. Si parazgjedhje, ndihma dhe kërkesat e linjës së komandës në gjuhën përkatëse shfaqen automatikisht sipas mjedisit gjuhësor të sistemit operativ; nëse gjuha e sistemit nuk është në listën e mbështetur, anglishtja përdoret si parazgjedhje.

Për të specifikuar gjuhën me dorë, vendosni variablin e mjedisit `SYNC0_LANG`:

```bash
SYNC0_LANG=en sync0 --help
SYNC0_LANG=ja sync0 --help
SYNC0_LANG=de sync0 --help
```