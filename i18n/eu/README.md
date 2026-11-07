# sync0

`sync0` komando-lerroko tresna bat da fitxategiak sareetan zehar transferitzeko, direktorioak pixkanaka sinkronizatzeko eta aldi berean datuak hainbat makinatara banatzeko.

## Jatorrizko diseinuaren asmoa

Sare-atzerapenak eta pakete-galerak dituzten hainbat makina-gela, herrialde edo inguruneetan, `rsync` eta `scp` bezalako tresnak konexio-sekuentzia bakarreko berrespenean oinarritzen dira, eta horrek banda-zabaleraren erabilera mugatzen du; Hainbat makina aldi berean zerbitzari beretik datuak ateratzen dituztenean, iturburuko zerbitzariaren sare-txartela erraz bihur daiteke botila-lepoa.

`sync0`-ren helburua bi arazo konpontzea da:

1. Utzi bi makinen arteko fitxategien eta direktorioen sinkronizazioak banda-zabalera fisiko osoa exekutatzen, eta aldatutako zatia soilik transferitu fitxategia partzialki aldatzen denean.
2. Makina anitzek datu-blokeak elkarri transferi ditzakete direktorio bera sinkronizatzean, iturburuko zerbitzariaren karga murriztuz eta sinkronizazio-denbora orokorra laburtuz.

## Abiadura konparaketa rsync-ekin

Proba-ingurunea: Europako nodoa $\leftrightarrow$ Ipar Amerikako nodoa, joan-etorria 86 ms-ko atzerapena, direktorioen eskaneatzea, desbikoizketa konparatzea, sarearen transmisioa eta diskoaren kokapenaren egiaztapena barne hartzen dituen denbora osoa neurtuz:

| Transmisioaren eszenatokia | Datuen tamaina | `sync0` | `rsync` | anitz |
| :--- | :---: | :---: | :---: | :---: |
| 10.000 fitxategi txiki | 77.6 MB | 0.90 s (688 Mbps) | 4.86 s (128 Mbps) | 5.4x |
| Kode eta artxibo paketeak (`.tar`) | 512 MB | 1.94 s (2111 Mbps) | 9.36 s (438 Mbps) | 4.8x |
| Bideo-fitxategia (`.mp4`) | 512 MB | 7.36 s (556 Mbps) | 12.11 s (338 Mbps) | 1.6x |
| Sinkronizazio gehigarria (aldatu % 1eko edukia) | 512 MB | 0.12 s | 1.85 s | 15.4x |

## Errendimendua hobetzeko metodoak

- **UDP aldibereko transmisioa**: Bidali datu-blokeak UDP bidez, eta galdutako zatiak soilik birtransmititu pakete bakar batek ondorengo fitxategi-transferentziak blokeatzen ez ditzan.
- **Edukien segmentazioa eta desbikoizpena**: zatitu fitxategia datu-blokeetan edukiaren arabera eta kalkulatu egiaztapen-balioa. Dagoeneko hartzailean dagoen eduki bera zuzenean lokalean berrerabiltzen da sarea bikoiztu gabe.
- **Egokitu automatikoki konpresioa sarearen abiaduraren arabera**: Aktibatu konpresioa eduki konprimigarrietarako, hala nola testua eta kodea; automatikoki saltatu konpresioa bideoak, irudiak, konprimitutako paketeak edo abiadura handiko LAN pean, PUZaren kontsumoa murrizteko.
- **Inguruko nodo anitzeko tiraketa**: hainbat makina direktorio bera ateratzen dutenean, datu-blokeak aldi berean aterako dira hiri bereko deskargatutako nodoetatik edo latentzia txikiagoarekin.

## Instalatu

### 1. metodoa: instalatu zama bidez

```bash
cargo install sync0
```

### 2. metodoa: deskargatu aurrez konpilatutako bitarrak

