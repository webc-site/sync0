# sync0

`sync0` este un instrument peer-to-peer (P2P) de partajare a fișierelor și de sincronizare pe mai multe mașini, care concurează cu Resilio Sync.

Fiecare mașină este atât un server, cât și un client: orice nod poate publica mai multe grupuri de partajare a fișierelor. După ce alte noduri se alătură, aceștia pot obține automat nodurile semințe disponibile de la serverul peer și le pot extrage simultan din mai multe surse. După finalizarea sincronizării, semințele vor fi create automat pentru mai multe noduri pentru a accelera descărcările.

## Repere principale

- **Toată lumea este atât un server, cât și un client**: comparabil cu Resilio Sync, fără dependență de serverul central. Orice nod poate crea mai multe grupuri de partajare a fișierelor și, de asemenea, se poate alătura grupurilor de partajare ale altor persoane; nodurile schimbă automat și obțin noduri de semințe disponibile pentru a obține tragere simultană și sincronizare în timp real a mai multor mașini.
- **Adresă de nod fără IP**: în mod implicit, cheia de nod și adresa `base64url` codificată `iroh` sunt generate automat în `~/.config/sync0` fără a fi nevoie să vă amintiți sau să expuneți IP-ul public.
- **Sincronizare cu selecție multiplă la cerere**: acceptă sincronizarea întregului grup de partajare și, de asemenea, acceptă sincronizarea la cerere prin selectarea mai multor fișiere sau subdirectoare în același timp.
- **Permisiuni ierarhice pentru membrii grupului și controlul directorului de încărcare**: acceptă permisiuni numai citire (`ro`), încărcare citire-scriere (`rw`) și gestionare (`admin`) și poate specifica individual prefixul de cale pe care fiecare membru are voie să îl încarce (în mod implicit, încărcarea în directorul rădăcină este permisă).
- **Sincronizare la viteză maximă și incrementală între rețele**: transmisie concomitentă de fișiere mari, agregare de fișiere masive mici în memorie și blocuri de date sincronizate numai atunci când fișierele sunt modificate parțial.

## Comparație de viteză cu rsync

### Măsurare reală a rețelei transoceanice cu o singură sursă (China → Statele Unite, întârziere 260 ms)

| Scenariul transmisiei | Dimensiunea datelor | model | `sync0` | `rsync -az` | Creșteți multiplicatorul |
| :--- | :---: | :---: | :---: | :---: | :---: |
| 500 de fișiere mici | 1.91 MB | Sincronizare completă | 18.84 Mbps | 5.32 Mbps | 3.5x |
| 500 de fișiere mici | 1.91 MB | Sincronizare incrementală | 27.15 Mbps | 3.83 Mbps | 7.1x |
| Pachete de cod și arhivare (`.tar`) | 10.0 MB | Sincronizare completă | 139.70 Mbps | 24.97 Mbps | 5.6x |
| Pachete de cod și arhivare (`.tar`) | 10.0 MB | Sincronizare incrementală | 83.38 Mbps | 26.02 Mbps | 3.2x |
| Fișier video (`.mp4`) | 15.0 MB | Sincronizare completă | 54.06 Mbps | 27.04 Mbps | 2.0x |
| Fișier video (`.mp4`) | 15.0 MB | Sincronizare incrementală | 216.83 Mbps | 21.51 Mbps | 10.1x |

### Test de tragere P2P cu mai multe noduri de rețea transoceanica (China → Statele Unite ale Americii noduri duale, întârziere 260 ms)

Comparație măsurată a atracției concomitente cu dungi cu noduri de semințe multi-surse:

| Scenariul transmisiei | Dimensiunea datelor | Semănat cu un singur nod | Semănare concomitentă P2P cu două noduri | Creșteți multiplicatorul |
| :--- | :---: | :---: | :---: | :---: |
| 500 de fișiere mici | 1.97 MB | 5.28 Mbps | 7.87 Mbps | 1.5x |
| Pachete de cod și arhivare (`.tar`) | 30.0 MB | 56.35 Mbps | 83.17 Mbps | 1.5x |
| Fișier video (`.mp4`) | 30.0 MB | 27.00 Mbps | 39.46 Mbps | 1.5x |

