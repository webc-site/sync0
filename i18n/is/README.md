# sync0

`sync0` er jafningi-til-jafningi (P2P) skráadeilingar- og samstillingartæki fyrir margar vélar sem keppir við Resilio Sync.

Hver vél er bæði þjónn og viðskiptavinur: hvaða hnút sem er getur birt marga samnýtingarhópa. Eftir að aðrir hnútar sameinast geta þeir sjálfkrafa fengið tiltæka fræhnúta frá jafningjaþjóninum og dregið þá samtímis frá mörgum aðilum. Eftir að samstillingu er lokið verða fræ sjálfkrafa búin til fyrir fleiri hnúta til að flýta fyrir niðurhali.

## Kjarnaatriði

- **Allir eru bæði netþjónar og viðskiptavinir**: Sambærilegt við Resilio Sync, engin miðlæg miðlara háð. Hvaða hnút sem er getur búið til marga skráadeilingarhópa og getur einnig gengið í hópa annarra; hnútar skiptast sjálfkrafa á og fá tiltæka fræhnúta til að ná samhliða togi og rauntíma samstillingu margra véla.
- **IP-laust hnútfang**: Sjálfgefið er að hnútalykillinn og `base64url` kóðaða `iroh` vistfangið eru sjálfkrafa til í `~/.config/sync0` án þess að þurfa að muna eða afhjúpa opinbera IP-tölu.
- **Margvalssamstilling á eftirspurn**: Styður samstillingu á öllum samnýtingarhópnum og styður einnig samstillingu á eftirspurn með því að velja margar skrár eða undirmöppur á sama tíma.
- **Stifveldisheimildir hópmeðlima og upphleðsluskráastjórnun**: Styður skrifvarinn (`ro`), les- og skrifhleðslu (`rw`) og stjórnun (`admin`) heimildir og getur fyrir sig tilgreint slóðaforskeyti sem hverjum meðlimi er heimilt að hlaða upp (sjálfgefið er upphleðsla í rótarskrá hlutdeildarinnar leyfð).
- **Fullhraði og stigvaxandi samstilling yfir netkerfi**: Samhliða sending stórra skráa, samansöfnun stórra lítilla skráa í minni og aðeins samstilltar gagnablokkir þegar skrám er breytt að hluta.

## Hraðasamanburður við rsync

### Transoceanic net eins uppspretta raunmæling (Kína → Bandaríkin, seinkun 260 ms)

| Sendingaratburðarás | Gagnastærð | fyrirmynd | `sync0` | `rsync -az` | Auka margfaldara |
| :--- | :---: | :---: | :---: | :---: | :---: |
| 500 litlar skrár | 1.91 MB | Full samstilling | 18.84 Mbps | 5.32 Mbps | 3.5x |
| 500 litlar skrár | 1.91 MB | Stigvaxandi samstilling | 27.15 Mbps | 3.83 Mbps | 7.1x |
| Kóði og geymslupakkar (`.tar`) | 10.0 MB | Full samstilling | 139.70 Mbps | 24.97 Mbps | 5.6x |
| Kóði og geymslupakkar (`.tar`) | 10.0 MB | Stigvaxandi samstilling | 83.38 Mbps | 26.02 Mbps | 3.2x |
| Myndbandsskrá (`.mp4`) | 15.0 MB | Full samstilling | 54.06 Mbps | 27.04 Mbps | 2.0x |
| Myndbandsskrá (`.mp4`) | 15.0 MB | Stigvaxandi samstilling | 216.83 Mbps | 21.51 Mbps | 10.1x |

### Transoceanic net P2P multi-node pull test (Kína → Bandaríkin tvöfaldir hnútar, seinkun 260 ms)

Mældur samanburður á samhliða röndóttu fræhnúti með mörgum uppruna:

| Sendingaratburðarás | Gagnastærð | Sáning á einum hnút | Tveggja hnúta P2P samhliða sáning | Auka margfaldara |
| :--- | :---: | :---: | :---: | :---: |
| 500 litlar skrár | 1.97 MB | 5.28 Mbps | 7.87 Mbps | 1.5x |
| Kóði og geymslupakkar (`.tar`) | 30.0 MB | 56.35 Mbps | 83.17 Mbps | 1.5x |
| Myndbandsskrá (`.mp4`) | 30.0 MB | 27.00 Mbps | 39.46 Mbps | 1.5x |

