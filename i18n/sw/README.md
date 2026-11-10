# sync0

`sync0` ni ugawizaji wa faili wa P2P uliogatuliwa, ulandanishaji wa wakati halisi na zana ya usambazaji wa vyanzo vingi inayotumika kuchukua nafasi ya `rsync` - kasi iliyopimwa ya mtandao wa eneo pana linalovuka bara ni hadi **6.95×** ya `rsync` (kasi ya wastani ya kijiometri ya nukta moja **2.01×**), P3.3 ya wastani ya kasi ya kijiometri***, P3P ya wastani* ya kijiometri*, na P3P ya kasi mbili*** ulandanishi wa faili kubwa ndogo kwenye bahari ni haraka kama **7.19×**.

Kila mashine ni seva na mteja: hakuna haja ya seva kuu au anwani ya mtandao wa umma, ubomoaji wa shimo otomatiki unatokana na anwani ya ufunguo wa umma uliosambazwa, na inasaidia upitishaji wa moja kwa moja kati ya mashine mbili, upangishaji wa ruhusa wa vikundi vingi, uteuzi kadhaa wa subdirectories unapohitajika, usawazishaji wa nyongeza wa rasilimali nyingi wa kiwango cha millisecond na upatanishi wa P2P.

## Sakinisha

### Njia ya 1: Ufungaji wa mizigo

```bash
cargo install sync0
```

### Njia ya 2: Pakua jozi zilizokusanywa mapema

