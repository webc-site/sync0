# sync0

`sync0` és una eina per compartir fitxers i sincronització de diverses màquines entre iguals (P2P) que competeix amb Resilio Sync.

Cada màquina és alhora un servidor i un client: qualsevol node pot publicar diversos grups per compartir fitxers. Després que altres nodes s'uneixin, poden obtenir automàticament nodes llavor disponibles del servidor igual i extreure'ls simultàniament de diverses fonts. Un cop finalitzada la sincronització, es crearan llavors automàticament per a més nodes per accelerar les descàrregues.

## Aspectes principals

- **Tothom és alhora un servidor i un client**: comparable a Resilio Sync, sense dependència del servidor central. Qualsevol node pot crear diversos grups per compartir fitxers i també es pot unir a grups per compartir fitxers d'altres persones; els nodes intercanvien i obtenen automàticament nodes llavor disponibles per aconseguir l'extracció simultània i la sincronització en temps real de múltiples màquines.
- **Adreça de node sense IP**: de manera predeterminada, la clau de node i l'adreça `base64url` codificada `iroh` es generen automàticament a `~/.config/sync0` sense necessitat de recordar ni exposar la IP pública.
- **Sincronització de selecció múltiple sota demanda**: admet la sincronització de tot el grup compartit i també admet la sincronització a demanda seleccionant diversos fitxers o subdirectoris alhora.
- **Permisos jeràrquics dels membres del grup i control del directori de càrrega**: admet permisos de només lectura (`ro`), de càrrega de lectura-escriptura (`rw`) i de gestió (`admin`), i pot especificar individualment el prefix de camí que cada membre pot carregar (per defecte, la càrrega del directori compartit està permesa).
- **Sincronització incremental i a tota velocitat entre xarxes**: transmissió simultània de fitxers grans, agregació de fitxers petits massius a la memòria i només blocs de dades sincronitzats quan els fitxers es modifiquen parcialment.

## Comparació de velocitat amb rsync

### Mesura real de font única de xarxa transoceànica (Xina → Estats Units, retard de 260 ms)

| Escenari de transmissió | Mida de les dades | model | `sync0` | `rsync -az` | Augmentar el multiplicador |
| :--- | :---: | :---: | :---: | :---: | :---: |
| 500 fitxers petits | 1.91 MB | Sincronització total | 18.84 Mbps | 5.32 Mbps | 3.5x |
| 500 fitxers petits | 1.91 MB | Sincronització incremental | 27.15 Mbps | 3.83 Mbps | 7.1x |
| Codi i paquets d'arxiu (`.tar`) | 10.0 MB | Sincronització total | 139.70 Mbps | 24.97 Mbps | 5.6x |
| Codi i paquets d'arxiu (`.tar`) | 10.0 MB | Sincronització incremental | 83.38 Mbps | 26.02 Mbps | 3.2x |
| Fitxer de vídeo (`.mp4`) | 15.0 MB | Sincronització total | 54.06 Mbps | 27.04 Mbps | 2.0x |
| Fitxer de vídeo (`.mp4`) | 15.0 MB | Sincronització incremental | 216.83 Mbps | 21.51 Mbps | 10.1x |

### Prova d'extracció de múltiples nodes P2P de xarxa transoceànica (nodes duals Xina → Estats Units, retard 260 ms)

Comparació mesurada de l'extracció simultània de ratlles de nodes de llavors multifons:

| Escenari de transmissió | Mida de les dades | Sembra d'un sol node | Sembra concurrent P2P de dos nodes | Augmentar el multiplicador |
| :--- | :---: | :---: | :---: | :---: |
| 500 fitxers petits | 1.97 MB | 5.28 Mbps | 7.87 Mbps | 1.5x |
| Codi i paquets d'arxiu (`.tar`) | 30.0 MB | 56.35 Mbps | 83.17 Mbps | 1.5x |
| Fitxer de vídeo (`.mp4`) | 30.0 MB | 27.00 Mbps | 39.46 Mbps | 1.5x |

### Mesura real de la línia dedicada transcontinental d'alta velocitat (Europa → Amèrica del Nord, retard de 86 ms)

