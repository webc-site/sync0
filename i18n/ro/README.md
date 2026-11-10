# sync0

`sync0` este un instrument descentralizat de partajare a fișierelor P2P, de sincronizare în timp real și de distribuție multi-sursă folosit pentru a înlocui `rsync` - viteza măsurată a rețelei transcontinentale de suprafață este de până la **6,95×** de `rsync` (viteză medie geometrică într-un singur punct **2,01×**), viteza medie geometrică de sincronizare **2,01×**, P2P3 × **-sursă. fișiere masive mici peste oceane este la fel de rapidă ca **7,19×**.

Fiecare mașină este atât un server, cât și un client: nu este nevoie de un server central sau de o adresă de rețea publică, perforarea automată se bazează pe adresa cheii publice de rețea distribuită și acceptă transmisia directă între două mașini, găzduire cu permisiuni de grup de partajare multiple, selecție multiplă de subdirectoare la cerere, sincronizare incrementală în timp real la nivel de milisecunde și accelerare simultană P2P multi-surse.

## Instala

### Metoda 1: Instalarea încărcăturii

```bash
cargo install sync0
```

### Metoda 2: Descărcați fișiere binare precompilate

Accesați [GitHub Releases](https://github.com/webc-site/sync0/releases) pentru a descărca pachetul `.tar.xz` comprimat corespunzător sistemului (accepta Linux, macOS, Windows, Android, iOS, FreeBSD), dezarhivați și puneți-l în `PATH`:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Începeți repede

### 1. Transmitere directă fără configurație între două mașini (`send` / `recv`)

Nu este nevoie să configurați un grup de partajare, să transferați direct fișiere sau directoare între două mașini la viteză mare:

```bash
# Extremul receptor ascultă pe port și scrie în ./backup
sync0 recv 0.0.0.0:9000 ./backup

# Transmitere directă de la expeditor (opțional --șterge oglindă pentru a curăța fișierele în exces, --rata limită de viteză de 50 MB)
sync0 send ./project 192.168.1.10:9000
```

### 2. Partajarea postărilor și extragerea/încărcarea la cerere (`share` / `pull` / `push`)

```bash
# Server: publicați grupul de partajare cu o singură linie de comandă (ieșiți adresa nodului iroh fără IP după pornire)
sync0 share dataset:./data

# Client: trageți întregul grup de partajare a setului de date în ./local_data local
sync0 pull dataset:// ./local_data -p <adresa nodului>

# Client: extrage selecții multiple la cerere (sincronizează doar fișierele și subdirectoarele specificate)
sync0 pull dataset://docs/readme.md,assets/icons,src/core ./local_data -p <adresa nodului>

# Client: Încărcați fișiere sau directoare în grupuri de partajare autorizate
sync0 push dataset://releases/v1 ./build_output -p <adresa nodului>
```

### 3. Seeding de abonament în timp real și sincronizare bidirecțională cu mai multe mașini (`sub` / `group`)

După stabilirea unei conexiuni lungi, dacă orice fișier **adăugarea, modificarea, ștergerea, modificarea permisiunilor sau timpului de modificare** are loc la distanță, directorul local va fi actualizat sincron în milisecunde în timp real (fișierele mari noi/modificate vor trage doar blocurile de diferență și le vor actualiza la locul lor, ștergerea fișierelor va curăța automat fișierele locale și directoarele goale și va modifica direct blocurile de date zero sau ștampila timp):

- **`sync0 sub`(Oglindire unidirecțională în timp real + seeding automat)**: în timp real local urmează adăugările, ștergerile și modificările finalului de partajare și servește automat ca sursă de inițiere P2P pentru a oferi accelerare concomitentă pentru alte noduri de tragere.
- **`sync0 group`(Sincronizare peer-to-peer bidirecțională în timp real)**: dacă fișierele sunt adăugate, șterse sau modificate pe orice mașină din grup, toate celelalte noduri online vor fi actualizate sincron în timp real și vor servi drept semințe unul pentru celălalt.

```bash
# Abonați-vă la întregul grup de partajare: adăugările, ștergerile și modificările de la distanță sunt sincronizate local în timp real și înscrise automat
sync0 sub dataset:// ./local_mirror -p <adresa nodului>

# Abonament cu selecție multiplă la cerere: sincronizați și generați numai fișierele și subdirectoarele specificate în timp real
sync0 sub dataset://models/llama.onnx,configs ./local_mirror -p <adresa nodului>

# Sincronizare bidirecțională în timp real a grupului de egali cu mai multe mașini (accepta, de asemenea, căi cu selecție multiplă completă sau prin virgulă)
sync0 group team-docs:// ./docs -p <adresa nodului>
```

### 4. Configurarea grupului de partajare multiplă pentru echipă și a permisiunii membrilor (`.nt` configurație)

Directorul de configurare implicit este `~/.config/sync0` (poate fi comutat prin `-c <directory>`), folosind formatul minimalist [NestedText](https://nestedtext.org) (`.nt`):
- **Acceptă restricționarea citirii în rețeaua publică (numai utilizatorii autorizați pot citi)**: atâta timp cât grupul de partajare este configurat cu o listă de membri `users:` (sau linia de comandă specifică `--allow-r` / `--allow-rw` / `--allow-w` / `--auth`), sistemul **activează automat o rețea privată autentificată="identificație publică) enumerarea sau citirea nodurilor și Abonarea P2P la nodurile de seeding vor moșteni, de asemenea, lista albă simultan, permițând doar utilizatorilor autorizați să citească; grupurile de partajare care nu sunt configurate cu `users` sunt publice și numai în citire în mod prestabilit (dacă doriți să fie publice și numai în citire și deschise pentru încărcări numai pentru membri specificați, puteți seta în mod explicit `auth: false`).
- **Permisiuni detaliate de citire-scriere și de livrare a subdirectorului**: membrii sunt numai citire în mod implicit (`r` nu trebuie să fie scrise în mod explicit) și acceptă `r` (doar citire), `w` (scrie doar în caseta drop-box, nu poate fi citită), `rw` și `rw` direct (prefixe de citire = "write)" permis pentru încărcare).

**Exemplu de configurare membru (`~/.config/sync0/users/`):**

```yaml
# ~/.config/sync0/users/alice.nt (numai citire implicită, limită de viteză 80 MB)
id: 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c
rate: 80MB
```

```yaml
# ~/.config/sync0/users/bob.nt (permisiuni de citire și scriere, permise numai încărcarea în subdirectorul specificat)
id: 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f
perm: rw
write_prefix:
  - uploads/bob
  - shared/common
```

```yaml
# ~/.config/sync0/users/charlie.nt (doar permisiunea de livrare a scrierii, citirea este interzisă)
id: 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
perm: w
```

**Exemplu de configurare a grupului de partajare (`~/.config/sync0/config.nt`):**

```yaml
users_dir: ./users

groups:
  # Utilizatori neconfigurați: rețea publică implicită numai pentru citire
  dataset:
    path: ./data
    rate: 50MB

  # Utilizatori configurați: restricționează automat citirea în rețeaua publică (numai Alice și Bob pot citi, Charlie poate livra și încărca doar)
  team-vault:
    path: ./vault
    users:
      - alice
      - bob
      - charlie
```

Porniți serviciul (încărcați fișierul de configurare sau utilizați direct parametrii liniei de comandă pentru a restricționa citirea rețelei publice și a autoriza membrii):

```bash
# Încărcați ~/.config/sync0/config.nt pentru a porni
sync0 share --region SHA

# Sau publicați un grup de partajare privat prin linia de comandă pură (specificați --allow-* pentru a restricționa automat citirea rețelei publice și numai utilizatorii autorizați îl pot accesa)
sync0 share team-vault:./vault \
  --allow-r 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c \
  --allow-rw 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f:uploads/bob,shared/common \
  --allow-w 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
```

### 5. Adresă de nod, carte de vizită și multi-limbi

```bash
# Vizualizați adresa nodului local fără IP (cheia este generată automat pentru prima rulare)
sync0 id

# Generați o carte de vizită cu nod autosemnată Ed25519 (porecla, codul orașului, e-mail, site web, introducere)
sync0 profile -u alice --region SHA --email alice@example.com -b "Shanghai node"

# Există 75 de limbi încorporate, care urmează limba sistemului în mod implicit și pot fi comutate prin variabilele de mediu.
SYNC0_LANG=en sync0 --help
```

## Performanță și caracteristici de bază

### Măsurare reală WAN intercontinentală (Comparație `rsync`)

Mediu real de măsurare: Statele Unite `c1` → Europa `n1` (RTT 65 ms), setul de date este nucleul Linux `linux-7.3.3`:

#### 1) Modul a la carte vs `rsync`

| Set de date | dimensiune | `sync0` Punct unic<br>(Mbps) | `rsync`<br>(Mbps) | Accelerare (`sync0` / `rsync`) |
| :--- | ---: | ---: | ---: | ---: |
| `linux-7.3.3.tar.xz`(nu este compresibil) | 155 MB | **121.07** | 107.89 | **1,12×** |
| `linux-7.3.3.tar`(compresibil) | 1.6 GB | **442.07** | 506.62 | **0,87×** |
| `linux-7.3.3.tar`(increment de modificare locală) | 1.6 GB | **8,123.87** | 1,169.49 | **6,95×** |
| `linux-7.3.3/` (95.979 fișiere mici) | 1.5 GB | **565.49** | 235.27 | **2,40×** |
| **medie geometrică** | — | **704.17** | 350.19 | **2,01×** |

#### 2) Modul P2P cu sursă duală față de `rsync`

| Set de date | dimensiune | `sync0` P2P Dual Source<br>(Mbps) | `rsync`<br>(Mbps) | Accelerare (`sync0` / `rsync`) |
| :--- | ---: | ---: | ---: | ---: |
| `linux-7.3.3.tar.xz`(nu este compresibil) | 155 MB | **466.85** | 107.89 | **4,33×** |
| `linux-7.3.3.tar`(compresibil) | 1.6 GB | **992.17** | 506.62 | **1,96×** |
| `linux-7.3.3.tar`(increment de modificare locală) | 1.6 GB | **5,666.84** | 1,169.49 | **4,85×** |
| `linux-7.3.3/` (95.979 fișiere mici) | 1.5 GB | **703.30** | 235.27 | **2,99×** |
| **medie geometrică** | — | **1,165.63** | 350.19 | **3,33×** |

> Notă: în modul P2P cu sursă duală, nodurile europene `s1` colaborează pentru însămânțare, iar RTT-ul intercontinental este de 65 ms.

### Caracteristicile de bază ale produsului

- **Perforarea automată a cheii publice de adresare**: Nu este nevoie de un server central și de o adresă de rețea publică, perforarea automată a găurilor NAT bazată pe adresa cheii publice ale rețelei distribuite.
- **Sincronizare în timp real la nivel de milisecunde și cunoaștere a metadatelor**: pe baza monitorizării native a evenimentelor fișierului (`FSEvents` / `inotify` / `ReadDirectoryChangesW`), introducere în timp real a adăugărilor de fișiere, ștergeri, permisiuni POSIX (`mode`) și timp de modificare la nivel de milisecunde (`mtime`); zero transmisie bloc de date atunci când sunt modificate numai permisiunile sau ora.
- **Descărcare și încărcare simultane în stil BT și programare de afinitate regională**: Nodul de descărcare nu trebuie să aștepte pentru descărcarea întregii sume. Toate intervalele de bloc (`Op::Have`) care au trecut verificarea hash pe 128 de biți sunt blocate imediat în pool-ul de memorie/temporar pentru ca alte noduri să poată fi extrase concomitent. Împreună cu schimbul dinamic de noduri PEX și preempțiunea la jumătatea nodului lentă (Work-Stealing), debitul clusterului este maximizat.
- **Modificarea nodurilor anti-răușitoare și imunitate la otrăvirea clusterelor**: listă de surse autorizate `root_hash` Blocare auto-consecventă + accelerare hardware obligatorie `gxhash128` verificare digerată înainte ca fiecare bloc să fie plasat pe disc și însămânțat extern; dacă se găsește un bloc alterat, întrerupătorul de circuit la nivel de milisecunde va întrerupe nodul rău intenționat și va reveni automat de la sursa sinceră.
- **FastCDC increment in-situ și agregare de fișiere mici**: Numai blocurile diferențiale sunt transferate pentru modificarea parțială a fișierelor mari și plasate pe disc; Ambalarea masivă a memoriei de fișiere mici și compresia diferențială a prefixului elimină supraîncărcarea dus-întors fișier cu fișier.
- **Auto-vindecare după pană de curent și reluare după repornire**: formatul hibrid RDB + AOF, asemănător Redis (`.s0_subs.bin`), este folosit pentru a menține starea de seeding și lista de fișiere, modificările zilnice sunt adăugate în microsecunde și sunt rescrise automat și organizate în mod regulat. După ce serverul este repornit, modificările offline sunt trimise automat în nodul de seeding.

## Arhitectura tehnica si documentatia

- **Protocol de decuplare cu două planuri**:
  - **Suprafață de control și schimbare în timp real (`iroh` QUIC / TCP, primul octet `PROTO_VERSION` strângere de mână versiunea) **: responsabil pentru verificarea cărților de vizită Ed25519, verificarea permisiunii, descoperirea semințelor Swarm și suport pentru loturi de flux de modificare a nivelului `u64::MAX` și `mzstd` comprimat în timp real`mzstd` id="5 `FsOp::Remove` / `FsOp::Meta` Trei primitive).
  - **Suprafață de distribuție a datelor (UDP nativ + `compio` I/O asincron) **: antet compact 20B, feedback periodic TACK 18B, controlul congestiei hibrid BBRv3/Copa, recuperarea pierderii pachetelor RACK-TLP, bypass de compresie Zstd adaptiv al entropiei Shannon de la primul bloc și blocul de verificare la finalul <b id-="2">
- **Află mai multe**:
  - [Raport de măsurare a performanței și script de reproducere](https://github.com/webc-site/sync0/blob/main/doc/zh/benchmark.md)
  - [Proiectarea arhitecturii sistemului](https://github.com/webc-site/sync0/blob/main/doc/zh/design_v4.md)
  - [Protocolul firului binar și specificația de persistență](https://github.com/webc-site/sync0/blob/main/doc/zh/proto.md)
  - [Autentificarea identității, controlul autorității și verificarea integrității](https://github.com/webc-site/sync0/blob/main/doc/zh/crypto.md)