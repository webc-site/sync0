# sync0

Mae `sync0` yn offeryn rhannu ffeiliau P2P datganoledig, cydamseru amser real a dosbarthu aml-ffynhonnell a ddefnyddir i ddisodli `rsync` - mae cyflymder mesuredig WAN traws-gyfandirol hyd at `rsync` ** 5.89 × ** (cyflymder cyfartalog geometrig pwynt sengl **2.67 ×**, cyflymder geometrig ffynhonnell ddeuol P2P**3 uchel), cyflymder geometrig deuol-ffynhonnell uchel. Mae cyflymiad P2P hyd at **7.68 × ** (cyflymder cymedrig geometrig cynhwysfawr 24-senario cyswllt llawn ** 2.56 ×**).

Mae pob peiriant yn weinydd ac yn gleient: nid oes angen gweinydd canolog na chyfeiriad rhwydwaith cyhoeddus, mae dyrnu twll awtomatig yn seiliedig ar gyfeiriad allwedd cyhoeddus y rhwydwaith dosbarthedig, ac mae'n cefnogi trosglwyddiad uniongyrchol rhwng dau beiriant, cynnal caniatâd grŵp aml-rannu, dewis lluosog o is-gyfeiriaduron yn ôl y galw, cydamseru cynyddrannol amser real lefel milieiliad a chyflymiad cydamserol aml-ffynhonnell P2P.

## Gosod

### Dull 1: Gosod cargo

```bash
cargo install sync0
```

### Dull 2: Lawrlwythwch deuaidd sydd wedi'u llunio ymlaen llaw

Ewch i [GitHub Releases] (/0) i lawrlwytho'r pecyn `.tar.xz` cywasgedig sy'n cyfateb i'r system (yn cefnogi Linux, macOS, Windows, Android, iOS, FreeBSD), dadsipiwch a rhowch yn `PATH`:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Dechreuwch yn gyflym

### 1. Trosglwyddiad uniongyrchol sero-gyfluniad rhwng dau beiriant (`send` / `recv`)

Nid oes angen ffurfweddu grŵp rhannu, trosglwyddo ffeiliau neu gyfeiriaduron yn uniongyrchol rhwng dau beiriant ar gyflymder uchel:

```bash
# Mae'r pen derbyn yn gwrando ar y porthladd ac yn ysgrifennu at ./backup
sync0 recv 0.0.0.0:9000 ./backup

# Trosglwyddiad uniongyrchol gan yr anfonwr (dewisol -- dileu drych i lanhau ffeiliau gormodol, --cyfradd terfyn cyflymder 50MB)
sync0 send ./project 192.168.1.10:9000
```

### 2. Rhannu post a thynnu/llwytho i fyny ar-alw (`share` / `pull` / `push`)

```bash
# Gweinydd: Cyhoeddi'r grŵp rhannu gydag un llinell orchymyn (allbwn y cyfeiriad nod iroh di-IP ar ôl cychwyn)
sync0 share dataset:./data

# Cleient: Tynnwch y grŵp rhannu set ddata cyfan i'r ./local_data lleol
sync0 tynnu set ddata:// ./local_data -p <nod cyfeiriad>

# Cleient: Tynnwch ddetholiadau lluosog yn ôl y galw (dim ond cydamseru ffeiliau sengl ac is-gyfeiriaduron penodedig)
set ddata tynnu sync0://docs/readme.md,assets/icons,src/core ./local_data -p <nod cyfeiriad>

# Cleient: Llwythwch i fyny ffeiliau neu gyfeiriaduron i grwpiau rhannu awdurdodedig
set ddata gwthio sync0://releases/v1 ./build_output -p <nod cyfeiriad>
```

### 3. Hadu tanysgrifiad amser real a chydamseru dwy ffordd aml-beiriant (`sub` / `group`)

Ar ôl sefydlu cysylltiad hir, os bydd unrhyw ffeil **ychwanegu, addasu, dileu, newid caniatâd neu amser addasu** yn digwydd ar y pen pell, bydd y cyfeiriadur lleol yn cael ei ddiweddaru'n gydamserol mewn milieiliadau mewn amser real (bydd ffeiliau mawr newydd / addasedig ond yn tynnu'r blociau gwahaniaeth ac yn eu diweddaru yn eu lle, bydd dileu ffeiliau yn glanhau ffeiliau lleol a chyfeiriaduron gwag yn awtomatig, a dim ond yn newid caniatâd neu stamp amser blociau data yn uniongyrchol):

