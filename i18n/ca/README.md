# sync0

`sync0` és una eina descentralitzada per compartir fitxers P2P, sincronització en temps real i distribució de fonts múltiples que s'utilitza per substituir `rsync`: la velocitat mesurada de la xarxa d'àrea àmplia transcontinental és de fins a **6,95×** de `rsync` (velocitat mitjana geomètrica d'un sol punt **2,01×**), velocitat mitjana de sincronització geomètrica** i P2P3**-3**, font. fitxers petits massius a través dels oceans és tan ràpid com **7,19×**.

Cada màquina és alhora un servidor i un client: no hi ha necessitat d'un servidor central ni d'una adreça de xarxa pública, la perforació automàtica es basa en l'adreça de la clau pública de la xarxa distribuïda i admet la transmissió directa entre dues màquines, allotjament de permisos de grup per compartir múltiples, selecció múltiple de subdirectoris sota demanda, sincronització incremental en temps real de mil·lisegons i acceleració concurrent P2P multifont.

## Instal·lar

### Mètode 1: Instal·lació de càrrega

```bash
cargo install sync0
```

### Mètode 2: Baixeu binaris precompilats

Aneu a [Versions de GitHub](https://github.com/webc-site/sync0/releases) per descarregar el paquet comprimit `.tar.xz` corresponent al sistema (admet Linux, macOS, Windows, Android, iOS, FreeBSD), descomprimiu-lo i col·loqueu-lo a `PATH`:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Comença ràpidament

### 1. Transmissió directa de configuració zero entre dues màquines (`send` / `recv`)

No cal configurar un grup per compartir, transferir directament fitxers o directoris entre dues màquines a gran velocitat:

```bash
# El receptor escolta al port i escriu a ./backup
sync0 recv 0.0.0.0:9000 ./backup

# Transmissió directa des del remitent (opcional --eliminar mirall per netejar l'excés de fitxers, --rate límit de velocitat de 50 MB)
sync0 send ./project 192.168.1.10:9000
```

### 2. Compartició de publicacions i extracció/càrrega sota demanda (`share` / `pull` / `push`)

```bash
# Servidor: publiqueu el grup de compartició amb una línia d'ordres (emet l'adreça del node iroh sense IP després de l'inici)
sync0 share dataset:./data

# Client: traieu tot el grup per compartir el conjunt de dades al local ./local_data
sync0 pull dataset:// ./local_data -p <adreça del node>

# Client: extreu múltiples seleccions a demanda (només sincronitzeu fitxers i subdirectoris individuals especificats)
sync0 pull dataset://docs/readme.md,assets/icons,src/core ./local_data -p <adreça del node>

# Client: carregueu fitxers o directoris a grups de compartició autoritzats
sync0 push dataset://releases/v1 ./build_output -p <adreça del node>
```

### 3. Generació de subscripcions en temps real i sincronització bidireccional de diverses màquines (`sub` / `group`)

Després d'establir una connexió llarga, si es produeix algun fitxer **afegir, modificar, suprimir, canviar els permisos o el temps de modificació** a l'extrem remot, el directori local s'actualitzarà de manera sincrònica en mil·lisegons en temps real (els fitxers grans nous/modificats només trauran els blocs de diferència i els actualitzaran al seu lloc, suprimir els fitxers netejarà automàticament els fitxers locals i els directoris buits, i només canviarà directament el bloc de permisos zero o marca de temps):

- **`sync0 sub`(Replica en temps real unidireccional + sembra automàtica)**: el temps real local segueix les addicions, les supressions i les modificacions del final de la compartició i serveix automàticament com a font de llavor P2P per proporcionar acceleració simultània per a altres nodes d'extracció.
- **`sync0 group`(Sincronització bidireccional d'igual a punt de diverses màquines en temps real)**: si s'afegeixen, se suprimeixen o es modifiquen fitxers a qualsevol màquina del grup, la resta de nodes en línia s'actualitzaran de manera sincrònica en temps real i serviran com a llavor els uns dels altres.

```bash
# Subscriu-te a tot el grup per compartir: les addicions, les supressions i les modificacions remotes es sincronitzen localment en temps real i es llamen automàticament
sync0 sub dataset:// ./local_mirror -p <adreça del node>

# Subscripció de selecció múltiple sota demanda: només sincronitzeu i engegueu fitxers i subdirectoris específics en temps real
sync0 subdataset://models/llama.onnx,configs ./local_mirror -p <adreça del node>

# Sincronització bidireccional en temps real de grups d'iguals multimàquina (també admet camins de selecció múltiple complets o per comes)
sync0 group team-docs:// ./docs -p <adreça del node>
```

### 4. Configuració de permisos per a membres i grups per compartir diversos equips (`.nt` configuració)

El directori de configuració predeterminat és `~/.config/sync0` (es pot canviar mitjançant `-c <directory>`), utilitzant el format minimalista [NestedText](https://nestedtext.org) (`.nt`):
- **Admet la restricció de la lectura de la xarxa pública (només els usuaris autoritzats poden llegir)**: sempre que el grup de compartició estigui configurat amb una llista de membres `users:` (o la línia d'ordres especifiqui `--allow-r` / `--allow-rw` / `--allow-w` / `--auth`), el sistema **activa automàticament la xarxa privada autèntica i **identificació pública. enumeració o lectura de nodes, i la subscripció P2P als nodes de sembra també heretarà la llista blanca simultàniament, permetent que només els usuaris autoritzats puguin llegir; els grups per compartir que no estan configurats amb `users` són públics i només de lectura per defecte (si voleu ser públics i només de lectura i oberts a càrregues només per a membres especificats, podeu establir `auth: false` explícitament).
- **Permisos de lectura-escriptura i de lliurament de subdirectoris**: els membres són només de lectura per defecte (`r` no cal escriure explícitament) i admeten `r` (només lectura), `w` (només escriviu al bústia, no es pot llegir), `rw` i `rw` directe (prefixos de lectura) i <b-id="write"> permesos per a la càrrega).

**Exemple de configuració del membre (`~/.config/sync0/users/`):**

```yaml
# ~/.config/sync0/users/alice.nt (només de lectura per defecte, límit de velocitat 80 MB)
id: 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c
rate: 80MB
```

```yaml
# ~/.config/sync0/users/bob.nt (permisos de lectura i escriptura, només es permet pujar al subdirectori especificat)
id: 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f
perm: rw
write_prefix:
  - uploads/bob
  - shared/common
```

```yaml
# ~/.config/sync0/users/charlie.nt (només permís de lliurament d'escriptura, la lectura està prohibida)
id: 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
perm: w
```

**Exemple de configuració de grup compartit (`~/.config/sync0/config.nt`):**

```yaml
users_dir: ./users

groups:
  # Usuaris no configurats: xarxa pública predeterminada només de lectura
  dataset:
    path: ./data
    rate: 50MB

  # Usuaris configurats: restringeix automàticament la lectura de la xarxa pública (només l'Alice i el Bob poden llegir, Charlie només pot lliurar i carregar)
  team-vault:
    path: ./vault
    users:
      - alice
      - bob
      - charlie
```

Inicieu el servei (carregueu el fitxer de configuració o utilitzeu directament els paràmetres de la línia d'ordres per restringir la lectura de la xarxa pública i autoritzar els membres):

```bash
# Carregueu ~/.config/sync0/config.nt per començar
sync0 share --region SHA

# O publiqueu un grup de compartició privat mitjançant una línia d'ordres pura (especifiqueu --allow-* per restringir automàticament la lectura de la xarxa pública i només els usuaris autoritzats hi poden accedir)
sync0 share team-vault:./vault \
  --allow-r 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c \
  --allow-rw 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f:uploads/bob,shared/common \
  --allow-w 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
```

### 5. Adreça de node, targeta de visita i multi-idioma

```bash
# Veure l'adreça del node local sense IP (la clau es genera automàticament per a la primera execució)
sync0 id

# Genereu la targeta de visita del node autosignada Ed25519 (àlies, codi de ciutat, correu electrònic, lloc web, introducció)
sync0 profile -u alice --region SHA --email alice@example.com -b "Shanghai node"

# Hi ha 75 idiomes integrats, que segueixen l'idioma del sistema per defecte i es poden canviar mitjançant variables d'entorn.
SYNC0_LANG=en sync0 --help
```

## Rendiment i característiques bàsiques

### Mesura real de la WAN intercontinental (comparació `rsync`)

Entorn de mesura real: Estats Units `c1` → Europa `n1` (RTT 65 ms), el conjunt de dades és el nucli de Linux `linux-7.3.3`:

#### 1) Mode a la carta vs `rsync`

| Conjunt de dades | mida | `sync0` Punt únic<br>(Mbps) | `rsync`<br>(Mbps) | Acceleració (`sync0`/`rsync`) |
| :--- | ---: | ---: | ---: | ---: |
| `linux-7.3.3.tar.xz`(no compressible) | 155 MB | **121.07** | 107.89 | **1,12×** |
| `linux-7.3.3.tar`(comprimible) | 1.6 GB | **442.07** | 506.62 | **0,87×** |
| `linux-7.3.3.tar`(increment de canvi local) | 1.6 GB | **8,123.87** | 1,169.49 | **6,95×** |
| `linux-7.3.3/` (95.979 fitxers petits) | 1.5 GB | **565.49** | 235.27 | **2,40×** |
| **mitjana geomètrica** | — | **704.17** | 350.19 | **2,01×** |

#### 2) Mode de font dual P2P vs `rsync`

| Conjunt de dades | mida | `sync0` Font dual P2P<br>(Mbps) | `rsync`<br>(Mbps) | Acceleració (`sync0`/`rsync`) |
| :--- | ---: | ---: | ---: | ---: |
| `linux-7.3.3.tar.xz`(no compressible) | 155 MB | **466.85** | 107.89 | **4,33×** |
| `linux-7.3.3.tar`(comprimible) | 1.6 GB | **992.17** | 506.62 | **1,96×** |
| `linux-7.3.3.tar`(increment de canvi local) | 1.6 GB | **5,666.84** | 1,169.49 | **4,85×** |
| `linux-7.3.3/` (95.979 fitxers petits) | 1.5 GB | **703.30** | 235.27 | **2,99×** |
| **mitjana geomètrica** | — | **1,165.63** | 350.19 | **3,33×** |

> Nota: en el mode de font dual P2P, els nodes europeus `s1` col·laboren per a la sembra, i l'RTT de la columna vertebral intercontinental és de 65 ms.

### Característiques bàsiques del producte

- **Perforació automàtica d'adreça de clau pública**: no cal un servidor central i una adreça de xarxa pública, perforació automàtica de forats NAT basada en l'adreça de la clau pública de la xarxa distribuïda.
- **Sincronització en temps real a nivell de mil·lisegons i consciència de metadades**: basat en la supervisió nativa d'esdeveniments de fitxers (`FSEvents` / `inotify` / `ReadDirectoryChangesW`), incorporació de fitxers en temps real, supressió, permisos POSIX (`mode`) i temps de modificació de nivell de mil·lisegons (`mtime`); transmissió de bloc de dades zero quan només es modifiquen els permisos o l'hora.
- **Descàrrega i càrrega simultànies a l'estil BT i programació d'afinitat regional**: el node de descàrrega no ha d'esperar a que es descarregui la quantitat completa. Tots els intervals de bloc (`Op::Have`) que han superat la comprovació hash de 128 bits es bloquegen immediatament a la memòria/agrupació temporal perquè altres nodes puguin extreure's simultàniament. Juntament amb l'intercanvi dinàmic de nodes PEX i la preempció de la meitat del node lenta (robar de treball), es maximitza el rendiment del clúster.
- **Alteració de nodes anti-maliciós i immunitat a l'enverinament del clúster**: llista de fonts autoritzades `root_hash` Bloqueig autònom + acceleració de maquinari obligatòria `gxhash128` verificació del resum abans de col·locar cada bloc al disc i sembrar externament; si es troba un bloc manipulat, l'interruptor de nivell de mil·lisegon apagarà el node maliciós i es recuperarà automàticament de la font honesta.
- **Increment FastCDC in situ i agregació de fitxers petits**: només es transfereixen blocs diferencials per a la modificació parcial de fitxers grans i es col·loquen al disc; L'embalatge massiu de memòria de fitxers petits i la compressió diferencial de prefix eliminen la sobrecàrrega d'anada i tornada fitxer per fitxer.
- **Autocuració després d'un tall d'alimentació i reinici després del reinici**: el format híbrid RDB + AOF semblant a Redis (`.s0_subs.bin`) s'utilitza per mantenir l'estat de sembra i la llista de fitxers, els canvis diaris s'afegeixen en microsegons i es reescriuen i s'organitzen automàticament amb regularitat. Després de reiniciar el servidor, els canvis fora de línia s'envien automàticament al node de sembra.

## Arquitectura tècnica i documentació

- **Protocol de desacoblament de doble pla**:
  - **Superfície de control i canvi en temps real (`iroh` QUIC / TCP, primer byte `PROTO_VERSION` versió encaixada) **: Responsable de la verificació de targetes de visita Ed25519, verificació de permisos, descobriment de llavors Swarm i suport per a la transmissió per lots de canvi massiu de nivell `mzstd` i `mzstd` comprimit en temps real `mzstd` id="6 `FsOp::Remove` / `FsOp::Meta` Tres primitives).
  - **Superfície de distribució de dades (UDP + `compio` E/S asíncrona nativa) **: capçalera compacta 20B, retroalimentació periòdica TACK 18B, control de congestió híbrid BBRv3/Copa, recuperació de pèrdua de paquets RACK-TLP, bypass de compressió Zstd adaptatiu d'entropia Shannon del primer bloc i bloqueig de verificació final <b id-="2">
- **Més informació**:
  - [Informe de mesura del rendiment i guió de reproducció](https://github.com/webc-site/sync0/blob/main/doc/zh/benchmark.md)
  - [Disseny de l'arquitectura del sistema](https://github.com/webc-site/sync0/blob/main/doc/zh/design_v4.md)
  - [Protocol de fil binari i especificació de persistència](https://github.com/webc-site/sync0/blob/main/doc/zh/proto.md)
  - [Autenticació d'identitat, control d'autoritat i verificació d'integritat](https://github.com/webc-site/sync0/blob/main/doc/zh/crypto.md)