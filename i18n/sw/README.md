# sync0

`sync0` ni zana ya kushiriki faili ya peer-to-peer (P2P) na zana ya kusawazisha ya mashine nyingi ambayo inashindana na Usawazishaji wa Resilio.

Kila mashine ni seva na mteja: nodi yoyote inaweza kuchapisha vikundi vingi vya kushiriki faili. Baada ya vifundo vingine kujiunga, wanaweza kupata kiotomatiki nodi za mbegu kutoka kwa seva rika na kuzivuta kwa wakati mmoja kutoka kwa vyanzo vingi. Baada ya kusawazisha kukamilika, mbegu zitaundwa kiotomatiki kwa nodi zaidi ili kuharakisha upakuaji.

## Vivutio vya Msingi

- **Kila mtu ni seva na mteja**: Inalinganishwa na Usawazishaji wa Resilio, hakuna utegemezi wa seva kuu. Nodi yoyote inaweza kuunda vikundi vingi vya kushiriki faili, na pia inaweza kujiunga na vikundi vya kushiriki vya watu wengine; nodi hubadilishana kiotomatiki na kupata nodi za mbegu zinazopatikana ili kufikia uvutaji pamoja na ulandanishi wa wakati halisi wa mashine nyingi.
- **Anwani ya nodi isiyo na IP**: Kwa chaguo-msingi, kitufe cha nodi na `base64url` anwani iliyosimbwa `iroh` huzalishwa kiotomatiki katika `~/.config/sync0` bila kuhitaji kukumbuka au kufichua IP ya umma.
- **Chaguo nyingi ulandanishi unapohitajika**: Inaauni ulandanishi wa kikundi kizima cha kushiriki, na pia inasaidia ulandanishi unapohitajika kwa kuchagua faili nyingi au saraka ndogo kwa wakati mmoja.
- **Ruhusa za daraja la washiriki wa kikundi na udhibiti wa saraka ya upakiaji**: Inaauni ruhusa za kusoma tu (`ro`), kusoma-kuandika (`rw`), na ruhusa za usimamizi (`admin`), na inaweza kubainisha kiambishi awali cha njia ambacho kila mshiriki anaruhusiwa kupakia (kwa chaguo-msingi, upakiaji kwenye saraka ya msingi ya kushiriki unaruhusiwa).
- **Usawazishaji wa kasi kamili na wa ziada kwenye mitandao**: Utumaji wa faili kubwa kwa wakati mmoja, ujumlisho wa faili kubwa ndogo kwenye kumbukumbu, na vizuizi vya data vilivyosawazishwa tu faili zinaporekebishwa kwa kiasi.

## Ulinganisho wa kasi na rsync

### Mtandao wa Transoceanic kipimo halisi cha chanzo kimoja (Uchina → Marekani, kuchelewa kwa ms 260)

| Hali ya maambukizi | Ukubwa wa data | mfano | `sync0` | `rsync -az` | Ongeza kizidishi |
| :--- | :---: | :---: | :---: | :---: | :---: |
| 500 faili ndogo | 1.91 MB | Usawazishaji kamili | 18.84 Mbps | 5.32 Mbps | 3.5x |
| 500 faili ndogo | 1.91 MB | Usawazishaji unaoongezeka | 27.15 Mbps | 3.83 Mbps | 7.1x |
| Msimbo na vifurushi vya kumbukumbu (`.tar`) | 10.0 MB | Usawazishaji kamili | 139.70 Mbps | 24.97 Mbps | 5.6x |
| Msimbo na vifurushi vya kumbukumbu (`.tar`) | 10.0 MB | Usawazishaji unaoongezeka | 83.38 Mbps | 26.02 Mbps | 3.2x |
| Faili ya video (`.mp4`) | 15.0 MB | Usawazishaji kamili | 54.06 Mbps | 27.04 Mbps | 2.0x |
| Faili ya video (`.mp4`) | 15.0 MB | Usawazishaji unaoongezeka | 216.83 Mbps | 21.51 Mbps | 10.1x |

### Mtandao wa Transoceanic P2P mtihani wa kuvuta nodi nyingi (Uchina → nodi mbili za Marekani, kuchelewa kwa ms 260)

Ulinganisho uliopimwa wa nodi ya vyanzo vingi vya mbegu yenye milia ya kuvuta kwa wakati mmoja:

| Hali ya maambukizi | Ukubwa wa data | Mbegu za nodi moja | Mbegu za P2P za nodi mbili kwa wakati mmoja | Ongeza kizidishi |
| :--- | :---: | :---: | :---: | :---: |
| 500 faili ndogo | 1.97 MB | 5.28 Mbps | 7.87 Mbps | 1.5x |
| Msimbo na vifurushi vya kumbukumbu (`.tar`) | 30.0 MB | 56.35 Mbps | 83.17 Mbps | 1.5x |
| Faili ya video (`.mp4`) | 30.0 MB | 27.00 Mbps | 39.46 Mbps | 1.5x |