- **`sync0 sub` (Drychio amser real unffordd + hadu awtomatig)**: Mae'r amser real lleol yn dilyn ychwanegiadau, dileadau ac addasiadau'r diwedd rhannu, ac mae'n gweithredu'n awtomatig fel ffynhonnell hadau P2P i ddarparu cyflymiad cydamserol ar gyfer nodau tynnu eraill.
- **`sync0 group` (Cydamseru dwy ffordd aml-beiriant cyfoedion-i-gymar)**: Os caiff ffeiliau eu hychwanegu, eu dileu, neu eu haddasu ar unrhyw beiriant yn y grŵp, bydd yr holl nodau ar-lein eraill yn cael eu diweddaru'n gydamserol mewn amser real ac yn gwasanaethu fel hadau i'w gilydd.

```bash
# Tanysgrifiwch i'r grŵp rhannu cyfan: mae ychwanegiadau o bell, dileadau, ac addasiadau yn cael eu cysoni'n lleol mewn amser real a'u hadu'n awtomatig
sync0 is-set data:// ./local_mirror -p <nod cyfeiriad>

# Tanysgrifiad aml-ddewis ar-alw: dim ond cydamseru a hadu ffeiliau unigol ac is-gyfeiriaduron penodedig mewn amser real
sync0 is-set data://models/llama.onnx,configs ./local_mirror -p <nod cyfeiriad>

# Cydamseru amser real dwy ffordd grŵp cyfoedion aml-beiriant (hefyd yn cefnogi llwybrau aml-ddewis llawn neu goma)
sync0 group team-docs:// ./docs -p <nod cyfeiriad>
```

### 4. Ffurfweddiad caniatâd grŵp aml-rannu tîm ac aelod (`.nt` ffurfweddiad)

Y cyfeiriadur cyfluniad rhagosodedig yw `~/.config/sync0` (gellir ei newid trwy `-c <directory>`), gan ddefnyddio'r fformat minimalaidd [NestedText](https://nestedtext.org)(`.nt`):
- **Yn cefnogi cyfyngu ar ddarllen rhwydwaith cyhoeddus (dim ond defnyddwyr awdurdodedig all ddarllen)**: Cyn belled â bod y grŵp rhannu wedi'i ffurfweddu â rhestr aelodau `users:` (neu mae'r llinell orchymyn yn nodi `--allow-r` / `--allow-rw` / `--allow-w` / `--auth`), mae'r system ** rhwydwaith yn gwahardd id dilysu cyhoeddus (<**) yn awtomatig ac yn gwahardd id dilysu cyhoeddus (<**) cyfrifo neu ddarllen, a P2P Bydd tanysgrifio i nodau hadu hefyd yn etifeddu'r rhestr wen ar yr un pryd, gan ganiatáu i ddefnyddwyr awdurdodedig yn unig ddarllen; rhannu grwpiau nad ydynt wedi'u ffurfweddu â `users` yn gyhoeddus ac yn ddarllenadwy yn unig yn ddiofyn (os ydych am fod yn gyhoeddus ac yn ddarllenadwy yn unig ac yn agored i uwchlwythiadau yn unig i aelodau penodedig, gallwch osod `auth: false` yn benodol).
- **Caniatadau darllen-ysgrifenedig ac is-gyfeiriaduron manwl**: Mae aelodau'n cael eu darllen yn unig yn ddiofyn (nid oes angen ysgrifennu `r` yn benodol), a chefnogaeth `r` (darllen yn unig), `w` (dim ond ysgrifennu i'r blwch gollwng, nid yw'n ddarllenadwy), `rw` (darllen-ysgrifennu) a `r`caniateir uwchlwytho'r is-bennawd 5).

**Enghraifft ffurfweddiad aelod (`~/.config/sync0/users/`):**

```yaml
# ~/.config/sync0/users/alice.nt (darllen yn unig diofyn, terfyn cyflymder 80MB)
id: 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c
rate: 80MB
```

```yaml
# ~/.config/sync0/users/bob.nt (caniatadau darllen ac ysgrifennu, dim ond i'w huwchlwytho i is-gyfeiriadur penodedig y caniateir ei uwchlwytho)
id: 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f
perm: rw
write_prefix:
  - uploads/bob
  - shared/common
```

```yaml
# ~/.config/sync0/users/charlie.nt (ysgrifennwch ganiatâd danfon yn unig, gwaherddir darllen)
id: 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
perm: w
```

**Rhannu enghraifft o ffurfweddiad grŵp (`~/.config/sync0/config.nt`):**