Joan [GitHub Releases](https://github.com/webc-site/sync0/releases) atalera dagokion sistemarako `.tar.xz` pakete konprimitua deskargatzeko (Linux x86_64/ARM64, macOS Apple Silicon/Intel, Windows x64/ARM64, Android ARM64/x86_64, FreeBSD, NetBSD onartzen ditu). Deskonprimitu ondoren, jarri `sync0` `PATH` direktorioan:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Erabiltzailearen Gida

### 1. Zuzenean transferitu fitxategiak edo direktorioak bi makinen artean

Hasi monitorizatzen makina hartzailean eta zehaztu jasotzeko eta gordetzeko bidea:

```bash
sync0 recv 0.0.0.0:9000 ./backup
```

Bidali fitxategi bakar bat edo direktorio oso bat bidaltzen duen makinatik:

```bash
sync0 send ./project 192.168.1.10:9000
```

Gehien erabiltzen diren parametroak:

```bash
# Ezabatu hartzailean baina ez igorlean dauden fitxategi erredundanteak (ispilu-sinkronizazioa)
sync0 send ./project 192.168.1.10:9000 --delete

# Mugatu bidalketa-tasa (unitatea: byte/segundo, adibidez 50 MB/s)
sync0 send ./project 192.168.1.10:9000 --rate 50000000

# Modu isila (ez dago aurrerapen-barra erakusten)
sync0 send ./project 192.168.1.10:9000 -q
```

### 2. Direktorio partekatuak argitaratzea eta nodo anitzetatik ateratzea

Argitaratu direktorio partekatu bat (`dataset` izenekoa) jatorri zerbitzarian:

```bash
sync0 share dataset ./data --region SHA
```

Atera partekatutako direktorioa beste makinetan (hainbat nodo helbide bete ditzakezu aldi berean, komaz bereizita aldi berean ateratzeko):

```bash
sync0 pull 10.0.0.1:9000,10.0.0.2:9000 dataset ./local_data
```

Atera direktorio partekatuko azpidirektorio jakin bat bakarrik:

```bash
sync0 pull 10.0.0.1:9000 dataset ./local_data --subpath 2026/logs
```

Itzuli aldaketa lokalak urruneko direktorio partekatura:

```bash
sync0 push 10.0.0.1:9000 dataset ./local_data
```

### 3. Harpidetu katalogoaren eguneratze automatikoetara

Harpidetu iturburu-nodoaren direktorio partekatuan. Fitxategi bat iturburu-direktorioan gehitzen edo aldatzen denean, automatikoki sinkronizatuko da tokiko ordenagailuarekin. Aldi berean, tokiko makinak automatikoki nodo gisa balioko du beste makinek tira ditzaten:

```bash
sync0 sub 10.0.0.1:9000 dataset ./local_mirror
```

### 4. Makina anitzeko taldeen bi norabideko sinkronizazioa

Hainbat makina talde-izen berean sartzen dira, eta edozein nodo-direktorioko aldaketak automatikoki sinkronizatuko dira taldeko beste nodoekin:

```bash
sync0 group team-docs ./docs 10.0.0.1:9000,10.0.0.2:9000
```

### 5. Nodoaren identitatea eta bisita-txartel pertsonala

Nodo bakoitzak erabiltzailearen izena, hiria, posta elektronikoa, webgunea eta sinadura pertsonalizatua (gehienez 256 karaktere) ezartzea onartzen du, eta tokiko gako pribatuaren sinaduraren autentifikazioa pasatzen du:

```bash
# Ikusi tokiko gako publikoaren IDa (zehaztu gakoa mantentzeko konfigurazio-direktorioa)
sync0 id -c ~/.config/sync0

# Sortu eta bistaratu nodoen bisita-txartelak sinadura pertsonalizatuekin
sync0 profile -c ~/.config/sync0 \
  -u alice \
  --region SHA \
  --email alice@example.com \
  --url https://example.com \
  -b "Shanghai backup node"
```

`share`, `pull`, `sub`, `group` ere abiarazi daitezke profil-parametro hauekin:

```bash
sync0 share dataset ./data -c ~/.config/sync0 -u alice --region SHA -b "Shanghai backup node"
```

### 6. Hizkuntza anitzeko aldaketa

`sync0` 75 hizkuntza onartzen ditu. Lehenespenez, komando-lerroko laguntza eta dagokion hizkuntzan automatikoki bistaratuko dira sistema eragilearen hizkuntza-ingurunearen arabera; sistemaren hizkuntza onartzen ez bada zerrendan, ingelesa erabiltzen da lehenespenez.

Hizkuntza eskuz zehazteko, ezarri ingurune-aldagaia `SYNC0_LANG`:

```bash
SYNC0_LANG=en sync0 --help
SYNC0_LANG=ja sync0 --help
SYNC0_LANG=de sync0 --help
```