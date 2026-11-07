# sync0

Offeryn llinell orchymyn yw `sync0` ar gyfer trosglwyddo ffeiliau ar draws rhwydweithiau, cysoni cyfeiriaduron yn gynyddol, a dosbarthu data ar yr un pryd i beiriannau lluosog.

## Bwriad dylunio gwreiddiol

Mewn ystafelloedd traws-beiriant, gwledydd, neu amgylcheddau gydag oedi rhwydwaith a cholli pecynnau, mae offer megis `rsync` a `scp` yn seiliedig ar gadarnhad dilyniant cysylltiad sengl, sy'n cyfyngu ar y defnydd o led band; pan fydd peiriannau lluosog yn tynnu data o'r un gweinydd ar yr un pryd, gall cerdyn rhwydwaith y gweinydd ffynhonnell ddod yn dagfa yn hawdd.

Nod `sync0` yw datrys dwy broblem:

1. Gadewch i'r cydamseriad ffeil a chyfeiriadur rhwng y ddau beiriant redeg lled band corfforol llawn, a dim ond trosglwyddo'r rhan sydd wedi'i newid pan fydd y ffeil wedi'i haddasu'n rhannol.
2. Gall peiriannau lluosog drosglwyddo blociau data i'w gilydd wrth gydamseru'r un cyfeiriadur, gan leihau'r llwyth ar y gweinydd ffynhonnell a byrhau'r amser cydamseru cyffredinol.

## Cymhariaeth cyflymder â rsync

Amgylchedd prawf: nôd Ewropeaidd $\leftrightarrow$ nod Gogledd America, oedi taith gron 86 ms, mesur yr amser cyflawn gan gynnwys sganio cyfeiriadur, cymharu dad-ddyblygiad, trawsyrru rhwydwaith a dilysu lleoliad disg:

| Senario trosglwyddo | Maint data | `sync0` | `rsync` | lluosog |
| :--- | :---: | :---: | :---: | :---: |
| 10,000 o ffeiliau bach | 77.6 MB | 0.90 s (688 Mbps) | 4.86 s (128 Mbps) | 5.4x |
| Pecynnau cod ac archif (`.tar`) | 512 MB | 1.94 s (2111 Mbps) | 9.36 s (438 Mbps) | 4.8x |
| Ffeil fideo (`.mp4`) | 512 MB | 7.36 s (556 Mbps) | 12.11 s (338 Mbps) | 1.6x |
| Cydamseru cynyddrannol (addasu cynnwys 1%) | 512 MB | 0.12 s | 1.85 s | 15.4x |

## Dulliau gwella perfformiad

- **Trosglwyddiad cydamserol CDU**: Anfonwch flociau data trwy'r CDU, a dim ond darnau coll a ail-drosglwyddwch er mwyn osgoi colled pecyn unigol sy'n rhwystro trosglwyddiadau ffeiliau dilynol.
- **Segmentu cynnwys a dad-ddyblygu**: Rhannwch y ffeil yn flociau data yn ôl y cynnwys a chyfrifwch y gwerth siec. Mae'r un cynnwys sydd eisoes ar y pen derbyn yn cael ei ailddefnyddio'n uniongyrchol yn lleol heb ddyblygu'r rhwydwaith.
- ** Addasu cywasgu yn awtomatig yn ôl cyflymder rhwydwaith**: Trowch y cywasgu ymlaen ar gyfer cynnwys cywasgadwy fel testun a chod; hepgor cywasgu yn awtomatig ar gyfer fideos, lluniau, pecynnau cywasgedig neu o dan LAN cyflym i leihau'r defnydd o CPU.
- ** Tynnu aml-nodyn gerllaw **: Pan fydd peiriannau lluosog yn tynnu'r un cyfeiriadur, bydd blociau data yn cael eu tynnu ar yr un pryd o nodau wedi'u llwytho i lawr yn yr un ddinas neu gyda hwyrni is.

## Gosod

### Dull 1: Gosod trwy Cargo

```bash
cargo install sync0
```

### Dull 2: Lawrlwythwch deuaidd sydd wedi'u llunio ymlaen llaw

Ewch i [GitHub Releases] (/0) i lawrlwytho'r pecyn `.tar.xz` cywasgedig ar gyfer y system gyfatebol (yn cefnogi Linux x86_64/ARM64, macOS Apple Silicon/Intel, Windows x64/ARM64, Android ARM64/x86_64, FreeBSD, NetBSD). Ar ôl dadsipio, rhowch `sync0` yn y cyfeiriadur `PATH`:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Canllaw Defnyddiwr

### 1. Trosglwyddo ffeiliau neu gyfeiriaduron yn uniongyrchol rhwng dau beiriant