```yaml
users_dir: ./users

groups:
  # Defnyddwyr heb eu ffurfweddu: rhwydwaith cyhoeddus rhagosodedig darllen yn unig
  dataset:
    path: ./data
    rate: 50MB

  # Defnyddwyr wedi'u ffurfweddu: cyfyngu darllen rhwydwaith cyhoeddus yn awtomatig (dim ond Alice a Bob sy'n gallu darllen, dim ond anfon a llwytho i fyny y gall charlie ei anfon)
  team-vault:
    path: ./vault
    users:
      - alice
      - bob
      - charlie
```

Dechreuwch y gwasanaeth (llwythwch y ffeil ffurfweddu neu defnyddiwch baramedrau llinell orchymyn yn uniongyrchol i gyfyngu ar ddarllen rhwydwaith cyhoeddus ac awdurdodi aelodau):

```bash
# Llwythwch ~/.config/sync0/config.nt i ddechrau
sync0 share --region SHA

# Neu cyhoeddwch grŵp rhannu preifat trwy linell orchymyn pur (nodwch --allow-* i gyfyngu darllen rhwydwaith cyhoeddus yn awtomatig a dim ond defnyddwyr awdurdodedig all gael mynediad iddo)
sync0 share team-vault:./vault \
  --allow-r 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c \
  --allow-rw 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f:uploads/bob,shared/common \
  --allow-w 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
```

### 5. Cyfeiriad nod, cerdyn busnes ac aml-iaith

```bash
# Gweld y cyfeiriad nod di-IP lleol (cynhyrchir allwedd yn awtomatig ar gyfer y rhediad cyntaf)
sync0 id

# Cynhyrchu cerdyn busnes nod hunan-lofnod Ed25519 (llysenw, cod dinas, e-bost, gwefan, cyflwyniad)
sync0 profile -u alice --region SHA --email alice@example.com -b "Shanghai node"

# Mae yna 75 o ieithoedd adeiledig, sy'n dilyn iaith y system yn ddiofyn a gellir eu newid trwy newidynnau amgylchedd.
SYNC0_LANG=en sync0 --help
```

## Perfformiad a nodweddion craidd

### Mesuriad gwirioneddol WAN traws-gyfandirol (Cymharu `rsync`)

Amgylchedd mesur gwirioneddol: Unol Daleithiau `c1` → Europe `n1` (RTT 65 ms), y set ddata yw cnewyllyn Linux `linux-7.3.3`:

#### 1) Modd A la carte vs `rsync`

| Set ddata | maint | `sync0` Pwynt Sengl<br>(Mbps) | `rsync`<br>(Mbps) | Cyflymder (`sync0` / `rsync`) |
| :--- | ---: | ---: | ---: | ---: |
| `linux-7.3.3.tar.xz` (ddim yn gywasgadwy) | 155 MB | **236.48** | 108.74 | **2.17×** |
| `linux-7.3.3.tar` (cywasgadwy) | 1.6 GB | **843.83** | 385.06 | **2.19×** |
| `linux-7.3.3.tar` (cynyddiad newid lleol) | 1.6 GB | **8,014.15** | 1,361.72 | **5.89×** |
| `linux-7.3.3/` (95,979 o ffeiliau bach) | 1.5 GB | **627.58** | 347.80 | **1.80×** |
| **cymedr geometrig** | - | **1,000.91** | 375.26 | **2.67×** |

#### 2) Modd ffynhonnell ddeuol P2P yn erbyn `rsync`

| Set ddata | maint | `sync0` Ffynhonnell Ddeuol P2P<br>(Mbps) | `rsync`<br>(Mbps) | Cyflymder (`sync0` / `rsync`) |
| :--- | ---: | ---: | ---: | ---: |
| `linux-7.3.3.tar.xz` (ddim yn gywasgadwy) | 155 MB | **330.09** | 108.74 | **3.04×** |
| `linux-7.3.3.tar` (cywasgadwy) | 1.6 GB | **1,551.86** | 385.06 | **4.03×** |
| `linux-7.3.3.tar` (cynyddiad newid lleol) | 1.6 GB | **5,474.17** | 1,361.72 | **4.02×** |
| `linux-7.3.3/` (95,979 o ffeiliau bach) | 1.5 GB | **706.56** | 347.80 | **2.03×** |
| **cymedr geometrig** | - | **1,186.42** | 375.26 | **3.16×** |

> Nodyn: Yn y modd ffynhonnell ddeuol P2P, mae nodau Ewropeaidd `s1` yn cydweithio ar gyfer hadu, ac mae'r asgwrn cefn traws-gyfandirol RTT yn 65 ms.

### Nodweddion cynnyrch craidd