| Escenari de transmissió | Mida de les dades | `sync0` | `rsync` | Augmentar el multiplicador |
| :--- | :---: | :---: | :---: | :---: |
| 10.000 fitxers petits | 77.6 MB | 688 Mbps | 128 Mbps | 5.4x |
| Codi i paquets d'arxiu (`.tar`) | 512 MB | 2111 Mbps | 438 Mbps | 4.8x |
| Fitxer de vídeo (`.mp4`) | 512 MB | 556 Mbps | 338 Mbps | 1.6x |
| Sincronització incremental (modifica el contingut de l'1%) | 512 MB | 34133 Mbps | 2214 Mbps | 15.4x |

## Instal·lar

### Mètode 1: instal·lar mitjançant càrrega

```bash
cargo install sync0
```

### Mètode 2: Baixeu binaris precompilats

Aneu a [Versions de GitHub](https://github.com/webc-site/sync0/releases) per descarregar el paquet comprimit `.tar.xz` per al sistema corresponent (admet Linux, macOS, Windows, Android, FreeBSD, NetBSD), descomprimiu-lo i col·loqueu-lo al directori `PATH`:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Comença ràpidament

`sync0` utilitza `~/.config/sync0` com a directori de configuració de manera predeterminada (si necessiteu canviar a altres directoris, podeu afegir `-c <directory>`).

### 1. Consulteu l'adreça del node local i la configuració de la targeta de visita

```bash
# Veure l'adreça del node local (la clau es genera automàticament a ~/.config/sync0 per primera vegada)
sync0 id

# Genereu una targeta de visita de node personal (admet nom d'usuari, ciutat, correu electrònic, lloc web i signatura en 256 paraules)
sync0 profile -u alice --region SHA --email alice@example.com --url https://example.com -b "Shanghai node"
```

### 2. Publicar diversos grups per compartir fitxers i establir els permisos dels membres del grup (`.nt` Configuració)

`sync0` està configurat en el format [NestedText](https://nestedtext.org) (`.nt`). Cada usuari té un fitxer de configuració `.nt` independent. Podeu especificar el directori del perfil d'usuari mitjançant `users_dir` a `config.nt` (que inclou automàticament tots els fitxers `.nt` d'aquest directori). Tant l'usuari com el prefix del camí que es permet penjar utilitzen una llista no ordenada (`-`). **Quan el prefix del camí de càrrega no s'omple, es permet la càrrega al directori arrel de la compartició de manera predeterminada**.

1. Creeu fitxers de configuració `.nt` separats per a cada membre del directori `~/.config/sync0/users/`:

`~/.config/sync0/users/alice.nt` (membre només de lectura, tarifa limitada `80MB`, `rate` admet unitats `GB`, `MB`, `KB`):

```yaml
id: 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c
perm: ro
rate: 80MB
```

`~/.config/sync0/users/bob.nt` (membres de càrrega restringida: només es permet penjar als directoris `uploads/bob` i `shared/common`):

```yaml
id: 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f
perm: rw
write_prefix:
  - uploads/bob
  - shared/common
```

`~/.config/sync0/users/charlie.nt` (Càrrega de membres per defecte: quan s'omet `write_prefix`, la càrrega al directori arrel de la compartició es permet de manera predeterminada):

```yaml
id: 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
perm: rw
```

2. Configureu el directori d'usuaris i diversos grups per compartir fitxers a `~/.config/sync0/config.nt`:

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

Comenceu a compartir el servei:

```bash
# Inici directe (carrega automàticament totes les configuracions d'usuari .nt als directoris ~/.config/sync0/config.nt i user_dir)
sync0 share --region SHA

# O publiqueu ràpidament diversos grups per compartir i autoritzeu els membres del grup directament mitjançant la línia d'ordres
sync0 share dataset:./data team-vault:./vault \
  --auth \
  --allow-ro 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c \
  --allow-rw 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f:uploads/bob,shared/common \
  --allow-rw 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
```

### 3. Extraieu tot el contingut o seleccioneu diversos fitxers i directoris (`sync0 pull`)

Connecteu-vos a qualsevol node d'igual per obtenir automàticament tots els nodes llavor disponibles al grup de compartició i extreu-vos de diversos nodes llavor simultàniament:

```bash
# Traieu tot el grup per compartir el conjunt de dades al local ./local_data
sync0 pull dataset:// ./local_data -p <adreça del node>

# Sincronització de selecció múltiple: traieu simultàniament el fitxer únic docs/readme.md i subdirectoris actius/icones, src/core
sync0 pull dataset://docs/readme.md,assets/icons,src/core ./local_data -p <adreça del node>
```

### 4. Penja un fitxer o un directori a un grup per compartir (`sync0 push`)

Els membres amb permisos de càrrega (`rw` o `admin`) poden penjar fitxers o directoris locals a grups de compartició remota:

```bash
# Membres amb permís de càrrega del directori arrel: carregueu directament al directori arrel compartit
sync0 push team-vault:// ./new_release -p <adreça del node>

# Només els membres autoritzats per a un prefix específic (com ara càrregues/bob): penja al prefix de camí permès
sync0 push team-vault://uploads/bob/report.pdf ./report.pdf -p <adreça del node>
```

### 5. Subscriu-te a la sincronització i la sembra automàtica (`sync0 sub`)

Després de subscriure's al grup de compartició d'un node determinat, el servidor local és alhora un client receptor i un nou servidor de sembra; quan s'actualitza el fitxer del node font, es sincronitza automàticament i proporciona descàrregues fragmentades per a altres nodes:

```bash
# Subscriu-te a tot el grup per compartir i comença automàticament
sync0 sub dataset:// ./local_mirror -p <adreça del node>

# Només seleccioneu múltiple per subscriure-us als fitxers i subdirectoris especificats i crear-los
sync0 subdataset://models/llama.onnx,configs ./local_mirror -p <adreça del node>
```

### 6. Sincronització entre iguals de grups de diverses màquines (`sync0 group`)

Diverses màquines s'uneixen al mateix grup de compartició, entre elles descobreixen automàticament els nodes de llavor disponibles i mantenen la sincronització bidireccional (admet la sincronització completa o la selecció múltiple de fitxers i directoris):

```bash
# Uneix-te completament al grup i sincronització bidireccional
sync0 group team-docs:// ./docs -p <adreça del node>

# Només el directori wal i el fitxer meta.json del grup de sincronització de selecció múltiple
sync0 group team-docs://wal,meta.json ./docs -p <adreça del node>
```

### 7. Transmissió directa punt a punt entre dues màquines (`send` / `recv`)

No cal preconfigurar un grup i podeu transferir fitxers o directoris temporalment directament entre dues màquines:

```bash
# Monitorització del receptor
sync0 recv 0.0.0.0:9000 ./backup

# L'extrem d'enviament transfereix directament fitxers o directoris (opcional --delete mirall per netejar l'excés de fitxers, --rate límit de velocitat)
sync0 send ./project 192.168.1.10:9000
```

### 8. Canvi multi-idioma

`sync0` admet 75 idiomes i canvia automàticament segons l'entorn d'idioma del sistema per defecte; també es pot especificar manualment mitjançant `SYNC0_LANG`:

```bash
SYNC0_LANG=en sync0 --help
SYNC0_LANG=ja sync0 --help
SYNC0_LANG=de sync0 --help
```