Nenda kwa [GitHub Releases](https://github.com/webc-site/sync0/releases) ili kupakua `.tar.xz` kifurushi kilichobanwa kinacholingana na mfumo (inaauni Linux, macOS, Windows, Android, iOS, FreeBSD), fungua na uweke kwenye `PATH`:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Anza haraka

### 1. Usambazaji wa moja kwa moja wa usanidi sifuri kati ya mashine mbili (`send` / `recv`)

Hakuna haja ya kusanidi kikundi cha kushiriki, kuhamisha faili moja kwa moja au saraka kati ya mashine mbili kwa kasi ya juu:

```bash
# Mwisho wa kupokea husikiza kwenye mlango na kuandika kwa ./backup
sync0 recv 0.0.0.0:9000 ./backup

# Utumaji wa moja kwa moja kutoka kwa mtumaji (hiari --futa kioo ili kusafisha faili nyingi, --rate kikomo cha kasi cha 50MB)
sync0 send ./project 192.168.1.10:9000
```

### 2. Kushiriki chapisho na kuvuta/kupakia unapohitaji (`share` / `pull` / `push`)

```bash
# Seva: Chapisha kikundi cha kushiriki na safu moja ya amri (toa anwani ya nodi ya iroh isiyo na IP baada ya kuanza)
sync0 share dataset:./data

# Mteja: Vuta kikundi kizima cha kushiriki seti ya data hadi ./local_data
sync0 pull dataset:// ./local_data -p <nodi address>

# Mteja: Vuta chaguo nyingi unapohitaji (sawazisha faili na saraka zilizobainishwa pekee)
sync0 pull dataset://docs/readme.md,assets/icons,src/core ./local_data -p <anwani ya nodi>

# Mteja: Pakia faili au saraka kwa vikundi vilivyoidhinishwa vya kushiriki
sync0 push dataset://releases/v1 ./build_output -p <anwani ya nodi>
```

### 3. Usajili wa wakati halisi na usawazishaji wa njia mbili wa mashine nyingi (`sub` / `group`)

Baada ya kuanzisha muunganisho wa muda mrefu, ikiwa faili yoyote ** kuongeza, kurekebisha, kufuta, kubadilisha ruhusa au wakati wa kurekebisha ** hutokea kwenye mwisho wa mbali, saraka ya ndani itasasishwa kwa usawa katika milliseconds kwa wakati halisi (faili mpya / zilizorekebishwa zitavuta tu vizuizi tofauti na kuzisasisha mahali, kufuta faili zitasafisha kiotomatiki faili za ndani na saraka tupu, na kubadilisha tu ruhusa za data moja kwa moja)

- **`sync0 sub`(Kuakisi kwa njia moja kwa wakati halisi + upandaji otomatiki)**: Muda halisi wa ndani hufuata nyongeza, ufutaji na marekebisho ya mwisho wa kushiriki, na hutumika kiotomatiki kama chanzo cha mbegu cha P2P ili kutoa uharakishaji kwa wakati mmoja kwa nodi zingine za kuvuta.
- **`sync0 group`(Usawazishaji wa njia mbili wa wakati halisi wa mashine-zaidi-kwa-rika)**: Faili zitaongezwa, kufutwa, au kurekebishwa kwenye mashine yoyote kwenye kikundi, nodi nyingine zote za mtandaoni zitasasishwa kwa usawazishaji katika muda halisi na kutumika kama mbegu za kila mmoja.

```bash
# Jiandikishe kwa kikundi kizima cha kushiriki: nyongeza za mbali, ufutaji, na marekebisho husawazishwa ndani ya nchi kwa wakati halisi na kupandwa kiotomatiki.
sync0 seti ndogo ya data:// ./local_mirror -p <anwani ya nodi>

# Usajili wa chaguo nyingi unapohitajika: landanisha tu na mbegu bainisha faili moja na saraka ndogo kwa wakati halisi.
sync0 seti ndogo ya data://models/llama.onnx,configs ./local_mirror -p <anwani ya nodi>

# Kundi la rika la mashine nyingi za njia mbili za ulandanishi wa wakati halisi (pia inasaidia njia kamili au za chaguo nyingi za koma)
sync0 team-docs:// ./docs -p <anwani ya nodi>
```

### 4. Usanidi wa idhini ya timu ya kushiriki sehemu nyingi na washiriki (`.nt` usanidi)

Saraka chaguo-msingi ya usanidi ni `~/.config/sync0` (inaweza kubadilishwa kupitia `-c <directory>`), kwa kutumia umbizo la chini kabisa [NestedText](https://nestedtext.org) (`.nt`):
- **Inaauni kuzuia usomaji wa mtandao wa umma (watumiaji walioidhinishwa pekee ndio wanaweza kusoma)**: Ilimradi kikundi cha kushiriki kimesanidiwa na `users:`orodha ya washiriki (au mstari wa amri unabainisha `--allow-r` / `--allow-rw` / `--allow-w` / `--auth`), mfumo **uwezeshaji wa 6 na kuwasha faragha(6)<<b>otomatiki =" na kuwasha pro*b 6(hi) kiotomatiki (6)<b>> probit ya faragha (hi)<b>hibiti=" na kuwasha umma*b i . mtandao bila majina kuhesabu au kusoma, na P2P Kujisajili kwa nodi za mbegu pia kurithi orodha iliyoidhinishwa kwa wakati mmoja, kuruhusu watumiaji walioidhinishwa pekee kusoma; vikundi vya kushiriki ambavyo havijasanidiwa na `users` ni vya hadharani na vinasomwa tu kwa chaguo-msingi (ikiwa unataka kuonekana hadharani na kusoma pekee na kufunguliwa kwa upakiaji kwa wanachama waliobainishwa pekee, unaweza kuweka `auth: false` kwa uwazi).
- **Ruhusa nzuri za kusoma-kuandika na uwasilishaji wa saraka ndogo**: Wanachama husomwa tu kwa chaguo-msingi (`r` haihitaji kuandikwa kwa uwazi), na inasaidia `r` (kusoma pekee), `w` (andika tu kwenye kisanduku cha kunjuzi, kisichosomeka), `rw`5(w) 5 (w) t (w) soma (w)  (w)  (w)  (w) t (w) kiambishi awali kinachoruhusiwa kupakiwa).

**Mfano wa usanidi wa mwanachama (`~/.config/sync0/users/`):**

```yaml
# ~/.config/sync0/users/alice.nt (chaguo-msingi ya kusoma tu, kikomo cha kasi cha 80MB)
id: 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c
rate: 80MB
```

```yaml
# ~/.config/sync0/users/bob.nt (ruhusa za kusoma na kuandika, zinazoruhusiwa tu kupakia kwenye saraka ndogo maalum)
id: 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f
perm: rw
write_prefix:
  - uploads/bob
  - shared/common
```

```yaml
# ~/.config/sync0/users/charlie.nt (ruhusa ya uwasilishaji ya maandishi pekee, usomaji umepigwa marufuku)
id: 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
perm: w
```

**Mfano wa usanidi wa kikundi unaoshiriki (`~/.config/sync0/config.nt`):**

```yaml
users_dir: ./users

groups:
  # Si watumiaji waliosanidiwa: mtandao chaguo-msingi wa umma wa kusoma pekee
  dataset:
    path: ./data
    rate: 50MB

  # Watumiaji waliosanidiwa: zuia kiotomati usomaji wa mtandao wa umma (alice na bob pekee ndio wanaoweza kusoma, charlie anaweza tu kutoa na kupakia)
  team-vault:
    path: ./vault
    users:
      - alice
      - bob
      - charlie
```

Anzisha huduma (pakia faili ya usanidi au tumia moja kwa moja vigezo vya mstari wa amri ili kuzuia usomaji wa mtandao wa umma na kuidhinisha wanachama):

```bash
# Pakia ~/.config/sync0/config.nt ili kuanza
sync0 share --region SHA

# Au chapisha kikundi cha kushiriki kibinafsi kupitia mstari wa amri safi (taja --allow-* ili kuzuia usomaji wa mtandao wa umma kiotomatiki na ni watumiaji walioidhinishwa pekee wanaoweza kuipata)
sync0 share team-vault:./vault \
  --allow-r 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c \
  --allow-rw 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f:uploads/bob,shared/common \
  --allow-w 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
```

### 5. Anwani ya nodi, kadi ya biashara na lugha nyingi

```bash
# Tazama anwani ya nodi ya ndani isiyo na IP (ufunguo hutengenezwa kiotomatiki kwa mara ya kwanza)
sync0 id

# Tengeneza kadi ya biashara iliyosainiwa ya Ed25519 (jina la utani, nambari ya jiji, barua pepe, tovuti, utangulizi)
sync0 profile -u alice --region SHA --email alice@example.com -b "Shanghai node"

# Kuna lugha 75 zilizojengwa ndani, ambazo hufuata lugha ya mfumo kwa chaguo-msingi na zinaweza kubadilishwa kupitia vigezo vya mazingira.
SYNC0_LANG=en sync0 --help
```

## Utendaji na sifa kuu

### Kipimo halisi cha WAN katika bara zima (Ulinganisho `rsync`)

Mazingira halisi ya kipimo: Marekani `c1` → Ulaya `n1` (RTT 65 ms), seti ya data ni Linux kernel `linux-7.3.3`:

#### 1) Hali ya la carte dhidi ya `rsync`

| Seti ya data | ukubwa | `sync0` Pointi Moja<br>(Mbps) | `rsync`<br>(Mbps) | Kuongeza kasi (`sync0` / `rsync`) |
| :--- | ---: | ---: | ---: | ---: |
| `linux-7.3.3.tar.xz`(haiwezi kubana) | 155 MB | **121.07** | 107.89 | **1.12×** |
| `linux-7.3.3.tar`(inayobana) | 1.6 GB | **442.07** | 506.62 | **0.87×** |
| `linux-7.3.3.tar`(ongezeko la mabadiliko ya ndani) | 1.6 GB | **8,123.87** | 1,169.49 | **6.95×** |
| `linux-7.3.3/` (faili ndogo 95,979) | 1.5 GB | **565.49** | 235.27 | **2.40×** |
| **maana ya kijiometri** | - | **704.17** | 350.19 | **2.01×** |

#### 2) Hali ya P2P ya vyanzo viwili dhidi ya `rsync`

| Seti ya data | ukubwa | `sync0` P2P Chanzo Mbili<br>(Mbps) | `rsync`<br>(Mbps) | Kuongeza kasi (`sync0` / `rsync`) |
| :--- | ---: | ---: | ---: | ---: |
| `linux-7.3.3.tar.xz`(haiwezi kubana) | 155 MB | **466.85** | 107.89 | **4.33×** |
| `linux-7.3.3.tar`(inayobana) | 1.6 GB | **992.17** | 506.62 | **1.96×** |
| `linux-7.3.3.tar`(ongezeko la mabadiliko ya ndani) | 1.6 GB | **5,666.84** | 1,169.49 | **4.85×** |
| `linux-7.3.3/` (faili ndogo 95,979) | 1.5 GB | **703.30** | 235.27 | **2.99×** |
| **maana ya kijiometri** | - | **1,165.63** | 350.19 | **3.33×** |

> Kumbuka: Katika hali ya P2P-chanzo-mbili, nodi za Ulaya `s1` hushirikiana kwa ajili ya kupanda, na uti wa mgongo wa mabara RTT ni 65 ms.

### Vipengele vya msingi vya bidhaa

- **Ufunguo wa umma unaoshughulikia ubomoaji wa shimo kiotomatiki**: Hakuna haja ya seva kuu na anwani ya mtandao wa umma, ubomoaji wa kiotomatiki wa shimo la NAT kulingana na anwani ya mtandao iliyosambazwa ya ufunguo wa umma.
- **Usawazishaji wa wakati halisi wa kiwango cha milisekunde na ufahamu wa metadata**: Kulingana na ufuatiliaji wa tukio la faili asili (`FSEvents` / `inotify` / `ReadDirectoryChangesW`), usukumaji wa wakati halisi wa nyongeza za faili, ufutaji, ruhusa za POSIX (`mode`) na muda wa urekebishaji wa kiwango cha millisecond=""><b5); uwasilishaji wa kuzuia data sifuri wakati ruhusa au wakati pekee zinarekebishwa.
- **Mtindo wa BT upakuaji na upakiaji kwa wakati mmoja na upangaji wa mshikamano wa kieneo**: Njia ya upakuaji haihitaji kusubiri hadi kiasi kamili ipakuliwe. Vipindi vyote vya kuzuia (`Op::Have`) ambavyo vimepitisha ukaguzi wa heshi ya biti 128 hufungwa mara moja kwenye hifadhi ya kumbukumbu/muda kwa nodi nyingine kuvuta kwa wakati mmoja. Pamoja na ubadilishanaji wa nodi za PEX na uokoaji wa nusu ya nodi polepole (Uiba-Kazi), upitishaji wa nguzo unakuzwa.
- **Uharibifu wa nodi dhidi ya malicious na kinga ya sumu ya nguzo**: authoritative source list `root_hash` Kufunga kwa kujitegemea + kuongeza kasi ya vifaa vya lazima `gxhash128` uthibitishaji wa digest kabla ya kila kizuizi kuwekwa kwenye diski na kupandwa nje; ikiwa kizuizi kilichoharibiwa kitapatikana, kivunja mzunguko wa kiwango cha millisecond kitazima nodi hasidi na kupona kiotomatiki kutoka kwa chanzo cha uaminifu.
- **Ongezeko la FastCDC in-situ na mkusanyiko wa faili ndogo**: Vitalu vya tofauti pekee ndivyo vinavyohamishwa kwa marekebisho ya sehemu ya faili kubwa na kuwekwa kwenye diski; ufungashaji mkubwa wa kumbukumbu ya faili ndogo na ukandamizaji wa tofauti wa kiambishi awali huondoa faili-kwa-faili ya safari ya kwenda na kurudi.
- **Kujiponya baada ya umeme kukatika na kuanza tena baada ya kuwasha upya**: Umbizo la mseto linalofanana na RDB + AOF (`.s0_subs.bin`) hutumika kudumisha hali ya mbegu na orodha ya faili, mabadiliko ya kila siku huongezwa kwa sekunde ndogo, na huandikwa upya kiotomatiki na kupangwa mara kwa mara. Baada ya seva kuwashwa upya, mabadiliko ya nje ya mtandao yanasukumwa kiotomatiki kwenye nodi ya mbegu.

