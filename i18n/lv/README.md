# sync0

`sync0` ir vienādranga (P2P) failu koplietošanas un vairāku iekārtu sinhronizācijas rīks, kas konkurē ar Resilio Sync.

Katra iekārta ir gan serveris, gan klients: jebkurš mezgls var publicēt vairākas failu koplietošanas grupas. Pēc citu mezglu pievienošanās tie var automātiski iegūt pieejamos sākuma mezglus no vienādranga servera un vienlaikus iegūt tos no vairākiem avotiem. Kad sinhronizācija būs pabeigta, tiks automātiski izveidotas sēklas papildu mezgliem, lai paātrinātu lejupielādi.

## Galvenās iezīmes

- **Ikviens ir gan serveris, gan klients**: salīdzināms ar Resilio Sync, nav centrālā servera atkarības. Jebkurš mezgls var izveidot vairākas failu koplietošanas grupas, kā arī pievienoties citu personu koplietošanas grupām; mezgli automātiski apmainās un iegūst pieejamos sākuma mezglus, lai panāktu vairāku iekārtu vienlaicīgu vilkšanu un reāllaika sinhronizāciju.
- **mezgla adrese bez IP**: pēc noklusējuma mezgla atslēga un `base64url` kodētā `iroh` adrese tiek automātiski ģenerēta `~/.config/sync0` bez nepieciešamības atcerēties vai atklāt publisko IP.
- **Vairāku atlases sinhronizācija pēc pieprasījuma**: atbalsta visas koplietošanas grupas sinhronizāciju, kā arī atbalsta sinhronizāciju pēc pieprasījuma, vienlaikus atlasot vairākus failus vai apakšdirektorijus.
- **Grupas dalībnieku hierarhiskās atļaujas un augšupielādes direktorija kontrole**: atbalsta tikai lasīšanas (`ro`), lasīšanas-rakstīšanas augšupielādes (`rw`) un pārvaldības (`admin`) atļaujas, kā arī var atsevišķi norādīt ceļa prefiksu, ko katrs dalībnieks drīkst augšupielādēt (pēc noklusējuma ir atļauta augšupielāde koplietojuma saknes direktorijā).
- **Pilna ātruma un pakāpeniska sinhronizācija tīklos**: lielu failu vienlaicīga pārsūtīšana, lielu mazu failu apkopošana atmiņā un tikai sinhronizēti datu bloki, ja faili ir daļēji modificēti.

## Ātruma salīdzinājums ar rsync

### Transokeāna tīkla viena avota faktiskais mērījums (Ķīna → Amerikas Savienotās Valstis, aizkave 260 ms)

| Pārraides scenārijs | Datu lielums | modelis | `sync0` | `rsync -az` | Palielināt reizinātāju |
| :--- | :---: | :---: | :---: | :---: | :---: |
| 500 mazi faili | 1.91 MB | Pilna sinhronizācija | 18.84 Mbps | 5.32 Mbps | 3.5x |
| 500 mazi faili | 1.91 MB | Pakāpeniska sinhronizācija | 27.15 Mbps | 3.83 Mbps | 7.1x |
| Kodu un arhīvu pakotnes (`.tar`) | 10.0 MB | Pilna sinhronizācija | 139.70 Mbps | 24.97 Mbps | 5.6x |
| Kodu un arhīvu pakotnes (`.tar`) | 10.0 MB | Pakāpeniska sinhronizācija | 83.38 Mbps | 26.02 Mbps | 3.2x |
| Video fails (`.mp4`) | 15.0 MB | Pilna sinhronizācija | 54.06 Mbps | 27.04 Mbps | 2.0x |
| Video fails (`.mp4`) | 15.0 MB | Pakāpeniska sinhronizācija | 216.83 Mbps | 21.51 Mbps | 10.1x |

### Aizokeāna tīkla P2P vairāku mezglu vilkšanas tests (Ķīna → Amerikas Savienotās Valstis, divi mezgli, aizkave 260 ms)

Izmērītais vairāku avotu sēklas mezgla svītrainās vienlaicīgas vilkšanas salīdzinājums:

| Pārraides scenārijs | Datu lielums | Viena mezgla sēšana | Divu mezglu P2P vienlaicīga sēšana | Palielināt reizinātāju |
| :--- | :---: | :---: | :---: | :---: |
| 500 mazi faili | 1.97 MB | 5.28 Mbps | 7.87 Mbps | 1.5x |
| Kodu un arhīvu pakotnes (`.tar`) | 30.0 MB | 56.35 Mbps | 83.17 Mbps | 1.5x |
| Video fails (`.mp4`) | 30.0 MB | 27.00 Mbps | 39.46 Mbps | 1.5x |

