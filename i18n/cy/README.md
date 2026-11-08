# sync0

Mae `sync0` yn declyn rhannu ffeiliau rhwng cymheiriaid (P2P) a chydamseru aml-beiriant sy'n cystadlu â Resilio Sync.

Mae pob peiriant yn weinydd ac yn gleient: gall unrhyw nod gyhoeddi sawl grŵp rhannu ffeiliau. Ar ôl i nodau eraill ymuno, gallant gael nodau had sydd ar gael yn awtomatig o'r gweinydd cymheiriaid a'u tynnu ar yr un pryd o sawl ffynhonnell. Ar ôl cwblhau cydamseru, bydd hadau'n cael eu creu'n awtomatig ar gyfer mwy o nodau i gyflymu lawrlwythiadau.

## Uchafbwyntiau Craidd

- **Mae pawb yn weinydd ac yn gleient**: Yn debyg i Resilio Sync, dim dibyniaeth ar weinydd canolog. Gall unrhyw nod greu grwpiau rhannu ffeiliau lluosog, a gall hefyd ymuno â grwpiau rhannu pobl eraill; mae nodau'n cyfnewid yn awtomatig ac yn cael nodau hadau sydd ar gael i gyflawni tynnu cydamserol a chydamseru amser real o beiriannau lluosog.
- **Cyfeiriad nod di-IP**: Yn ddiofyn, mae'r allwedd nod a'r cyfeiriad `base64url` sydd wedi'i amgodio `iroh` yn cael eu cynhyrchu'n awtomatig yn `~/.config/sync0` heb fod angen cofio na datgelu'r IP cyhoeddus.
- ** Cydamseru aml-ddethol yn ôl y galw**: Yn cefnogi cydamseru'r grŵp rhannu cyfan, a hefyd yn cefnogi cydamseru yn ôl y galw trwy ddewis ffeiliau lluosog neu is-gyfeiriaduron ar yr un pryd.
- **Caniatadau hierarchaidd aelodau grŵp a rheolaeth cyfeiriadur lanlwytho**: Yn cefnogi caniatadau darllen-yn-unig (`ro`), uwchlwytho darllen-ysgrifennu (`rw`), a rheolaeth (`admin`), a gall nodi'n unigol y rhagddodiad llwybr y caniateir i bob aelod ei uwchlwytho (yn ddiofyn, caniateir uwchlwytho i gyfeiriadur gwraidd y gyfran).
- ** Cydamseru cyflym a chynyddrannol ar draws rhwydweithiau**: Trosglwyddo ffeiliau mawr ar yr un pryd, cydgrynhoi ffeiliau bach enfawr yn y cof, a dim ond blociau data wedi'u cydamseru pan fydd ffeiliau'n cael eu haddasu'n rhannol.

## Cymhariaeth cyflymder â rsync

### Mesuriad gwirioneddol ffynhonnell sengl rhwydwaith transoceanic (Tsieina → Unol Daleithiau, oedi 260 ms)

| Senario trosglwyddo | Maint data | model | `sync0` | `rsync -az` | Cynyddu lluosydd |
| :--- | :---: | :---: | :---: | :---: | :---: |
| 500 o ffeiliau bach | 1.91 MB | Cydamseru llawn | 18.84 Mbps | 5.32 Mbps | 3.5x |
| 500 o ffeiliau bach | 1.91 MB | Cydamseru cynyddrannol | 27.15 Mbps | 3.83 Mbps | 7.1x |
| Pecynnau cod ac archif (`.tar`) | 10.0 MB | Cydamseru llawn | 139.70 Mbps | 24.97 Mbps | 5.6x |
| Pecynnau cod ac archif (`.tar`) | 10.0 MB | Cydamseru cynyddrannol | 83.38 Mbps | 26.02 Mbps | 3.2x |
| Ffeil fideo (`.mp4`) | 15.0 MB | Cydamseru llawn | 54.06 Mbps | 27.04 Mbps | 2.0x |
| Ffeil fideo (`.mp4`) | 15.0 MB | Cydamseru cynyddrannol | 216.83 Mbps | 21.51 Mbps | 10.1x |

### Prawf tynnu aml-nôd rhwydwaith transoceanic P2P (Tsieina → nodau deuol Unol Daleithiau, oedi 260 ms)

Cymhariaeth fesuredig o dynnu streipiog nod hadau aml-ffynhonnell:

| Senario trosglwyddo | Maint data | Hadu nod sengl | Hadu cydamserol P2P dau nôd | Cynyddu lluosydd |
| :--- | :---: | :---: | :---: | :---: |
| 500 o ffeiliau bach | 1.97 MB | 5.28 Mbps | 7.87 Mbps | 1.5x |
| Pecynnau cod ac archif (`.tar`) | 30.0 MB | 56.35 Mbps | 83.17 Mbps | 1.5x |
| Ffeil fideo (`.mp4`) | 30.0 MB | 27.00 Mbps | 39.46 Mbps | 1.5x |

