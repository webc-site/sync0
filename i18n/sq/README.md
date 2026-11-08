# sync0

`sync0` është një mjet për ndarjen e skedarëve dhe sinkronizimin me shumë makineri peer-to-peer (P2P) që konkurron me Resilio Sync.

Çdo makinë është njëkohësisht një server dhe një klient: çdo nyje mund të publikojë grupe të shumta për ndarjen e skedarëve. Pasi të bashkohen nyjet e tjera, ata mund të marrin automatikisht nyjet e disponueshme të farërave nga serveri peer dhe t'i tërheqin ato njëkohësisht nga burime të shumta. Pas përfundimit të sinkronizimit, farat do të krijohen automatikisht për më shumë nyje për të përshpejtuar shkarkimet.

## Pikat kryesore

- **Të gjithë janë njëkohësisht server dhe klient**: I krahasueshëm me Resilio Sync, nuk ka varësi nga serveri qendror. Çdo nyje mund të krijojë grupe të shumta për ndarjen e skedarëve, dhe gjithashtu mund të bashkohet me grupet e ndarjes së njerëzve të tjerë; nyjet shkëmbejnë automatikisht dhe marrin nyjet e disponueshme të farës për të arritur tërheqjen e njëkohshme dhe sinkronizimin në kohë reale të makinave të shumta.
- **Adresa e nyjës pa IP**: Si parazgjedhje, çelësi i nyjes dhe adresa `base64url` e koduar `iroh` gjenerohen automatikisht në `~/.config/sync0` pa pasur nevojë të mbani mend ose ekspozoni IP-në publike.
- **Sinkronizimi me shumë përzgjedhje sipas kërkesës**: Mbështet sinkronizimin e të gjithë grupit të ndarjes dhe gjithashtu mbështet sinkronizimin sipas kërkesës duke zgjedhur skedarë ose nëndrejtori të shumta në të njëjtën kohë.
- **Lejet hierarkike të anëtarëve të grupit dhe kontrolli i direktoriumit të ngarkimit**: Mbështet lejet vetëm për lexim (`ro`), ngarkimin për lexim-shkrim (`rw`) dhe menaxhimin (`admin`), dhe mund të specifikojë individualisht prefiksin e rrugës që çdo anëtar lejohet të ngarkojë (si parazgjedhje, ngarkimi i direktoriumit rrënjë është i paracaktuar).
- **Sinkronizimi me shpejtësi të plotë dhe në rritje nëpër rrjete**: Transmetimi i njëkohshëm i skedarëve të mëdhenj, grumbullimi i skedarëve të vegjël masiv në memorie dhe vetëm blloqe të sinkronizuara të të dhënave kur skedarët modifikohen pjesërisht.

## Krahasimi i shpejtësisë me rsync

### Matja aktuale e rrjetit transoqeanik me një burim të vetëm (Kinë → Shtetet e Bashkuara, vonesë 260 ms)

| Skenari i transmetimit | Madhësia e të dhënave | model | `sync0` | `rsync -az` | Rritja e shumëzuesit |
| :--- | :---: | :---: | :---: | :---: | :---: |
| 500 skedarë të vegjël | 1.91 MB | Sinkronizimi i plotë | 18.84 Mbps | 5.32 Mbps | 3.5x |
| 500 skedarë të vegjël | 1.91 MB | Sinkronizimi në rritje | 27.15 Mbps | 3.83 Mbps | 7.1x |
| Paketat e kodit dhe arkivave (`.tar`) | 10.0 MB | Sinkronizimi i plotë | 139.70 Mbps | 24.97 Mbps | 5.6x |
| Paketat e kodit dhe arkivave (`.tar`) | 10.0 MB | Sinkronizimi në rritje | 83.38 Mbps | 26.02 Mbps | 3.2x |
| Skedar video (`.mp4`) | 15.0 MB | Sinkronizimi i plotë | 54.06 Mbps | 27.04 Mbps | 2.0x |
| Skedar video (`.mp4`) | 15.0 MB | Sinkronizimi në rritje | 216.83 Mbps | 21.51 Mbps | 10.1x |

### Testi i tërheqjes me shumë nyje të rrjetit transoqeanik P2P (Kinë → nyje të dyfishta të Shteteve të Bashkuara, vonesë 260 ms)

Krahasimi i matur i tërheqjes së njëkohshme me shirita të nyjeve me shumë burime:

| Skenari i transmetimit | Madhësia e të dhënave | Mbjellja me një nyje të vetme | Mbjellja e njëkohshme P2P me dy nyje | Rritja e shumëzuesit |
| :--- | :---: | :---: | :---: | :---: |
| 500 skedarë të vegjël | 1.97 MB | 5.28 Mbps | 7.87 Mbps | 1.5x |
| Paketat e kodit dhe arkivave (`.tar`) | 30.0 MB | 56.35 Mbps | 83.17 Mbps | 1.5x |
| Skedar video (`.mp4`) | 30.0 MB | 27.00 Mbps | 39.46 Mbps | 1.5x |

### Matja aktuale e linjës së dedikuar transkontinentale me shpejtësi të lartë (Evropë → Amerika e Veriut, vonesë 86 ms)

| Skenari i transmetimit | Madhësia e të dhënave | `sync0` | `rsync` | Rritja e shumëzuesit |
| :--- | :---: | :---: | :---: | :---: |
| 10,000 skedarë të vegjël | 77.6 MB | 688 Mbps | 128 Mbps | 5.4x |
| Paketat e kodit dhe arkivave (`.tar`) | 512 MB | 2111 Mbps | 438 Mbps | 4.8x |
| Skedar video (`.mp4`) | 512 MB | 556 Mbps | 338 Mbps | 1.6x |
| Sinkronizimi në rritje (modifiko përmbajtjen 1%) | 512 MB | 34133 Mbps | 2214 Mbps | 15.4x |

## Instaloni

### Metoda 1: Instaloni përmes ngarkesave

```bash
cargo install sync0
```

### Metoda 2: Shkarkoni binarët e parakompiluar

