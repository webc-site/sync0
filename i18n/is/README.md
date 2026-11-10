# sync0

`sync0` er dreifð P2P skráaskipti, rauntíma samstillingu og dreifingartæki með mörgum uppsprettum sem notað er til að koma í stað `rsync` - mældur hraði þvermeginlands WAN er allt að `rsync` **5,89×** (einspunkts rúmfræðilegur meðalhraði **2,67×-** 1,67×-** uppspretta meðalhraði ** og P2P tvískiptur **, P2P tvískiptur meðalhraði .×. P2P hröðun á hlekki með mikilli leynd yfir haf er allt að **7,68×** (Full-link 24-sviðsmynd alhliða rúmfræðilegur meðalhraði **2,56×**).

Hver vél er bæði þjónn og viðskiptavinur: það er engin þörf á miðlægum netþjóni eða almennu netfangi, sjálfvirk gatagangur er byggður á dreifðu netfangi almenningslykils og hún styður beina sendingu á milli tveggja véla, hýsingu fyrir fjöldasamnýtingu hópheimilda, margfalt val á undirmöppum eftir beiðni, stigvaxandi samstillingu í rauntíma í millisekúndu og samhliða P2P fjölmiðlunarhröðun.

## Settu upp

### Aðferð 1: Uppsetning farms

```bash
cargo install sync0
```

### Aðferð 2: Hlaða niður forsamsettum tvíþættum

Farðu í [GitHub Releases](https://github.com/webc-site/sync0/releases) til að hlaða niður `.tar.xz` þjappaða pakkanum sem samsvarar kerfinu (styður Linux, macOS, Windows, Android, iOS, FreeBSD), pakkaðu niður og settu í `PATH`:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Byrjaðu fljótt

### 1. Bein sending með núllstillingu milli tveggja véla (`send` / `recv`)

Engin þörf á að stilla samnýtingarhóp, flytja skrár eða möppur beint á milli tveggja véla á miklum hraða:

```bash
# Móttakandi hlustar á portið og skrifar á ./backup
sync0 recv 0.0.0.0:9000 ./backup

# Bein sending frá sendanda (valfrjálst --eyða spegli til að hreinsa upp umfram skrár, --verð 50MB hraðamörk)
sync0 send ./project 192.168.1.10:9000
```

### 2. Færsludeilingu og eftirspurn draga/hlaða upp (`share` / `pull` / `push`)

```bash
# Miðlari: Birtu samnýtingarhópinn með einni skipunarlínu (útgefðu IP-lausa iroh hnút heimilisfangið eftir ræsingu)
sync0 share dataset:./data

# Viðskiptavinur: Dragðu allan gagnasafnsdeilingarhópinn að staðbundnu ./local_data
sync0 pull dataset:// ./local_data -p <node address>

# Viðskiptavinur: Dragðu mörg val eftir beiðni (samstilltu aðeins tilgreindar stakar skrár og undirmöppur)
sync0 pull dataset://docs/readme.md,assets/icons,src/core ./local_data -p <node address>

# Viðskiptavinur: Hladdu upp skrám eða möppum í viðurkennda samnýtingarhópa
sync0 push dataset://releases/v1 ./build_output -p <node address>
```

### 3. Rauntímaáskriftarsáning og tvíhliða samstilling með mörgum vélum (`sub` / `group`)

Eftir að hafa komið á langri tengingu, ef einhver skrá **að bæta við, breyta, eyða, breyta heimildum eða breytingatíma** á sér stað á ytri endanum, verður staðbundin möppu uppfærð samstillt á millisekúndum í rauntíma (nýjar/breyttar stórar skrár munu aðeins draga mismunablokkirnar og uppfæra þær á sínum stað, eyða skrám mun sjálfkrafa hreinsa upp staðbundnar skrár og tómar gagnaskrár, og aðeins breyta gögnum beint á millisekúndum)

- **`sync0 sub`(Einátta rauntímaspeglun + sjálfvirk sáning)**: Staðbundinn rauntími fylgir viðbótum, eyðileggingum og breytingum á deilingarendanum og þjónar sjálfkrafa sem P2P fræuppspretta til að veita samhliða hröðun fyrir aðra toghnúta.
- **`sync0 group`(Multi-machine peer-to-peer tvíhliða rauntíma samstillingu)**: Ef skrám er bætt við, þeim eytt eða þeim breytt á hvaða vél sem er í hópnum verða allir aðrir nethnútar uppfærðir samstillt í rauntíma og þjóna sem fræ fyrir hvern annan.

```bash
# Gerast áskrifandi að öllum deilingarhópnum: fjarlægar viðbætur, eyðingar og breytingar eru samstilltar á staðnum í rauntíma og sjálfkrafa settar
sync0 undirgagnasafn:// ./local_mirror -p <node address>

# Fjölvalsáskrift á eftirspurn: samstilltu aðeins og sáðu tilgreindar stakar skrár og undirmöppur í rauntíma
sync0 undirgagnasafn://models/llama.onnx,configs ./local_mirror -p <node address>

# Margvéla jafningjahópur tvíhliða rauntíma samstillingu (styður einnig fullar eða kommu fjölvalsleiðir)
sync0 group team-docs:// ./docs -p <node address>
```

### 4. Samskipun hóps og leyfis meðlima fyrir fjöldeilingu (`.nt` stillingar)

Sjálfgefin stillingarskrá er `~/.config/sync0` (hægt að skipta um með `-c <directory>`), með því að nota lægsta [NestedText](https://nestedtext.org) (`.nt`) sniðið:
- **Styður við að takmarka lestur almenningsnets (aðeins viðurkenndir notendur geta lesið)**: Svo lengi sem samnýtingarhópurinn er stilltur með `users:` meðlimalista (eða skipanalínan tilgreinir `--allow-r` / `--allow-rw` / `--allow-w` / `--auth`), kveikir kerfið **sjálfkrafa á nafnlausu neti (`users` eru opinberir og skrifvarnir sjálfgefið (ef þú vilt vera opinberir og skrifvarinn og opnir fyrir upphleðslu eingöngu til tiltekinna meðlima, geturðu beinlínis stillt `auth: false`).
- **Fínn afhendingarheimildir til að lesa og skrifa og undirskrár**: Meðlimir eru sjálfgefið skriflegir (`r` þarf ekki að skrifa skýrt), og styðja `r` (skrifvarið), `w` (aðeins skrifa í sleppa kassann, ekki læsilegt), `rw` 5 (les-takmarka id e) fyrir undirskrá. hleður upp).

**Dæmi um uppsetningu meðlima (`~/.config/sync0/users/`):**

```yaml
# ~/.config/sync0/users/alice.nt (sjálfgefinn skrifvarinn, hraði 80MB)
id: 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c
rate: 80MB
```

```yaml
# ~/.config/sync0/users/bob.nt (les- og ritheimildir, aðeins leyfilegt að hlaða upp í tilgreinda undirskrá)
id: 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f
perm: rw
write_prefix:
  - uploads/bob
  - shared/common
```

```yaml
# ~/.config/sync0/users/charlie.nt (aðeins skrifa leyfi fyrir afhendingu, lestur er bannaður)
id: 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
perm: w
```

**Dæmi um stillingar fyrir deilingarhóp (`~/.config/sync0/config.nt`):**

```yaml
users_dir: ./users

groups:
  # Ekki stilltir notendur: sjálfgefið skrifvarið almenningsnet
  dataset:
    path: ./data
    rate: 50MB

  # Stilltir notendur: takmarka sjálfkrafa lestur almenningsnetsins (aðeins Alice og Bob geta lesið, Charlie getur aðeins sent og hlaðið upp)
  team-vault:
    path: ./vault
    users:
      - alice
      - bob
      - charlie
```

Ræstu þjónustuna (hlaðaðu stillingarskránni eða notaðu beint skipanalínufæribreytur til að takmarka lestur almenningsnets og heimila meðlimum):

```bash
# Hladdu ~/.config/sync0/config.nt til að byrja
sync0 share --region SHA

# Eða birtu einkadeilingarhóp í gegnum hreina skipanalínu (tilgreindu --allow-* til að takmarka sjálfkrafa lestur almenningsnets og aðeins viðurkenndir notendur hafa aðgang að því)
sync0 share team-vault:./vault \
  --allow-r 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c \
  --allow-rw 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f:uploads/bob,shared/common \
  --allow-w 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
```

### 5. Heimilisfang hnúts, nafnspjald og fjöltungumál

```bash
# Skoða staðbundið IP-laust hnútfang (lykill er sjálfkrafa búinn til í fyrstu keyrslu)
sync0 id

# Búðu til Ed25519 sjálfundirritað hnút nafnspjald (gælunafn, borgarkóði, tölvupóstur, vefsíða, kynning)
sync0 profile -u alice --region SHA --email alice@example.com -b "Shanghai node"

# Það eru 75 innbyggð tungumál, sem fylgja kerfismálinu sjálfgefið og hægt er að skipta um umhverfisbreytur.
SYNC0_LANG=en sync0 --help
```

## Afköst og kjarnaeiginleikar

### Raunveruleg WAN-mæling yfir meginlandið (Samanburður `rsync`)

Raunverulegt mæliumhverfi: Bandaríkin `c1` → Evrópa `n1` (RTT 65 ms), gagnasettið er Linux kjarna `linux-7.3.3`:

#### 1) A la carte ham á móti `rsync`

| Gagnasett | stærð | `sync0` Einn punktur<br>(Mbps) | `rsync`<br>(Mbps) | Hraða upp (`sync0` / `rsync`) |
| :--- | ---: | ---: | ---: | ---: |
| `linux-7.3.3.tar.xz`(ekki þjappanlegt) | 155 MB | **236.48** | 108.74 | **2,17×** |
| `linux-7.3.3.tar`(þjappanlegt) | 1.6 GB | **843.83** | 385.06 | **2,19×** |
| `linux-7.3.3.tar`(staðbundin breytingu) | 1.6 GB | **8,014.15** | 1,361.72 | **5,89×** |
| `linux-7.3.3/` (95.979 litlar skrár) | 1.5 GB | **627.58** | 347.80 | **1,80×** |
| **rúmfræðilegt meðaltal** | — | **1,000.91** | 375.26 | **2,67×** |

#### 2) P2P tvískiptur hamur á móti `rsync`

| Gagnasett | stærð | `sync0` P2P Dual Source<br>(Mbps) | `rsync`<br>(Mbps) | Hraða upp (`sync0` / `rsync`) |
| :--- | ---: | ---: | ---: | ---: |
| `linux-7.3.3.tar.xz`(ekki þjappanlegt) | 155 MB | **330.09** | 108.74 | **3,04×** |
| `linux-7.3.3.tar`(þjappanlegt) | 1.6 GB | **1,551.86** | 385.06 | **4,03×** |
| `linux-7.3.3.tar`(staðbundin breytingu) | 1.6 GB | **5,474.17** | 1,361.72 | **4,02×** |
| `linux-7.3.3/` (95.979 litlar skrár) | 1.5 GB | **706.56** | 347.80 | **2,03×** |
| **rúmfræðilegt meðaltal** | — | **1,186.42** | 375.26 | **3,16×** |

> Athugið: Í P2P dual-source ham, vinna evrópskir hnútar `s1` saman við sáningu og RTT yfir meginlandið er 65 ms.

### Kjarnaeiginleikar vöru

- **Opinber lykill sem fjallar um sjálfvirka gata**: Engin þörf á miðlægum netþjóni og almennu netfangi, sjálfvirk NAT gatagerð byggt á dreift netfangi almenningslykils.
- **Millisekúndu-stigi rauntímasamstillingu og meðvitund um lýsigögn**: Byggt á innfæddum skráaratburðaeftirliti (`FSEvents` / `inotify` / `ReadDirectoryChangesW`), rauntíma ýtt á skráarviðbætur, eyðingu, POSIX heimildum (`mode`) og millisekúndna-stigs breytingartíma="5">; núll gagnablokksending þegar aðeins heimildum eða tíma er breytt.
- **Samtímis niðurhal og upphleðsla í BT-stíl og tímasetning svæðisbundinnar skyldleika**: Niðurhalshnúturinn þarf ekki að bíða eftir að heildarupphæðinni sé hlaðið niður. Öll blokkabilin (`Op::Have`) sem hafa staðist 128 bita kjötkássaprófið eru strax læst í minni/tímabundnu lauginni fyrir aðra hnúta til að draga samtímis. Samhliða kraftmiklum PEX hnútaskiptum og hægum hnútahelmingum (Work-Steeling), er afköst klasans hámarkað.
- **Vandinn gegn illgjarnri hnút og friðhelgi fyrir klasaeitrun**: opinber heimildalisti `root_hash` Sjálfstætt læsing + lögboðin vélbúnaðarhröðun `gxhash128` samantekt áður en hver kubb er sett á diskinn og sáð fyrir utan; ef átt er við blokk finnst mun millisekúndna-stigs aflrofinn myrkva illgjarna hnútinn og jafna sig sjálfkrafa frá heiðarlegum uppruna.
- **FastCDC aukning á staðnum og samsöfnun lítilla skráa**: Aðeins mismunablokkir eru fluttar til að breyta stórum skrám að hluta og settar á disk; gríðarstórar umbúðir um litla skráarminni og mismunadrifsþjöppun fyrir forskeyti koma í veg fyrir kostnað frá skrá fyrir skrá fram og til baka.
- **Sjálfsgræðsla eftir rafmagnsleysi og endurræsingu eftir endurræsingu**: Redis-líkt RDB + AOF blendingssnið (`.s0_subs.bin`) er notað til að viðhalda sáningarstöðu og skráarlista, daglegar breytingar bætast við á míkrósekúndum og eru sjálfkrafa endurskrifaðar og skipulagðar reglulega. Eftir að þjónninn er endurræstur eru breytingar án nettengingar sjálfkrafa ýttar á sáningarhnútinn.

## Tæknilegur arkitektúr og skjöl

- **Tvískipt aftengingaraðferð**:
  - **Stjórn- og rauntímabreytingaryfirborð (`iroh` QUIC / TCP, fyrsta bæti `PROTO_VERSION` útgáfuhandabandi) **: Ábyrgð fyrir Ed25519 nafnspjaldastaðfestingu, heimildastaðfestingu, Swarm fræ uppgötvun og stuðning við `u64::MAX` straumspilun á stigi massabreytingar og <b id="id="4"> (compressed id="4"> í rauntíma id="6"> / `FsOp::Remove` / `FsOp::Meta` Þrjár frumstæður).
  - **Gagnadreifingaryfirborð (native UDP + `compio` ósamstilltur I/O) **: 20B samningur haus, 18B TACK reglubundin endurgjöf, BBRv3/Copa blendingur þrengslustýring, RACK-TLP endurheimt pakkataps, Shannon entropy adaptive Zstd compression framhjá, og <b id.="-2"> lokun.
- **Frekari upplýsingar**:
  - [Frammistöðumælingarskýrsla og endurgerð handrit](https://github.com/webc-site/sync0/blob/main/doc/zh/benchmark.md)
  - [Kerfisarkitektúrhönnun](https://github.com/webc-site/sync0/blob/main/doc/zh/design_v4.md)
  - [Binary Wire Protocol and Persistence Specification](https://github.com/webc-site/sync0/blob/main/doc/zh/proto.md)
  - [Auðkenningu auðkenna, valdstjórn og sannprófun á heiðarleika](https://github.com/webc-site/sync0/blob/main/doc/zh/crypto.md)