### Kipimo halisi cha laini ya kasi ya juu inayovuka bara (Ulaya → Amerika Kaskazini, kuchelewa kwa ms 86)

| Hali ya maambukizi | Ukubwa wa data | `sync0` | `rsync` | Ongeza kizidishi |
| :--- | :---: | :---: | :---: | :---: |
| 10,000 faili ndogo | 77.6 MB | 688 Mbps | 128 Mbps | 5.4x |
| Msimbo na vifurushi vya kumbukumbu (`.tar`) | 512 MB | 2111 Mbps | 438 Mbps | 4.8x |
| Faili ya video (`.mp4`) | 512 MB | 556 Mbps | 338 Mbps | 1.6x |
| Usawazishaji unaoongezeka (rekebisha maudhui 1%) | 512 MB | 34133 Mbps | 2214 Mbps | 15.4x |

## Sakinisha

### Njia ya 1: Sakinisha kupitia Cargo

```bash
cargo install sync0
```

### Njia ya 2: Pakua jozi zilizokusanywa mapema

Nenda kwa [GitHub Releases](https://github.com/webc-site/sync0/releases) ili kupakua `.tar.xz` kifurushi kilichobanwa kwa mfumo unaolingana (inaauni Linux, macOS, Windows, Android, FreeBSD, NetBSD), fungua unzip na ukiweke kwenye saraka ya `PATH`:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Anza haraka

`sync0` hutumia `~/.config/sync0` kama saraka ya usanidi kwa chaguo-msingi (ikiwa unahitaji kubadilisha hadi saraka nyingine, unaweza kuongeza `-c <directory>`).

### 1. Tazama anwani ya eneo la nodi na uweke kadi ya biashara

```bash
# Tazama anwani ya eneo la eneo (ufunguo hutengenezwa kiotomatiki ~/.config/sync0 kwa mara ya kwanza)
sync0 id

# Tengeneza kadi ya biashara ya nodi ya kibinafsi (inasaidia jina la mtumiaji, jiji, barua pepe, tovuti na sahihi ndani ya maneno 256)
sync0 profile -u alice --region SHA --email alice@example.com --url https://example.com -b "Shanghai node"
```

### 2. Chapisha vikundi vingi vya kushiriki faili na uweke ruhusa za washiriki wa kikundi (`.nt` Usanidi)

`sync0` imesanidiwa katika umbizo la [NestedText](https://nestedtext.org) (`.nt`). Kila mtumiaji ana `.nt` faili tofauti ya usanidi. Unaweza kubainisha saraka ya wasifu wa mtumiaji kupitia `users_dir` katika `config.nt` (ambayo inajumuisha kiotomatiki faili zote `.nt` katika saraka hii). Mtumiaji na kiambishi awali cha njia kinachoruhusiwa kupakiwa hutumia orodha isiyopangwa (`-`). **Wakati kiambishi awali cha njia ya upakiaji hakijajazwa, upakiaji kwenye saraka ya mizizi ya sehemu inaruhusiwa kwa chaguo-msingi**.

1. Unda `.nt` faili tofauti za usanidi kwa kila mwanachama katika saraka ya `~/.config/sync0/users/`:

`~/.config/sync0/users/alice.nt` (mwanachama wa kusoma pekee, kiwango kidogo `80MB`, `rate` kinaauni vitengo `GB`, `MB`, `KB`):

```yaml
id: 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c
perm: ro
rate: 80MB
```

`~/.config/sync0/users/bob.nt` (wanachama waliowekewa vikwazo: wanaruhusiwa kupakia tu kwenye saraka za `uploads/bob` na `shared/common`):

```yaml
id: 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f
perm: rw
write_prefix:
  - uploads/bob
  - shared/common
```

`~/.config/sync0/users/charlie.nt` (Upakiaji chaguo-msingi wa mwanachama: Wakati `write_prefix` imeachwa, upakiaji kwenye saraka ya mizizi ya ugavi unaruhusiwa kwa chaguomsingi):

```yaml
id: 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
perm: rw
```

2. Sanidi saraka ya mtumiaji na vikundi vingi vya kushiriki faili katika `~/.config/sync0/config.nt`:

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

Anza kushiriki huduma:

```bash
# Kuanzisha moja kwa moja (hupakia kiotomati usanidi wote wa .nt katika ~/.config/sync0/config.nt na saraka za watumiaji_dir)
sync0 share --region SHA

# Au uchapishe haraka vikundi vingi vya kushiriki na uidhinishe washiriki wa kikundi moja kwa moja kupitia safu ya amri
sync0 share dataset:./data team-vault:./vault \
  --auth \
  --allow-ro 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c \
  --allow-rw 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f:uploads/bob,shared/common \
  --allow-rw 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
```

### 3. Vuta maudhui yote au chagua faili na saraka nyingi (`sync0 pull`)

Unganisha kwa nodi rika yoyote ili kupata kiotomatiki nodi zote za mbegu zinazopatikana katika kikundi cha kushiriki, na kuvuta kutoka kwa nodi nyingi za mbegu kwa wakati mmoja:

```bash
# Vuta kikundi kizima cha kushiriki seti ya data hadi ./local_data
sync0 pull dataset:// ./local_data -p <nodi address>

# Usawazishaji wa chaguzi nyingi: wakati huo huo vuta hati za faili moja/readme.md na saraka ndogo za mali/ikoni, src/core
sync0 pull dataset://docs/readme.md,assets/icons,src/core ./local_data -p <anwani ya nodi>
```

### 4. Pakia faili au saraka kwa kikundi cha kushiriki (`sync0 push`)

Washiriki walio na ruhusa za upakiaji (`rw` au `admin`) wanaweza kupakia faili za ndani au saraka kwa vikundi vya kushiriki vya mbali:

```bash
# Wanachama walio na ruhusa ya kupakia saraka ya mizizi: Pakia moja kwa moja kwenye saraka ya mizizi iliyoshirikiwa
sync0 push team-vault:// ./new_release -p <anwani ya nodi>

# Wanachama pekee walioidhinishwa kwa kiambishi awali maalum (kama vile vipakiaji/bob): pakia kwenye kiambishi awali cha njia kinachoruhusiwa
sync0 push team-vault://uploads/bob/report.pdf ./report.pdf -p <anwani ya nodi>
```

### 5. Jisajili ili kusawazisha na kupanda mbegu kiotomatiki (`sync0 sub`)

Baada ya kujiandikisha kwa kikundi cha kushiriki cha nodi fulani, seva ya ndani ni mteja anayepokea na seva mpya ya mbegu; faili ya nodi ya chanzo inaposasishwa, inasawazishwa kiotomatiki na hutoa upakuaji wa chunked kwa nodi zingine:

```bash
# Jiandikishe kwa kikundi kizima cha kushiriki na upate mbegu kiotomatiki
sync0 seti ndogo ya data:// ./local_mirror -p <anwani ya nodi>

# Chaguo nyingi pekee ili kujiandikisha kwa faili na saraka zilizobainishwa na kuziweka mbegu
sync0 seti ndogo ya data://models/llama.onnx,configs ./local_mirror -p <anwani ya nodi>
```

### 6. Usawazishaji wa rika wa vikundi vya mashine nyingi (`sync0 group`)

Mashine nyingi hujiunga na kikundi kimoja cha kushiriki, kila moja hugundua nodi za mbegu zinazopatikana kiotomatiki na kudumisha ulandanishi wa njia mbili (inaauni ulandanishi kamili au uteuzi mwingi wa faili na saraka):

```bash
# Jiunge kikamilifu na kikundi na usawazishaji wa njia mbili
sync0 team-docs:// ./docs -p <anwani ya nodi>

# Saraka ya wal na faili ya meta.json pekee katika kikundi cha ulandanishi chenye teule nyingi
sync0 group team-docs://wal,meta.json ./docs -p <anwani ya nodi>
```

### 7. Usambazaji wa uhakika kwa uhakika kati ya mashine mbili (`send` / `recv`)

Hakuna haja ya kusanidi kikundi mapema, na unaweza kuhamisha faili au saraka moja kwa moja kati ya mashine mbili kwa muda:

```bash
# Ufuatiliaji wa mpokeaji
sync0 recv 0.0.0.0:9000 ./backup

# Mwisho wa kutuma huhamisha faili au saraka moja kwa moja (hiari --delete kioo ili kusafisha faili nyingi, --rate kikomo cha kasi)
sync0 send ./project 192.168.1.10:9000
```

### 8. Kubadilisha lugha nyingi

`sync0` inaauni lugha 75, na hubadilika kiotomatiki kulingana na mazingira ya lugha ya mfumo kwa chaguo-msingi; inaweza pia kubainishwa mwenyewe kupitia `SYNC0_LANG`:

```bash
SYNC0_LANG=en sync0 --help
SYNC0_LANG=ja sync0 --help
SYNC0_LANG=de sync0 --help
```