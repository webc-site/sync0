# sync0

`sync0` ni zana ya mstari wa amri ya kuhamisha faili kwenye mitandao, kusawazisha saraka kwa kasi, na kusambaza data kwa mashine nyingi kwa wakati mmoja.

## Nia ya asili ya kubuni

Katika vyumba vya mashine mbalimbali, nchi au mazingira yenye ucheleweshaji wa mtandao na upotevu wa pakiti, zana kama vile `rsync` na `scp` zinatokana na uthibitishaji wa mfuatano wa muunganisho mmoja, ambao huzuia matumizi ya kipimo data; wakati mashine nyingi huvuta data kutoka kwa seva moja kwa wakati mmoja, kadi ya mtandao ya seva chanzo inaweza kuwa kizuizi kwa urahisi.

Lengo la `sync0` ni kutatua matatizo mawili:

1. Ruhusu ulandanishi wa faili na saraka kati ya mashine hizo mbili ziendeshe kipimo data kamili, na uhamishe tu sehemu iliyobadilishwa wakati faili imerekebishwa kwa sehemu.
2. Mashine nyingi zinaweza kuhamisha vizuizi vya data kwa kila nyingine wakati wa kusawazisha saraka sawa, kupunguza upakiaji kwenye seva chanzo na kufupisha muda wa jumla wa usawazishaji.

## Ulinganisho wa kasi na rsync

Mazingira ya majaribio: nodi ya Ulaya $\leftrightarrow$ nodi ya Amerika Kaskazini, kucheleweshwa kwa safari ya kwenda na kurudi 86 ms, kupima muda unaotumia muda mwingi ikiwa ni pamoja na kuchanganua saraka, ulinganisho wa kurudia, utumaji mtandao na uthibitishaji wa uwekaji wa diski:

| Hali ya maambukizi | Ukubwa wa data | `sync0` | `rsync` | nyingi |
| :--- | :---: | :---: | :---: | :---: |
| 10,000 faili ndogo | 77.6 MB | 0.90 s (688 Mbps) | 4.86 s (128 Mbps) | 5.4x |
| Msimbo na vifurushi vya kumbukumbu (`.tar`) | 512 MB | 1.94 s (2111 Mbps) | 9.36 s (438 Mbps) | 4.8x |
| Faili ya video (`.mp4`) | 512 MB | 7.36 s (556 Mbps) | 12.11 s (338 Mbps) | 1.6x |
| Usawazishaji unaoongezeka (rekebisha maudhui 1%) | 512 MB | 0.12 s | 1.85 s | 15.4x |

## Mbinu za kuboresha utendaji

- **Utumaji wa wakati mmoja wa UDP**: Tuma vizuizi vya data kupitia UDP, na utume tena vipande vilivyopotea ili kuepuka upotevu wa pakiti moja kuzuia uhamishaji wa faili unaofuata.
- **Mgawanyo wa maudhui na utenganishaji**: Gawanya faili katika vizuizi vya data kulingana na maudhui na ukokote thamani ya hundi. Maudhui yale yale ambayo tayari yapo kwenye sehemu ya kupokea yanatumiwa tena moja kwa moja ndani ya nchi bila kunakili mtandao.
- **Rekebisha mbano kiotomatiki kulingana na kasi ya mtandao**: Washa mbano kwa maudhui yanayoweza kubana kama vile maandishi na msimbo; ruka kiotomatiki mgandamizo wa video, picha, vifurushi vilivyobanwa au chini ya LAN ya kasi ya juu ili kupunguza matumizi ya CPU.
- **Njia nyingi za kuvuta zilizo karibu**: Wakati mashine nyingi zikivuta saraka sawa, vizuizi vya data vitatolewa kwa wakati mmoja kutoka kwa vifundo vilivyopakuliwa katika jiji moja au kwa muda wa chini wa kusubiri.

## Sakinisha

### Njia ya 1: Sakinisha kupitia Cargo

```bash
cargo install sync0
```

### Njia ya 2: Pakua jozi zilizokusanywa mapema