### Raunveruleg mæling á sérstakri háhraðalínu þvert á meginlandið (Evrópa → Norður-Ameríka, seinkun 86 ms)

| Sendingaratburðarás | Gagnastærð | `sync0` | `rsync` | Auka margfaldara |
| :--- | :---: | :---: | :---: | :---: |
| 10.000 litlar skrár | 77.6 MB | 688 Mbps | 128 Mbps | 5.4x |
| Kóði og geymslupakkar (`.tar`) | 512 MB | 2111 Mbps | 438 Mbps | 4.8x |
| Myndbandsskrá (`.mp4`) | 512 MB | 556 Mbps | 338 Mbps | 1.6x |
| Stigvaxandi samstilling (breyta 1% innihaldi) | 512 MB | 34133 Mbps | 2214 Mbps | 15.4x |

## Settu upp

### Aðferð 1: Settu upp í gegnum farm

```bash
cargo install sync0
```

### Aðferð 2: Hlaða niður forsamsettum tvíþættum

Farðu í [GitHub Releases](https://github.com/webc-site/sync0/releases) til að hlaða niður `.tar.xz` þjappaða pakkanum fyrir samsvarandi kerfi (styður Linux, macOS, Windows, Android, FreeBSD, NetBSD), pakkaðu niður og settu hann í `PATH` möppuna:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Byrjaðu fljótt

`sync0` notar `~/.config/sync0` sem stillingaskrá sjálfgefið (ef þú þarft að skipta yfir í aðrar möppur geturðu bætt `-c <directory>` við).

### 1. Skoðaðu heimilisfang staðbundins hnúts og nafnspjaldsstillingu

```bash
# Skoðaðu heimilisfang hnútsins (lykillinn er sjálfkrafa búinn til í ~/.config/sync0 í fyrsta skipti)
sync0 id

# Búðu til persónulegt hnúta nafnspjald (styður notendanafn, borg, tölvupóst, vefsíðu og undirskrift innan 256 orða)
sync0 profile -u alice --region SHA --email alice@example.com --url https://example.com -b "Shanghai node"
```

### 2. Birta marga skráadeilingarhópa og stilltu heimildir hópmeðlima (`.nt` Stillingar)

`sync0` er stillt á [NestedText](https://nestedtext.org) (`.nt`) sniði. Hver notandi hefur sérstaka `.nt` stillingarskrá. Þú getur tilgreint notandaprófílskrána í gegnum `users_dir` í `config.nt` (sem inniheldur sjálfkrafa allar `.nt` skrár í þessari möppu). Bæði notandinn og leiðarforskeytið sem leyfilegt er að hlaða upp nota óraðaðan lista (`-`). **Þegar forskeytið fyrir upphleðsluslóð er ekki fyllt út er sjálfgefið að hlaða upp í rótarskrá hlutdeildarinnar**.

1. Búðu til aðskildar `.nt` stillingarskrár fyrir hvern meðlim í `~/.config/sync0/users/` möppunni:

`~/.config/sync0/users/alice.nt` (skrifvarinn meðlimur, takmörkuð verð `80MB`, `rate` styður `GB`, `MB`, `KB` einingar):

```yaml
id: 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c
perm: ro
rate: 80MB
```

`~/.config/sync0/users/bob.nt` (takmörkuð upphleðslumeðlimir: aðeins heimilt að hlaða upp í `uploads/bob` og `shared/common` möppur):

```yaml
id: 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f
perm: rw
write_prefix:
  - uploads/bob
  - shared/common
```

`~/.config/sync0/users/charlie.nt` (Sjálfgefið upphleðsla meðlima: Þegar `write_prefix` er sleppt er sjálfgefið að hlaða inn í rótarskrá hlutdeildarinnar):

```yaml
id: 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
perm: rw
```

2. Stilltu notendaskrána og marga skráadeilingarhópa í `~/.config/sync0/config.nt`:

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

Byrjaðu að deila þjónustu:

```bash
# Bein ræsing (hleður sjálfkrafa öllum .nt notendastillingum í ~/.config/sync0/config.nt og users_dir möppur)
sync0 share --region SHA

# Eða birtu fljótt marga samnýtingarhópa og heimila hópmeðlimi beint í gegnum skipanalínuna
sync0 share dataset:./data team-vault:./vault \
  --auth \
  --allow-ro 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c \
  --allow-rw 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f:uploads/bob,shared/common \
  --allow-rw 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
```

### 3. Dragðu allt efni eða veldu margar skrár og möppur (`sync0 pull`)

Tengstu við hvaða jafningjahnút sem er til að fá sjálfkrafa alla tiltæka fræhnúta í samnýtingarhópnum og dragðu úr mörgum fræhnútum samtímis:

```bash
# Dragðu allan gagnasafnsdeilingarhópinn að staðbundnu ./local_data
sync0 pull dataset:// ./local_data -p <node address>

# Fjölvals samstilling: Dragðu samtímis staku skrána docs/readme.md og undirskrár eignir/tákn, src/core
sync0 pull dataset://docs/readme.md,assets/icons,src/core ./local_data -p <node address>
```

### 4. Hladdu upp skrá eða möppu í samnýtingarhóp (`sync0 push`)

Meðlimir með upphleðsluheimildir (`rw` eða `admin`) geta hlaðið upp staðbundnum skrám eða möppum í fjarsamnýtingarhópa:

```bash
# Meðlimir með heimild til að hlaða upp rótarskrá: Hladdu beint inn í sameiginlegu rótarskrána
sync0 push team-vault:// ./new_release -p <node address>

# Aðeins meðlimir sem hafa heimild fyrir tilteknu forskeyti (eins og upphleðslur/bob): hlaða upp á leyfilegt forskeyti
sync0 push team-vault://uploads/bob/report.pdf ./report.pdf -p <node address>
```

### 5. Gerast áskrifandi að sjálfvirkri samstillingu og sáningu (`sync0 sub`)

Eftir að hafa gerst áskrifandi að samnýtingarhópi ákveðins hnúts er staðbundinn þjónn bæði móttökuþjónn og nýr sáningarþjónn; þegar frumhnútaskráin er uppfærð er hún sjálfkrafa samstillt og veitir niðurhal í kubbum fyrir aðra hnúta:

```bash
# Gerast áskrifandi að öllum deilingarhópnum og sjáðu sjálfkrafa
sync0 undirgagnasafn:// ./local_mirror -p <node address>

# Aðeins fjölvalið til að gerast áskrifandi að tilgreindum skrám og undirmöppum og sjá þær
sync0 undirgagnasafn://models/llama.onnx,configs ./local_mirror -p <node address>
```

### 6. Jafningasamstilling margra véla hópa (`sync0 group`)

Margar vélar ganga í sama samnýtingarhópinn, hver önnur uppgötvar sjálfkrafa tiltæka fræhnúta og viðheldur tvíhliða samstillingu (styður fulla samstillingu eða margfalt val á skrám og möppum):

```bash
# Fullkomlega ganga í hópinn og tvíhliða samstillingu
sync0 group team-docs:// ./docs -p <node address>

# Aðeins wal skráin og meta.json skráin í fjölvals samstillingarhópnum
sync0 group team-docs://wal,meta.json ./docs -p <node address>
```

### 7. Bein sending frá punkti á milli tveggja véla (`send` / `recv`)

Það er engin þörf á að forstilla hóp og þú getur tímabundið flutt skrár eða möppur beint á milli tveggja véla:

```bash
# Vöktun móttakara
sync0 recv 0.0.0.0:9000 ./backup

# Sendiendinn flytur skrár eða möppur beint (valfrjálst --eyða spegli til að hreinsa upp umfram skrár, --hraðatakmarkanir)
sync0 send ./project 192.168.1.10:9000
```

### 8. Að skipta á mörgum tungumálum

`sync0` styður 75 tungumál og skiptir sjálfkrafa í samræmi við tungumálaumhverfi kerfisins sjálfgefið; það er líka hægt að tilgreina það handvirkt í gegnum `SYNC0_LANG`:

```bash
SYNC0_LANG=en sync0 --help
SYNC0_LANG=ja sync0 --help
SYNC0_LANG=de sync0 --help
```