## Usanifu wa kiufundi na nyaraka

- **Itifaki ya kutenganisha ndege mbili**:
  - **Udhibiti na uso wa mabadiliko ya wakati halisi (`iroh` QUIC / TCP, first byte `PROTO_VERSION` toleo la handshake) **: Inawajibika kwa uthibitishaji wa kadi ya biashara ya Ed25519, uthibitishaji wa ruhusa, ugunduzi wa mbegu za Swarm, na usaidizi wa `u64::MAX` wa mabadiliko ya kiwango cha utiririshaji batching na `mzstd``mzstd` kubatizwa `mzstd``mzstd``mzstd``mzstd``mzstd` iliyobanwa `FsOp::Remove` / `FsOp::Meta` Tatu za awali).
  - **Sehemu ya usambazaji wa data (UDP asilia + `compio` isiyosawazisha I/O) **: Kichwa cha kushikamana 20B, maoni ya mara kwa mara ya 18B TACK, kidhibiti cha msongamano cha BBRv3/Copa, urejeshaji wa upotevu wa pakiti ya RACK-TLP, kizuizi cha kwanza cha Shannon entropy adaptive Zstd compression bypass, na `gxhash128`end.
- **Pata maelezo zaidi**:
  - [Ripoti ya kipimo cha utendaji na hati ya kuzaliana](https://github.com/webc-site/sync0/blob/main/doc/zh/benchmark.md)
  - [Ubunifu wa usanifu wa mfumo](https://github.com/webc-site/sync0/blob/main/doc/zh/design_v4.md)
  - [Itifaki ya Waya ya Binary na Uainishaji wa Kudumu](https://github.com/webc-site/sync0/blob/main/doc/zh/proto.md)
  - [Uthibitishaji wa utambulisho, udhibiti wa mamlaka na uthibitishaji wa uadilifu](https://github.com/webc-site/sync0/blob/main/doc/zh/crypto.md)