- ** Allwedd gyhoeddus yn mynd i'r afael â dyrnu twll yn awtomatig**: Nid oes angen gweinydd canolog a chyfeiriad rhwydwaith cyhoeddus, dyrnu twll NAT awtomatig yn seiliedig ar gyfeiriad allwedd cyhoeddus y rhwydwaith dosbarthedig.
- ** Cydamseru amser real lefel millisecond ac ymwybyddiaeth metadata**: Yn seiliedig ar fonitro digwyddiadau ffeil brodorol (`FSEvents` / `inotify` / `ReadDirectoryChangesW`), gwthiad amser real o ychwanegiadau ffeil, dileadau, caniatadau POSIX (`mode`) ac amser addasu lefel milieiliad (`mtime`); trosglwyddiad bloc data sero pan mai dim ond caniatadau neu amser sy'n cael eu haddasu.
- **Lawrlwytho a lanlwytho ar yr un pryd ar ffurf BT ac amserlennu affinedd rhanbarthol**: Nid oes angen i'r nod lawrlwytho aros i'r swm llawn gael ei lawrlwytho. Mae'r holl gyfnodau bloc (`Op::Have`) sydd wedi pasio'r siec stwnsh 128-did yn cael eu cloi ar unwaith yn y pwll cof/dros dro er mwyn i nodau eraill dynnu ar yr un pryd. Ynghyd â chyfnewid nodau PEX deinamig a rhagbrynu hanner nod araf (Gwaith-Dwyn), mae trwygyrch y clwstwr yn cael ei gynyddu i'r eithaf.
- **Ymyriad nod gwrth-faleisus ac imiwnedd gwenwyn clwstwr**: rhestr ffynonellau awdurdodol `root_hash` Cloi hunan-gyson + cyflymiad caledwedd gorfodol `gxhash128` dilysu treuliad cyn gosod pob bloc ar y ddisg a'i hadu'n allanol; os canfyddir bloc wedi'i ymyrryd, bydd y torrwr cylched lefel milieiliad yn blacowt y nod maleisus ac yn adfer yn awtomatig o'r ffynhonnell onest.
- **Cynyddiad in-situ FastCDC a chydgrynhoi ffeiliau bach**: Dim ond blociau gwahaniaethol sy'n cael eu trosglwyddo ar gyfer addasu ffeiliau mawr yn rhannol a'u gosod ar ddisg; pecynnu cof ffeil bach enfawr a chywasgu gwahaniaethol rhagddodiad dileu ffeil-wrth-ffeil rownd-daith gorbenion.
- ** Hunan-iachâd ar ôl toriad pŵer ac ailddechrau ar ôl ailgychwyn **: Defnyddir fformat hybrid tebyg i Redis RDB + AOF (`.s0_subs.bin`) i barhau â'r statws hadu a'r rhestr ffeiliau, mae newidiadau dyddiol yn cael eu hatodi mewn microseconds, ac yn cael eu hailysgrifennu a'u trefnu'n rheolaidd yn awtomatig. Ar ôl i'r gweinydd gael ei ailgychwyn, mae newidiadau all-lein yn cael eu gwthio'n awtomatig i'r nod hadu.

## Pensaernïaeth dechnegol a dogfennaeth

- **Protocol datgysylltu awyren ddeuol**:
  - **Control and real-time change surface (`iroh` QUIC / TCP, first byte `PROTO_VERSION` version handshake) **: Responsible for Ed25519 business card verification, permission verification, Swarm seed discovery, and support for `u64::MAX` level mass change streaming batching and `mzstd` compressed `SubPush` real-time push (`FsOp::Upsert` / `FsOp::Remove` / `FsOp::Meta`Tri cyntefig).
  - **Arwyneb dosbarthu data (CDU brodorol + `compio` I/O asyncronaidd) **: pennawd cryno 20B, adborth cyfnodol 18B TACK, rheolaeth tagfeydd hybrid BBRv3/Copa, adferiad colled pecyn RACK-TLP, ffordd osgoi cywasgu Zstd addasol bloc Shannon cyntaf, a `gxhash128` bloc dilysu diwedd-i-ben.
- **Dysgu mwy**:
  - [Adroddiad mesur perfformiad a sgript atgynhyrchu](https://github.com/webc-site/sync0/blob/main/doc/zh/benchmark.md)
  - [Dyluniad pensaernïaeth system](https://github.com/webc-site/sync0/blob/main/doc/zh/design_v4.md)
  - [Protocol Gwifren Deuaidd a Manyleb Dyfalbarhad](https://github.com/webc-site/sync0/blob/main/doc/zh/proto.md)
  - [Dilysu hunaniaeth, rheolaeth awdurdod a gwirio cywirdeb](https://github.com/webc-site/sync0/blob/main/doc/zh/crypto.md)