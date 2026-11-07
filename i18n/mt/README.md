# sync0

`sync0` hija għodda tal-linja tal-kmand għat-trasferiment ta' fajls fuq in-netwerks, is-sinkronizzazzjoni ta' direttorji b'mod inkrementali, u t-tqassim tad-dejta fl-istess ħin lil magni multipli.

## Intenzjoni tad-disinn oriġinali

F'kmamar tal-magni, pajjiżi, jew ambjenti b'dewmien tan-netwerk u telf ta 'pakketti, għodod bħal `rsync` u `scp` huma bbażati fuq konferma ta' sekwenza ta 'konnessjoni waħda, li tillimita l-utilizzazzjoni tal-bandwidth; meta magni multipli jiġbdu data mill-istess server fl-istess ħin, il-karta tan-netwerk tas-server tas-sors tista 'faċilment issir ostaklu.

L-għan ta' `sync0` huwa li ssolvi żewġ problemi:

1. Ħalli s-sinkronizzazzjoni tal-fajl u tad-direttorju bejn iż-żewġ magni taħdem bandwidth fiżiku sħiħ, u tittrasferixxi biss il-parti mibdula meta l-fajl jiġi parzjalment modifikat.
2. Magni multipli jistgħu jittrasferixxu blokki tad-dejta lil xulxin meta jissinkronizzaw l-istess direttorju, inaqqsu t-tagħbija fuq is-server tas-sors u jqassru l-ħin ġenerali tas-sinkronizzazzjoni.

## Tqabbil tal-veloċità ma 'rsync

Ambjent tat-test: Nodu Ewropew $\leftrightarrow$ Nodu ta' l-Amerika ta' Fuq, dewmien ta' vjaġġ bir-ritorn ta' 86 ms, li jkejjel il-ħin kollu inkluż l-iskannjar tad-direttorju, il-paragun tad-deduplikazzjoni, it-trażmissjoni tan-netwerk u l-verifika tat-tqegħid tad-disk:

| Xenarju ta' trażmissjoni | Daqs tad-dejta | `sync0` | `rsync` | multipli |
| :--- | :---: | :---: | :---: | :---: |
| 10,000 fajl żgħir | 77.6 MB | 0.90 s (688 Mbps) | 4.86 s (128 Mbps) | 5.4x |
| Pakketti ta' kodiċi u arkivju (`.tar`) | 512 MB | 1.94 s (2111 Mbps) | 9.36 s (438 Mbps) | 4.8x |
| Fajl tal-vidjo (`.mp4`) | 512 MB | 7.36 s (556 Mbps) | 12.11 s (338 Mbps) | 1.6x |
| Sinkronizzazzjoni inkrementali (immodifika kontenut ta' 1%) | 512 MB | 0.12 s | 1.85 s | 15.4x |

## Metodi ta 'titjib tal-prestazzjoni

- **Trażmissjoni konkorrenti UDP**: Ibgħat blokki tad-dejta permezz tal-UDP, u terġa 'trażmetti biss frammenti mitlufa biex tevita telf ta' pakkett wieħed li jimblokka t-trasferimenti ta 'fajls sussegwenti.
- **Segmentazzjoni u deduplikazzjoni tal-kontenut**: Aqsam il-fajl fi blokki tad-dejta skont il-kontenut u kkalkula l-valur tal-kontroll. L-istess kontenut li diġà qed jirċievi jerġa' jintuża direttament lokalment mingħajr ma jiġi duplikat in-netwerk.
- **Aġġusta awtomatikament il-kompressjoni skont il-veloċità tan-netwerk**: Ixgħel il-kompressjoni għal kontenut kompressibbli bħal test u kodiċi; awtomatikament aqbeż il-kompressjoni għal vidjows, stampi, pakketti kkompressati jew taħt LAN b'veloċità għolja biex jitnaqqas il-konsum tas-CPU.
- **Ġbid fil-qrib b'ħafna nodi**: Meta magni multipli jiġbdu l-istess direttorju, il-blokki tad-dejta jinġibdu fl-istess ħin minn nodi mniżżla fl-istess belt jew b'latenza aktar baxxa.

## Installa

### Metodu 1: Installa permezz tal-Merkanzija

```bash
cargo install sync0
```

### Metodu 2: Niżżel binarji kkumpilati minn qabel

Mur [GitHub Releases](https://github.com/webc-site/sync0/releases) biex tniżżel il-`.tar.xz` pakkett kompressat għas-sistema korrispondenti (jappoġġja Linux x86_64/ARM64, macOS Apple Silicon/Intel, Windows x64/ARM64, Android ARM64/x86_64, FreeBSD, NetBSD). Wara li tħoll, poġġi `sync0` fid-direttorju `PATH`:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Gwida għall-Utent

### 1. Ittrasferixxi direttament fajls jew direttorji bejn żewġ magni

Ibda l-monitoraġġ fuq il-magna li tirċievi u speċifika t-triq tar-riċeviment u l-iffrankar:

```bash
sync0 recv 0.0.0.0:9000 ./backup
```

Ibgħat fajl wieħed jew direttorju sħiħ mill-magna li tibgħat:

```bash
sync0 send ./project 192.168.1.10:9000
```

Parametri komunement użati:

```bash
# Ħassar fajls żejda li jeżistu fit-tarf li jirċievi iżda mhux fit-tarf li jibgħat (sinkronizzazzjoni tal-mera)
sync0 send ./project 192.168.1.10:9000 --delete

# Limitu tar-rata ta' mibgħuta (unità: bytes/sekonda, pereżempju 50 MB/s)
sync0 send ./project 192.168.1.10:9000 --rate 50000000

# Modalità siekta (l-ebda bar tal-progress murija)
sync0 send ./project 192.168.1.10:9000 -q
```

### 2. Il-pubblikazzjoni tad-direttorji kondiviżi u l-ġbid minn nodi multipli

Ippubblika direttorju kondiviż (imsemmi `dataset`) fuq is-server tal-oriġini:

```bash
sync0 share dataset ./data --region SHA
```

Iġbed id-direttorju kondiviż fuq magni oħra (tista 'timla indirizzi multipli ta' nodi fl-istess ħin, separati b'virgoli biex tiġbed fl-istess ħin):

```bash
sync0 pull 10.0.0.1:9000,10.0.0.2:9000 dataset ./local_data
```

Iġbed biss ċertu subdirettorju fid-direttorju kondiviż:

```bash
sync0 pull 10.0.0.1:9000 dataset ./local_data --subpath 2026/logs
```

Imbotta l-bidliet lokali lura fid-direttorju kondiviż remot:

```bash
sync0 push 10.0.0.1:9000 dataset ./local_data
```

### 3. Abbona għall-aġġornamenti awtomatiċi tal-katalgu

Abbona għad-direttorju kondiviż tan-node tas-sors. Meta fajl jiġi miżjud jew modifikat fid-direttorju tas-sors, ikun awtomatikament sinkronizzat mal-kompjuter lokali. Fl-istess ħin, il-magna lokali awtomatikament isservi bħala node għal magni oħra biex jiġbdu:

```bash
sync0 sub 10.0.0.1:9000 dataset ./local_mirror
```

### 4. Sinkronizzazzjoni f'żewġ direzzjonijiet ta 'gruppi b'ħafna magni

Magni multipli jingħaqdu mal-istess isem tal-grupp, u l-bidliet fi kwalunkwe direttorju tan-nodi jiġu awtomatikament sinkronizzati ma' nodi oħra fil-grupp:

```bash
sync0 group team-docs ./docs 10.0.0.1:9000,10.0.0.2:9000
```

### 5. Identità tan-node u business card personali

Kull nodu jappoġġja l-issettjar tal-isem tal-utent, il-belt, l-email, il-websajt u firma personalizzata (massimu 256 karattru), u jgħaddi l-awtentikazzjoni tal-firma taċ-ċavetta privata lokali:

```bash
# Ara l-ID taċ-ċavetta pubblika lokali (speċifika d-direttorju tal-konfigurazzjoni biex tippersisti ċ-ċavetta)
sync0 id -c ~/.config/sync0

# Iġġenera u uri karti tan-negozju tan-nodi b'firem personalizzati
sync0 profile -c ~/.config/sync0 \
  -u alice \
  --region SHA \
  --email alice@example.com \
  --url https://example.com \
  -b "Shanghai backup node"
```

`share`, `pull`, `sub`, `group` jistgħu wkoll jiġu varati b'dawn il-parametri tal-profil:

```bash
sync0 share dataset ./data -c ~/.config/sync0 -u alice --region SHA -b "Shanghai backup node"
```

### 6. Qlib b'ħafna lingwi

`sync0` Jappoġġja 75 lingwa. B'mod awtomatiku, l-għajnuna tal-linja tal-kmand u l-promps fil-lingwa korrispondenti jintwerew awtomatikament skont l-ambjent tal-lingwa tas-sistema operattiva; jekk il-lingwa tas-sistema mhix fil-lista appoġġjata, l-Ingliż jintuża awtomatikament.

Biex tispeċifika l-lingwa manwalment, issettja l-varjabbli ambjentali `SYNC0_LANG`:

```bash
SYNC0_LANG=en sync0 --help
SYNC0_LANG=ja sync0 --help
SYNC0_LANG=de sync0 --help
```