### Gwir fesur llinell bwrpasol cyflymder uchel traws-gyfandirol (Ewrop → Gogledd America, oedi 86 ms)

| Senario trosglwyddo | Maint data | `sync0` | `rsync` | Cynyddu lluosydd |
| :--- | :---: | :---: | :---: | :---: |
| 10,000 o ffeiliau bach | 77.6 MB | 688 Mbps | 128 Mbps | 5.4x |
| Pecynnau cod ac archif (`.tar`) | 512 MB | 2111 Mbps | 438 Mbps | 4.8x |
| Ffeil fideo (`.mp4`) | 512 MB | 556 Mbps | 338 Mbps | 1.6x |
| Cydamseru cynyddrannol (addasu cynnwys 1%) | 512 MB | 34133 Mbps | 2214 Mbps | 15.4x |

## Gosod

### Dull 1: Gosod trwy Cargo

```bash
cargo install sync0
```

### Dull 2: Lawrlwythwch deuaidd sydd wedi'u llunio ymlaen llaw

Ewch i [GitHub Releases] (/0) i lawrlwytho'r pecyn `.tar.xz` cywasgedig ar gyfer y system gyfatebol (yn cefnogi Linux, macOS, Windows, Android, FreeBSD, NetBSD), dadsipiwch a rhowch ef yn y cyfeiriadur `PATH`:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Dechreuwch yn gyflym

Mae `sync0` yn defnyddio `~/.config/sync0` fel y cyfeiriadur cyfluniad yn ddiofyn (os oes angen i chi newid i gyfeiriaduron eraill, gallwch ychwanegu `-c <directory>`).

### 1. Gweld y cyfeiriad nod lleol a gosod cerdyn busnes

```bash
# Gweld cyfeiriad y nod lleol (cynhyrchir yr allwedd yn awtomatig yn ~/.config/sync0 am y tro cyntaf)
sync0 id

# Cynhyrchu cerdyn busnes nod personol (yn cefnogi enw defnyddiwr, dinas, e-bost, gwefan a llofnod o fewn 256 o eiriau)
sync0 profile -u alice --region SHA --email alice@example.com --url https://example.com -b "Shanghai node"
```

### 2. Cyhoeddi sawl grŵp rhannu ffeiliau a gosod hawliau aelodau grŵp (`.nt` Ffurfweddiad)