### Faktiskais starpkontinentālās ātrgaitas līnijas mērījums (Eiropa → Ziemeļamerika, aizkave 86 ms)

| Pārraides scenārijs | Datu lielums | `sync0` | `rsync` | Palielināt reizinātāju |
| :--- | :---: | :---: | :---: | :---: |
| 10 000 mazu failu | 77.6 MB | 688 Mbps | 128 Mbps | 5.4x |
| Kodu un arhīvu pakotnes (`.tar`) | 512 MB | 2111 Mbps | 438 Mbps | 4.8x |
| Video fails (`.mp4`) | 512 MB | 556 Mbps | 338 Mbps | 1.6x |
| Pakāpeniska sinhronizācija (mainīt 1% saturu) | 512 MB | 34133 Mbps | 2214 Mbps | 15.4x |

## Instalējiet

### 1. metode: instalējiet, izmantojot kravu

```bash
cargo install sync0
```

### 2. metode: lejupielādējiet iepriekš kompilētus bināros failus

Dodieties uz [GitHub Releases](https://github.com/webc-site/sync0/releases), lai lejupielādētu `.tar.xz` saspiesto pakotni attiecīgajai sistēmai (atbalsta Linux, macOS, Windows, Android, FreeBSD, NetBSD), izpakojiet un ievietojiet to `PATH` direktorijā:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Sāciet ātri

`sync0` pēc noklusējuma izmanto `~/.config/sync0` kā konfigurācijas direktoriju (ja nepieciešams pārslēgties uz citiem direktorijiem, varat pievienot `-c <direktoriju>`).

### 1. Skatiet vietējā mezgla adresi un iestatījumu vizītkarti

```bash
# Skatiet lokālā mezgla adresi (atslēga pirmo reizi tiek automātiski ģenerēta mapē ~/.config/sync0)
sync0 id

# Ģenerējiet personīgo mezgla vizītkarti (atbalsta lietotājvārdu, pilsētu, e-pastu, vietni un parakstu 256 vārdos)
sync0 profile -u alice --region SHA --email alice@example.com --url https://example.com -b "Shanghai node"
```

### 2. Publicēt vairākas failu koplietošanas grupas un iestatīt grupas dalībnieku atļaujas (`.nt` konfigurācija)

`sync0` ir konfigurēts [NestedText](https://nestedtext.org) (`.nt`) formātā. Katram lietotājam ir atsevišķs `.nt` konfigurācijas fails. Varat norādīt lietotāja profila direktoriju, izmantojot `users_dir` `config.nt` (kas automātiski ietver visus `.nt` failus šajā direktorijā). Gan lietotājs, gan ceļa prefikss, ko atļauts augšupielādēt, izmanto nesakārtotu sarakstu (`-`). **Ja augšupielādes ceļa prefikss nav aizpildīts, augšupielāde koplietošanas saknes direktorijā ir atļauta pēc noklusējuma**.

1. Izveidojiet atsevišķus `.nt` konfigurācijas failus katram dalībniekam `~/.config/sync0/users/` direktorijā:

`~/.config/sync0/users/alice.nt` (tikai lasāms dalībnieks, ierobežots skaits `80MB`, `rate` atbalsta `GB`, `MB`, `KB` vienības):

```yaml
id: 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c
perm: ro
rate: 80MB
```

`~/.config/sync0/users/bob.nt` (ierobežoti augšupielādes dalībnieki: atļauts augšupielādēt tikai `uploads/bob` un `shared/common` direktorijos):

```yaml
id: 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f
perm: rw
write_prefix:
  - uploads/bob
  - shared/common
```

`~/.config/sync0/users/charlie.nt` (Noklusējuma dalībnieku augšupielāde: ja `write_prefix` ir izlaists, augšupielāde koplietošanas saknes direktorijā ir atļauta pēc noklusējuma):

```yaml
id: 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
perm: rw
```

2. Konfigurējiet lietotāja direktoriju un vairākas failu koplietošanas grupas sadaļā `~/.config/sync0/config.nt`:

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

Sākt koplietošanas pakalpojumu:

```bash
# Tiešā palaišana (automātiski ielādē visas .nt lietotāju konfigurācijas direktorijos ~/.config/sync0/config.nt un users_dir)
sync0 share --region SHA

# Vai arī ātri publicējiet vairākas koplietošanas grupas un autorizējiet grupas dalībniekus tieši, izmantojot komandrindu
sync0 share dataset:./data team-vault:./vault \
  --auth \
  --allow-ro 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c \
  --allow-rw 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f:uploads/bob,shared/common \
  --allow-rw 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
```

### 3. Izvelciet visu saturu vai atlasiet vairākus failus un direktorijus (`sync0 pull`)

Izveidojiet savienojumu ar jebkuru vienādranga mezglu, lai automātiski iegūtu visus pieejamos sākuma mezglus koplietošanas grupā, un vienlaikus izvilktu no vairākiem sākuma mezgliem:

```bash
# Atvelciet visu datu kopas koplietošanas grupu uz vietējo ./local_data
sync0 pull dataset:// ./local_data -p <mezgla adrese>

# Vairāku atlases sinhronizācija: vienlaikus izvelciet vienu failu docs/readme.md un apakšdirektoriju aktīvi/icons, src/core
sync0 pull dataset://docs/readme.md,assets/icons,src/core ./local_data -p <mezgla adrese>
```

### 4. Augšupielādējiet failu vai direktoriju koplietošanas grupā (`sync0 push`)

Dalībnieki ar augšupielādes atļaujām (`rw` vai `admin`) var augšupielādēt vietējos failus vai direktorijus attālās koplietošanas grupās:

```bash
# Dalībnieki ar saknes direktorija augšupielādes atļauju: augšupielādējiet tieši koplietotajā saknes direktorijā
sync0 push team-vault:// ./new_release -p <mezgla adrese>

# Tikai dalībnieki, kuriem ir atļauts izmantot konkrētu prefiksu (piemēram, augšupielādes/bob): augšupielādējiet atļautajā ceļa prefiksā
sync0 push team-vault://uploads/bob/report.pdf ./report.pdf -p <mezgla adrese>
```

### 5. Abonēt automātisko sinhronizāciju un sēšanu (`sync0 sub`)

Pēc abonēšanas noteikta mezgla koplietošanas grupai vietējais serveris ir gan saņemošais klients, gan jauns sēšanas serveris; kad avota mezgla fails tiek atjaunināts, tas tiek automātiski sinhronizēts un nodrošina gabalos lejupielādes citiem mezgliem:

```bash
# Abonējiet visu koplietošanas grupu un sāciet automātiski
sync0 sub dataset:// ./local_mirror -p <mezgla adrese>

# Atlasiet tikai vairākas, lai abonētu noteiktus failus un apakšdirektorijus un tos sāktu
sync0 apakšdatu kopa://models/llama.onnx,configs ./local_mirror -p <mezgla adrese>
```

### 6. Vairāku mašīnu grupu vienādranga sinhronizācija (`sync0 group`)

Vienai koplietošanas grupai pievienojas vairākas iekārtas, viena otra automātiski atklāj pieejamos sākuma mezglus un uztur divvirzienu sinhronizāciju (atbalsta pilnu sinhronizāciju vai vairāku failu un direktoriju atlasi):

```bash
# Pilnībā pievienoties grupai un divvirzienu sinhronizācija
sync0 grupa team-docs:// ./docs -p <mezgla adrese>

# Vairāku atlases sinhronizācijas grupā ir tikai wal direktorijs un meta.json fails
sync0 grupa team-docs://wal,meta.json ./docs -p <mezgla adrese>
```

### 7. Tiešā pārraide no punkta uz punktu starp divām iekārtām (`send` / `recv`)

Nav nepieciešams iepriekš konfigurēt grupu, un jūs varat īslaicīgi pārsūtīt failus vai direktorijus tieši starp divām iekārtām:

```bash
# Uztvērēja uzraudzība
sync0 recv 0.0.0.0:9000 ./backup

# Sūtīšanas gals tieši pārsūta failus vai direktorijus (pēc izvēles --delete spogulis, lai notīrītu liekos failus, -- ātruma ierobežojums)
sync0 send ./project 192.168.1.10:9000
```

### 8. Vairāku valodu pārslēgšana

`sync0` atbalsta 75 valodas un pēc noklusējuma automātiski pārslēdzas atbilstoši sistēmas valodas videi; to var norādīt arī manuāli, izmantojot `SYNC0_LANG`:

```bash
SYNC0_LANG=en sync0 --help
SYNC0_LANG=ja sync0 --help
SYNC0_LANG=de sync0 --help
```