### Măsurarea efectivă a liniei dedicate transcontinentale de mare viteză (Europa → America de Nord, întârziere 86 ms)

| Scenariul transmisiei | Dimensiunea datelor | `sync0` | `rsync` | Creșteți multiplicatorul |
| :--- | :---: | :---: | :---: | :---: |
| 10.000 de fișiere mici | 77.6 MB | 688 Mbps | 128 Mbps | 5.4x |
| Pachete de cod și arhivare (`.tar`) | 512 MB | 2111 Mbps | 438 Mbps | 4.8x |
| Fișier video (`.mp4`) | 512 MB | 556 Mbps | 338 Mbps | 1.6x |
| Sincronizare incrementală (modificare 1% conținut) | 512 MB | 34133 Mbps | 2214 Mbps | 15.4x |

## Instala

### Metoda 1: Instalați prin Cargo

```bash
cargo install sync0
```

### Metoda 2: Descărcați fișiere binare precompilate

Accesați [GitHub Releases](https://github.com/webc-site/sync0/releases) pentru a descărca pachetul `.tar.xz` comprimat pentru sistemul corespunzător (suporta Linux, macOS, Windows, Android, FreeBSD, NetBSD), dezarhivați și puneți-l în directorul `PATH`:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Începeți repede

`sync0` folosește `~/.config/sync0` ca director de configurare în mod implicit (dacă trebuie să comutați la alte directoare, puteți adăuga `-c <directory>`).

### 1. Vizualizați adresa nodului local și setarea cărții de vizită

```bash
# Vizualizați adresa nodului local (cheia este generată automat în ~/.config/sync0 pentru prima dată)
sync0 id

# Generați o carte de vizită personală cu nod (suportă numele de utilizator, orașul, e-mailul, site-ul web și semnătura în termen de 256 de cuvinte)
sync0 profile -u alice --region SHA --email alice@example.com --url https://example.com -b "Shanghai node"
```

### 2. Publicați mai multe grupuri de partajare a fișierelor și setați permisiunile membrilor grupului (`.nt` Configurare)

`sync0` este configurat în formatul [NestedText](https://nestedtext.org) (`.nt`). Fiecare utilizator are un fișier de configurare `.nt` separat. Puteți specifica directorul profilului utilizatorului prin `users_dir` în `config.nt` (care include automat toate fișierele `.nt` din acest director). Atât utilizatorul, cât și prefixul de cale care poate fi încărcat folosesc o listă neordonată (`-`). **Când prefixul căii de încărcare nu este completat, încărcarea în directorul rădăcină al partajării este permisă în mod implicit**.

1. Creați fișiere de configurare `.nt` separate pentru fiecare membru din directorul `~/.config/sync0/users/`:

`~/.config/sync0/users/alice.nt` (membru numai în citire, tarif limitat `80MB`, `rate` acceptă unități `GB`, `MB`, `KB`):

```yaml
id: 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c
perm: ro
rate: 80MB
```

`~/.config/sync0/users/bob.nt` (membri cu încărcare restricționată: este permis doar încărcarea în directoare `uploads/bob` și `shared/common`):

```yaml
id: 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f
perm: rw
write_prefix:
  - uploads/bob
  - shared/common
```

`~/.config/sync0/users/charlie.nt` (Încărcarea implicită a membrilor: când `write_prefix` este omis, încărcarea în directorul rădăcină al partajării este permisă în mod implicit):

```yaml
id: 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
perm: rw
```

2. Configurați directorul de utilizatori și mai multe grupuri de partajare a fișierelor în `~/.config/sync0/config.nt`:

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

Începeți serviciul de partajare:

```bash
# Pornire directă (încarcă automat toate configurațiile utilizatorului .nt în directoarele ~/.config/sync0/config.nt și user_dir)
sync0 share --region SHA

# Sau publicați rapid mai multe grupuri de partajare și autorizați membrii grupului direct prin linia de comandă
sync0 share dataset:./data team-vault:./vault \
  --auth \
  --allow-ro 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c \
  --allow-rw 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f:uploads/bob,shared/common \
  --allow-rw 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
```

### 3. Extrageți tot conținutul sau selectați mai multe fișiere și directoare (`sync0 pull`)

Conectați-vă la orice nod peer pentru a obține automat toate nodurile semințe disponibile în grupul de partajare și extrageți de la mai multe noduri semințe simultan:

```bash
# Trageți întregul grup de partajare a setului de date în ./local_data local
sync0 pull dataset:// ./local_data -p <adresa nodului>

# Sincronizare cu selecție multiplă: trageți simultan fișierul unic docs/readme.md și subdirectoarele active/pictograme, src/core
sync0 pull dataset://docs/readme.md,assets/icons,src/core ./local_data -p <adresa nodului>
```

### 4. Încărcați un fișier sau un director într-un grup de partajare (`sync0 push`)

Membrii cu permisiuni de încărcare (`rw` sau `admin`) pot încărca fișiere sau directoare locale în grupuri de partajare de la distanță:

```bash
# Membrii cu permisiunea de încărcare a directorului rădăcină: Încărcați direct în directorul rădăcină partajat
sync0 push team-vault:// ./new_release -p <adresa nodului>

# Doar membrii autorizați la un anumit prefix (cum ar fi încărcări/bob): încărcați la prefixul de cale permis
sync0 push team-vault://uploads/bob/report.pdf ./report.pdf -p <adresa nodului>
```

### 5. Abonați-vă la sincronizare automată și seeding (`sync0 sub`)

După abonamentul la grupul de partajare al unui anumit nod, serverul local este atât un client de primire, cât și un nou server de seeding; când fișierul nodului sursă este actualizat, acesta este sincronizat automat și oferă descărcări fragmentate pentru alte noduri:

```bash
# Abonați-vă la întregul grup de partajare și începeți automat
sync0 sub dataset:// ./local_mirror -p <adresa nodului>

# Doar selecție multiplă pentru a vă abona la fișierele și subdirectoarele specificate și pentru a le genera
sync0 sub dataset://models/llama.onnx,configs ./local_mirror -p <adresa nodului>
```

### 6. Sincronizare peer de grup cu mai multe mașini (`sync0 group`)

Mai multe mașini se alătură aceluiași grup de partajare, unele descoperă automat nodurile de bază disponibile și mențin sincronizarea bidirecțională (suportă sincronizarea completă sau selecția multiplă de fișiere și directoare):

```bash
# Alăturați-vă complet la grup și sincronizare bidirecțională
sync0 group team-docs:// ./docs -p <adresa nodului>

# Numai directorul wal și fișierul meta.json din grupul de sincronizare cu selecție multiplă
sync0 group team-docs://wal,meta.json ./docs -p <adresa nodului>
```

### 7. Transmitere directă punct la punct între două mașini (`send` / `recv`)

Nu este nevoie să preconfigurați un grup și puteți transfera temporar fișiere sau directoare direct între două mașini:

```bash
# Monitorizarea receptorului
sync0 recv 0.0.0.0:9000 ./backup

# Sfârșitul de trimitere transferă direct fișiere sau directoare (opțional --delete mirror pentru a curăța fișierele în exces, --rate limită de viteză)
sync0 send ./project 192.168.1.10:9000
```

### 8. Comutare în mai multe limbi

`sync0` acceptă 75 de limbi și comută automat în funcție de mediul de limbă al sistemului în mod implicit; poate fi specificat și manual prin `SYNC0_LANG`:

```bash
SYNC0_LANG=en sync0 --help
SYNC0_LANG=ja sync0 --help
SYNC0_LANG=de sync0 --help
```