Mae `sync0` wedi'i ffurfweddu yn y fformat [NestedText](https://nestedtext.org)(`.nt`). Mae gan bob defnyddiwr `.nt` ffeil ffurfweddu ar wahân. Gallwch nodi'r cyfeiriadur proffil defnyddiwr trwy `users_dir` yn `config.nt` (sy'n cynnwys pob `.nt` ffeil yn y cyfeiriadur hwn yn awtomatig). Mae'r defnyddiwr a'r rhagddodiad llwybr y caniateir eu llwytho i fyny yn defnyddio rhestr heb ei threfnu (`-`). **Pan nad yw rhagddodiad y llwybr llwytho i fyny wedi'i lenwi, caniateir uwchlwytho i gyfeiriadur gwraidd y gyfran yn ddiofyn**.

1. Creu `.nt` ffeiliau ffurfweddu ar wahân ar gyfer pob aelod yn y cyfeiriadur `~/.config/sync0/users/`:

`~/.config/sync0/users/alice.nt` (aelod darllen yn unig, cyfradd gyfyngedig `80MB`, `rate` cefnogi `GB`, `MB`, `KB` unedau):

```yaml
id: 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c
perm: ro
rate: 80MB
```

`~/.config/sync0/users/bob.nt` (aelodau llwytho i fyny cyfyngedig: caniateir uwchlwytho i `uploads/bob` a `shared/common` cyfeiriaduron yn unig):

```yaml
id: 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f
perm: rw
write_prefix:
  - uploads/bob
  - shared/common
```

`~/.config/sync0/users/charlie.nt` (Llwythiad aelod diofyn: Pan fydd `write_prefix` wedi'i hepgor, caniateir uwchlwytho i gyfeiriadur gwraidd y gyfran yn ddiofyn):

```yaml
id: 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
perm: rw
```

2. Ffurfweddwch y cyfeiriadur defnyddwyr a'r grwpiau rhannu ffeiliau lluosog yn `~/.config/sync0/config.nt`:

```yaml
users_dir: ./users

groups:
  dataset:
    path: ./data
    auth: false
    perm: ro
    rate: 50MB

  models:
    path: ./models
    auth: false
    perm: ro
    rate: 1GB

  team-vault:
    path: ./vault
    auth: true
    perm: ro
    rate: 512KB
    users:
      - alice
      - bob
      - charlie
```

Dechrau rhannu gwasanaeth:

```bash
# Cychwyn uniongyrchol (yn llwytho'r holl ffurfweddiadau defnyddiwr .nt yn awtomatig mewn cyfeiriaduron ~/.config/sync0/config.nt a users_dir)
sync0 share --region SHA

# Neu cyhoeddwch grwpiau rhannu lluosog yn gyflym ac awdurdodwch aelodau'r grŵp yn uniongyrchol trwy'r llinell orchymyn
sync0 share dataset:./data team-vault:./vault \
  --auth \
  --allow-ro 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c \
  --allow-rw 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f:uploads/bob,shared/common \
  --allow-rw 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
```

### 3. Tynnwch yr holl gynnwys neu dewiswch ffeiliau a chyfeiriaduron lluosog (`sync0 pull`)

Cysylltwch ag unrhyw nod cyfoed i gael yr holl nodau had sydd ar gael yn awtomatig yn y grŵp rhannu, a thynnu o nodau hadau lluosog ar yr un pryd:

```bash
# Tynnwch y grŵp rhannu set ddata cyfan i'r data ./local_data
sync0 tynnu set ddata:// ./local_data -p <nod cyfeiriad>

# Cydamseru aml-ddewis: tynnwch y ffeil sengl docs/readme.md ac asedau/eiconau is-gyfeiriaduron, src/core ar yr un pryd
set ddata tynnu sync0://docs/readme.md,assets/icons,src/core ./local_data -p <nod cyfeiriad>
```

### 4. Uwchlwythwch ffeil neu gyfeiriadur i grŵp rhannu (`sync0 push`)

Gall aelodau sydd â chaniatâd uwchlwytho (`rw` neu `admin`) uwchlwytho ffeiliau neu gyfeiriaduron lleol i grwpiau rhannu o bell:

```bash
# Aelodau sydd â chaniatâd uwchlwytho cyfeiriadur gwraidd: Llwythwch yn uniongyrchol i'r cyfeiriadur gwraidd a rennir
sync0 push team-vault:// ./new_release -p <node address>

# Dim ond aelodau sydd wedi'u hawdurdodi i ragddodiad penodol (fel uwchlwythiadau/bob): uwchlwythwch i'r rhagddodiad llwybr a ganiateir
sync0 push team-vault://uploads/bob/report.pdf ./report.pdf -p <nod cyfeiriad>
```

### 5. Tanysgrifio i gysoni a hadu awtomatig (`sync0 sub`)

Ar ôl tanysgrifio i grŵp rhannu nod penodol, mae'r gweinydd lleol yn gleient sy'n derbyn ac yn weinydd hadu newydd; pan fydd y ffeil nod ffynhonnell yn cael ei diweddaru, mae'n cael ei chydamseru'n awtomatig ac yn darparu lawrlwythiadau talpedig ar gyfer nodau eraill:

```bash
# Tanysgrifiwch i'r grŵp rhannu cyfan a hadu'n awtomatig
sync0 is-set data:// ./local_mirror -p <nod cyfeiriad>

# Dim ond aml-ddewis i danysgrifio i ffeiliau ac is-gyfeiriaduron penodedig a'u hadu
sync0 is-set data://models/llama.onnx,configs ./local_mirror -p <nod cyfeiriad>
```

### 6. Cydamseru cyfoedion grŵp aml-beiriant (`sync0 group`)

Mae peiriannau lluosog yn ymuno â'r un grŵp rhannu, mae ei gilydd yn darganfod nodau hadau sydd ar gael yn awtomatig ac yn cynnal cydamseriad dwy ffordd (yn cefnogi cydamseriad llawn neu ddetholiad lluosog o ffeiliau a chyfeiriaduron):

```bash
# Ymunwch yn llawn â'r grŵp a chydamseru dwy ffordd
sync0 group team-docs:// ./docs -p <nod cyfeiriad>

# Dim ond y cyfeiriadur wal a ffeil meta.json yn y grŵp cydamseru aml-ddewis
sync0 group team-docs://wal,meta.json ./docs -p <nod cyfeiriad>
```

### 7. Trawsyriant uniongyrchol pwynt-i-bwynt rhwng dau beiriant (`send` / `recv`)

Nid oes angen ffurfweddu grŵp ymlaen llaw, a gallwch drosglwyddo ffeiliau neu gyfeiriaduron dros dro yn uniongyrchol rhwng dau beiriant:

```bash
# Monitro derbynnydd
sync0 recv 0.0.0.0:9000 ./backup

# Mae'r diwedd anfon yn trosglwyddo ffeiliau neu gyfeiriaduron yn uniongyrchol (dewisol --dileer drych i lanhau ffeiliau gormodol, terfyn cyflymder --cyfradd)
sync0 send ./project 192.168.1.10:9000
```

### 8. Newid aml-iaith

Mae `sync0` yn cefnogi 75 o ieithoedd, ac yn newid yn awtomatig yn ôl amgylchedd iaith y system yn ddiofyn; gellir ei nodi â llaw hefyd trwy `SYNC0_LANG`:

```bash
SYNC0_LANG=en sync0 --help
SYNC0_LANG=ja sync0 --help
SYNC0_LANG=de sync0 --help
```