Dechreuwch fonitro ar y peiriant derbyn a nodwch y llwybr derbyn ac arbed:

```bash
sync0 recv 0.0.0.0:9000 ./backup
```

Anfonwch un ffeil neu gyfeiriadur cyfan o'r peiriant anfon:

```bash
sync0 send ./project 192.168.1.10:9000
```

Paramedrau a ddefnyddir yn gyffredin:

```bash
# Dileu ffeiliau diangen sy'n bodoli ar y pen derbyn ond nid ar y diwedd anfon (cydamseru drych)
sync0 send ./project 192.168.1.10:9000 --delete

# Cyfradd anfon terfyn (uned: beit/eiliad, er enghraifft 50 MB/s)
sync0 send ./project 192.168.1.10:9000 --rate 50000000

# Modd tawel (dim bar cynnydd wedi'i ddangos)
sync0 send ./project 192.168.1.10:9000 -q
```

### 2. Cyhoeddi cyfeiriaduron a rennir a thynnu o nodau lluosog

Cyhoeddi cyfeiriadur a rennir (o'r enw `dataset`) ar y gweinydd tarddiad:

```bash
sync0 share dataset ./data --region SHA
```

Tynnwch y cyfeiriadur a rennir ar beiriannau eraill (gallwch lenwi cyfeiriadau nodau lluosog ar yr un pryd, wedi'u gwahanu gan atalnodau i'w tynnu ar yr un pryd):

```bash
sync0 pull 10.0.0.1:9000,10.0.0.2:9000 dataset ./local_data
```

Tynnwch is-gyfeiriadur penodol yn unig yn y cyfeiriadur a rennir:

```bash
sync0 pull 10.0.0.1:9000 dataset ./local_data --subpath 2026/logs
```

Gwthiwch newidiadau lleol yn ôl i'r cyfeiriadur a rennir o bell:

```bash
sync0 push 10.0.0.1:9000 dataset ./local_data
```

### 3. Tanysgrifiwch i gatalogio diweddariadau awtomatig

Tanysgrifiwch i'r cyfeiriadur a rennir o'r nod ffynhonnell. Pan fydd ffeil yn cael ei hychwanegu neu ei haddasu yn y cyfeiriadur ffynhonnell, bydd yn cael ei gysoni yn awtomatig i'r cyfrifiadur lleol. Ar yr un pryd, bydd y peiriant lleol yn gweithredu'n awtomatig fel nod i beiriannau eraill dynnu:

```bash
sync0 sub 10.0.0.1:9000 dataset ./local_mirror
```

### 4. Cydamseru dwy ffordd o grwpiau aml-beiriant

Mae peiriannau lluosog yn ymuno â'r un enw grŵp, a bydd newidiadau mewn unrhyw gyfeiriadur nodau yn cael eu cysoni'n awtomatig â nodau eraill yn y grŵp:

```bash
sync0 group team-docs ./docs 10.0.0.1:9000,10.0.0.2:9000
```

### 5. Adnabod nod a cherdyn busnes personol

Mae pob nod yn cefnogi gosod enw defnyddiwr, dinas, e-bost, gwefan a llofnod personol (uchafswm o 256 nod), ac yn pasio'r dilysiad llofnod allwedd preifat lleol:

```bash
# Gweld yr ID allwedd cyhoeddus lleol (nodwch y cyfeiriadur ffurfweddu i barhau â'r allwedd)
sync0 id -c ~/.config/sync0

# Cynhyrchu ac arddangos cardiau busnes nodau gyda llofnodion personol
sync0 profile -c ~/.config/sync0 \
  -u alice \
  --region SHA \
  --email alice@example.com \
  --url https://example.com \
  -b "Shanghai backup node"
```

Gellir lansio `share`, `pull`, `sub`, `group` hefyd gyda'r paramedrau proffil hyn:

```bash
sync0 share dataset ./data -c ~/.config/sync0 -u alice --region SHA -b "Shanghai backup node"
```

### 6. Newid aml-iaith

`sync0` Yn cefnogi 75 o ieithoedd. Yn ddiofyn, mae cymorth llinell orchymyn ac awgrymiadau yn yr iaith gyfatebol yn cael eu harddangos yn awtomatig yn unol ag amgylchedd iaith y system weithredu; os nad yw iaith y system yn y rhestr a gefnogir, defnyddir Saesneg yn ddiofyn.

I nodi'r iaith â llaw, gosodwch y newidyn amgylchedd `SYNC0_LANG`:

```bash
SYNC0_LANG=en sync0 --help
SYNC0_LANG=ja sync0 --help
SYNC0_LANG=de sync0 --help
```