Shkoni te [GitHub Releases](https://github.com/webc-site/sync0/releases) për të shkarkuar paketën e ngjeshur `.tar.xz` për sistemin përkatës (mbështet Linux, macOS, Windows, Android, FreeBSD, NetBSD), shkëputeni dhe vendoseni në drejtorinë `PATH`:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Filloni shpejt

`sync0` përdor `~/.config/sync0` si direktori të konfigurimit si parazgjedhje (nëse duhet të kaloni në drejtori të tjera, mund të shtoni `-c <directory>`).

### 1. Shikoni adresën e nyjës lokale dhe vendosjen e kartës së biznesit

```bash
# Shiko adresën e nyjës lokale (çelësi gjenerohet automatikisht në ~/.config/sync0 për herë të parë)
sync0 id

# Krijo një kartë biznesi të nyjeve personale (mbështet emrin e përdoruesit, qytetin, emailin, faqen e internetit dhe nënshkrimin brenda 256 fjalëve)
sync0 profile -u alice --region SHA --email alice@example.com --url https://example.com -b "Shanghai node"
```

### 2. Publikoni grupe të shumta për ndarjen e skedarëve dhe vendosni lejet e anëtarëve të grupit (`.nt` Konfigurimi)

`sync0` është konfiguruar në formatin [NestedText](https://nestedtext.org) (`.nt`). Çdo përdorues ka një skedar të veçantë konfigurimi `.nt`. Ju mund të specifikoni direktorinë e profilit të përdoruesit përmes `users_dir` në `config.nt` (e cila përfshin automatikisht të gjithë skedarët `.nt` në këtë direktori). Si përdoruesi ashtu edhe prefiksi i rrugës që lejohet të ngarkohet përdorin një listë të pa renditur (`-`). **Kur prefiksi i rrugës së ngarkimit nuk plotësohet, ngarkimi në direktoriumin rrënjë të aksionit lejohet si parazgjedhje**.

1. Krijo skedarë të veçantë konfigurimi `.nt` për secilin anëtar në drejtorinë `~/.config/sync0/users/`:

`~/.config/sync0/users/alice.nt` (anëtar vetëm për lexim, norma e kufizuar `80MB`, `rate` mbështet njësitë `GB`, `MB`, `KB`):

```yaml
id: 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c
perm: ro
rate: 80MB
```

`~/.config/sync0/users/bob.nt` (anëtarë të ngarkuar të kufizuar: lejohen të ngarkohen vetëm në drejtoritë `uploads/bob` dhe `shared/common`):

```yaml
id: 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f
perm: rw
write_prefix:
  - uploads/bob
  - shared/common
```

`~/.config/sync0/users/charlie.nt` (Ngarkimi i parazgjedhur i anëtarit: Kur `write_prefix` hiqet, ngarkimi në direktorinë rrënjë të ndarjes lejohet si parazgjedhje):

```yaml
id: 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
perm: rw
```

2. Konfiguro drejtorinë e përdoruesit dhe grupet e shumta të ndarjes së skedarëve në `~/.config/sync0/config.nt`:

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

Filloni shërbimin e ndarjes:

```bash
# Nisja e drejtpërdrejtë (ngarkon automatikisht të gjitha konfigurimet e përdoruesve .nt në drejtoritë ~/.config/sync0/config.nt dhe users_dir)
sync0 share --region SHA

# Ose publikoni shpejt grupe të shumta të ndarjes dhe autorizoni anëtarët e grupit drejtpërdrejt përmes linjës së komandës
sync0 share dataset:./data team-vault:./vault \
  --auth \
  --allow-ro 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c \
  --allow-rw 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f:uploads/bob,shared/common \
  --allow-rw 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
```

### 3. Tërhiq të gjithë përmbajtjen ose zgjidh shumë skedarë dhe drejtori (`sync0 pull`)

Lidhu me çdo nyje homologe për të marrë automatikisht të gjitha nyjet e farës së disponueshme në grupin e ndarjes dhe tërhiqeni njëkohësisht nga nyjet e shumëfishta të farës:

```bash
# Tërhiqeni të gjithë grupin e ndarjes së të dhënave në lokalin ./local_data
sync0 pull dataset:// ./local_data -p <adresa e nyjes>

# Sinkronizimi me shumë përzgjedhje: tërhiqni njëkohësisht skedarin e vetëm docs/readme.md dhe nëndrejtoritë asetet/ikonat, src/core
sync0 tërheqëse të dhënash://docs/readme.md,assets/icons,src/core ./local_data -p <adresa e nyjës>
```

### 4. Ngarko një skedar ose drejtori në një grup shpërndarjeje (`sync0 push`)

Anëtarët me leje ngarkimi (`rw` ose `admin`) mund të ngarkojnë skedarë ose drejtori lokale në grupet e ndarjes në distancë:

```bash
# Anëtarët me leje ngarkimi të direktoriumit rrënjë: Ngarko direkt në direktorinë e përbashkët rrënjë
sync0 push team-vault:// ./new_release -p <adresa e nyjes>

# Vetëm anëtarët e autorizuar për një parashtesë specifike (siç janë ngarkimet/bob): ngarko në prefiksin e lejuar të rrugës
sync0 push team-vault://uploads/bob/report.pdf ./report.pdf -p <adresa e nyjës>
```

### 5. Abonohu ​​në sinkronizimin dhe mbjelljen automatike (`sync0 sub`)

Pas abonimit në grupin e ndarjes së një nyje të caktuar, serveri lokal është njëkohësisht një klient marrës dhe një server i ri mbjellës; kur skedari i nyjes burimore përditësohet, ai sinkronizohet automatikisht dhe siguron shkarkime të copëtuara për nyjet e tjera:

```bash
# Abonohuni në të gjithë grupin e ndarjes dhe vendoseni automatikisht
sync0 subsets:// ./local_mirror -p <adresa e nyjes>

# Zgjidhni vetëm shumëfish për t'u abonuar në skedarë dhe nëndirektorë të specifikuar dhe për t'i vendosur ato
sync0 subsets://models/llama.onnx,configs ./local_mirror -p <adresa e nyjës>
```

### 6. Sinkronizimi i grupeve me shumë makina (`sync0 group`)

Makinat e shumta bashkohen me të njëjtin grup shkëmbimi, njëra-tjetra zbulon automatikisht nyjet e disponueshme të farës dhe ruan sinkronizimin e dyanshëm (mbështet sinkronizimin e plotë ose përzgjedhjen e shumëfishtë të skedarëve dhe drejtorive):

```bash
# Bashkohuni plotësisht me grupin dhe sinkronizimin e dyanshëm
sync0 group team-docs:// ./docs -p <adresa e nyjes>

# Vetëm direktoria wal dhe skedari meta.json në grupin e sinkronizimit me shumë përzgjedhje
sync0 group team-docs://wal,meta.json ./docs -p <adresa e nyjës>
```

### 7. Transmetim i drejtpërdrejtë pikë-për-pikë ndërmjet dy makinave (`send` / `recv`)

Nuk ka nevojë të konfiguroni paraprakisht një grup dhe ju mund të transferoni përkohësisht skedarë ose drejtori direkt midis dy makinave:

```bash
# Monitorimi i marrësit
sync0 recv 0.0.0.0:9000 ./backup

# Fundi i dërgimit transferon drejtpërdrejt skedarët ose drejtoritë (opsionale -- fshini pasqyrën për të pastruar skedarët e tepërt, -- kufiri i shpejtësisë së normës)
sync0 send ./project 192.168.1.10:9000
```

### 8. Ndërrimi në shumë gjuhë

`sync0` mbështet 75 gjuhë dhe kalon automatikisht sipas mjedisit të gjuhës së sistemit si parazgjedhje; mund të specifikohet gjithashtu manualisht përmes `SYNC0_LANG`:

```bash
SYNC0_LANG=en sync0 --help
SYNC0_LANG=ja sync0 --help
SYNC0_LANG=de sync0 --help
```