Nenda kwa [Matoleo ya GitHub](https://github.com/webc-site/sync0/releases) ili kupakua `.tar.xz` kifurushi kilichobanwa cha mfumo unaolingana (inaauni Linux x86_64/ARM64, macOS Apple Silicon/Intel, Windows x64/ARM64, Android ARM64/x86_64, FreeBSD, NetBSD). Baada ya kufungua zipu, weka `sync0` kwenye `PATH` saraka:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Mwongozo wa Mtumiaji

### 1. Hamisha faili au saraka moja kwa moja kati ya mashine mbili

Anza ufuatiliaji kwenye mashine ya kupokea na taja njia ya kupokea na kuokoa:

```bash
sync0 recv 0.0.0.0:9000 ./backup
```

Tuma faili moja au saraka nzima kutoka kwa mashine ya kutuma:

```bash
sync0 send ./project 192.168.1.10:9000
```

Vigezo vinavyotumika kawaida:

```bash
# Futa faili zisizohitajika ambazo zipo kwenye sehemu ya kupokea lakini sio kwenye mwisho wa kutuma (usawazishaji wa kioo)
sync0 send ./project 192.168.1.10:9000 --delete

# Punguza kiwango cha utumaji (kipimo: baiti/sekunde, kwa mfano 50 MB/s)
sync0 send ./project 192.168.1.10:9000 --rate 50000000

# Hali ya kimya (hakuna upau wa maendeleo unaoonyeshwa)
sync0 send ./project 192.168.1.10:9000 -q
```

### 2. Kuchapisha saraka zilizoshirikiwa na kuvuta kutoka nodi nyingi

Chapisha saraka iliyoshirikiwa (inayoitwa `dataset`) kwenye seva asili:

```bash
sync0 share dataset ./data --region SHA
```

Vuta saraka iliyoshirikiwa kwenye mashine zingine (unaweza kujaza anwani nyingi za nodi kwa wakati mmoja, ikitenganishwa na koma ili kuvuta wakati huo huo):

```bash
sync0 pull 10.0.0.1:9000,10.0.0.2:9000 dataset ./local_data
```

Vuta saraka ndogo tu kwenye saraka iliyoshirikiwa:

```bash
sync0 pull 10.0.0.1:9000 dataset ./local_data --subpath 2026/logs
```

Sukuma mabadiliko ya ndani kurudi kwenye saraka iliyoshirikiwa ya mbali:

```bash
sync0 push 10.0.0.1:9000 dataset ./local_data
```

### 3. Jisajili ili upate masasisho ya kiotomatiki ya katalogi

Jiandikishe kwa saraka iliyoshirikiwa ya nodi ya chanzo. Faili inapoongezwa au kurekebishwa kwenye saraka ya chanzo, itasawazishwa kiotomatiki kwenye kompyuta ya ndani. Wakati huo huo, mashine ya ndani itatumika kiatomati kama nodi ya mashine zingine kuvuta:

```bash
sync0 sub 10.0.0.1:9000 dataset ./local_mirror
```

### 4. Usawazishaji wa njia mbili wa vikundi vya mashine nyingi

Mashine nyingi hujiunga na jina moja la kikundi, na mabadiliko katika saraka ya nodi yoyote yatasawazishwa kiotomatiki kwa nodi zingine kwenye kikundi:

```bash
sync0 group team-docs ./docs 10.0.0.1:9000,10.0.0.2:9000
```

### 5. Utambulisho wa nodi na kadi ya biashara ya kibinafsi

Kila nodi inaauni kuweka jina la mtumiaji, jiji, barua pepe, tovuti na sahihi ya kibinafsi (idadi ya juu zaidi ya herufi 256), na kupitisha uthibitishaji wa saini ya ufunguo wa kibinafsi wa karibu:

```bash
# Tazama kitambulisho cha ufunguo wa umma (taja saraka ya usanidi ili kuendelea na ufunguo)
sync0 id -c ~/.config/sync0

# Tengeneza na uonyeshe kadi za biashara za nodi zilizo na saini za kibinafsi
sync0 profile -c ~/.config/sync0 \
  -u alice \
  --region SHA \
  --email alice@example.com \
  --url https://example.com \
  -b "Shanghai backup node"
```

`share`, `pull`, `sub`, `group` pia inaweza kuzinduliwa kwa vigezo hivi vya wasifu:

```bash
sync0 share dataset ./data -c ~/.config/sync0 -u alice --region SHA -b "Shanghai backup node"
```

### 6. Kubadilisha lugha nyingi

`sync0` Inaauni lugha 75. Kwa chaguo-msingi, usaidizi wa mstari wa amri na vidokezo katika lugha inayofanana huonyeshwa moja kwa moja kulingana na mazingira ya lugha ya mfumo wa uendeshaji; ikiwa lugha ya mfumo haiko katika orodha inayotumika, Kiingereza hutumiwa kwa chaguo-msingi.

Ili kubainisha lugha wewe mwenyewe, weka utofauti wa mazingira `SYNC0_LANG`:

```bash
SYNC0_LANG=en sync0 --help
SYNC0_LANG=ja sync0 --help
SYNC0_LANG=de sync0 --help
```