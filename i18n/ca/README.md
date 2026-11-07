# sync0

`sync0` és una eina de línia d'ordres per transferir fitxers a través de xarxes, sincronitzar de manera incremental directoris i distribuir dades simultàniament a diverses màquines.

## Intenció de disseny original

A les sales entre màquines, països o entorns amb retards de xarxa i pèrdua de paquets, eines com ara `rsync` i `scp` es basen en la confirmació de la seqüència de connexió única, que limita la utilització de l'ample de banda; quan diverses màquines treuen dades del mateix servidor alhora, la targeta de xarxa del servidor d'origen es pot convertir fàcilment en un coll d'ampolla.

L'objectiu de `sync0` és resoldre dos problemes:

1. Deixeu que la sincronització de fitxers i directoris entre les dues màquines executi ample de banda físic complet i només transferiu la part modificada quan el fitxer es modifiqui parcialment.
2. Diverses màquines poden transferir blocs de dades entre si quan sincronitzen el mateix directori, reduint la càrrega del servidor d'origen i escurçant el temps de sincronització global.

## Comparació de velocitat amb rsync

Entorn de prova: node europeu $\leftrightarrow$ node nord-americà, retard d'anada i tornada 86 ms, mesurant el temps complet inclòs l'exploració de directoris, la comparació de desduplicacions, la transmissió de xarxa i la verificació de la ubicació del disc:

| Escenari de transmissió | Mida de les dades | `sync0` | `rsync` | múltiples |
| :--- | :---: | :---: | :---: | :---: |
| 10.000 fitxers petits | 77.6 MB | 0.90 s (688 Mbps) | 4.86 s (128 Mbps) | 5.4x |
| Codi i paquets d'arxiu (`.tar`) | 512 MB | 1.94 s (2111 Mbps) | 9.36 s (438 Mbps) | 4.8x |
| Fitxer de vídeo (`.mp4`) | 512 MB | 7.36 s (556 Mbps) | 12.11 s (338 Mbps) | 1.6x |
| Sincronització incremental (modifica el contingut de l'1%) | 512 MB | 0.12 s | 1.85 s | 15.4x |

## Mètodes de millora del rendiment

- **Transmissió simultània UDP**: envieu blocs de dades a través d'UDP i només retransmeteu els fragments perduts per evitar que la pèrdua d'un sol paquet bloquegi les transferències de fitxers posteriors.
- **Segmentació i desduplicació del contingut**: divideix el fitxer en blocs de dades segons el contingut i calcula el valor de comprovació. El mateix contingut que ja està a l'extrem receptor es reutilitza directament localment sense duplicar la xarxa.
- **Ajusta automàticament la compressió segons la velocitat de la xarxa**: activa la compressió per a contingut compressible, com ara text i codi; saltar automàticament la compressió de vídeos, imatges, paquets comprimits o en LAN d'alta velocitat per reduir el consum de CPU.
- **Extracció propera de diversos nodes**: quan diverses màquines extreuen el mateix directori, els blocs de dades s'extrauran simultàniament dels nodes descarregats de la mateixa ciutat o amb una latència més baixa.

## Instal·lar

### Mètode 1: instal·lar mitjançant càrrega

```bash
cargo install sync0
```

### Mètode 2: Baixeu binaris precompilats

Aneu a [Versions de GitHub](https://github.com/webc-site/sync0/releases) per baixar el paquet comprimit `.tar.xz` per al sistema corresponent (admet Linux x86_64/ARM64, macOS Apple Silicon/Intel, Windows x64/ARM64, Android ARM64/x86_64, FreeBSD, NetBSD). Després de descomprimir, col·loqueu `sync0` al directori `PATH`:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Guia d'usuari

### 1. Transfereix directament fitxers o directoris entre dues màquines

Inicieu la supervisió a la màquina receptora i especifiqueu el camí de recepció i desat:

```bash
sync0 recv 0.0.0.0:9000 ./backup
```

Envieu un únic fitxer o un directori sencer des de la màquina remitent:

```bash
sync0 send ./project 192.168.1.10:9000
```

Paràmetres d'ús habitual:

```bash
# Suprimeix els fitxers redundants que existeixen a l'extrem receptor però no a l'extrem d'enviament (sincronització mirall)
sync0 send ./project 192.168.1.10:9000 --delete

# Limitar la velocitat d'enviament (unitat: bytes/segon, per exemple 50 MB/s)
sync0 send ./project 192.168.1.10:9000 --rate 50000000

# Mode silenciós (no es mostra cap barra de progrés)
sync0 send ./project 192.168.1.10:9000 -q
```

### 2. Publicació de directoris compartits i extracció de diversos nodes

Publiqueu un directori compartit (anomenat `dataset`) al servidor d'origen:

```bash
sync0 share dataset ./data --region SHA
```

Traieu el directori compartit en altres màquines (podeu omplir diverses adreces de nodes al mateix temps, separades per comes per extreure simultàniament):

```bash
sync0 pull 10.0.0.1:9000,10.0.0.2:9000 dataset ./local_data
```

Traieu només un determinat subdirectori al directori compartit:

```bash
sync0 pull 10.0.0.1:9000 dataset ./local_data --subpath 2026/logs
```

Torneu a enviar els canvis locals al directori compartit remot:

```bash
sync0 push 10.0.0.1:9000 dataset ./local_data
```

### 3. Subscriu-te a les actualitzacions automàtiques del catàleg

Subscriu-te al directori compartit del node font. Quan s'afegeix o es modifica un fitxer al directori d'origen, es sincronitzarà automàticament amb l'ordinador local. Al mateix temps, la màquina local servirà automàticament com a node perquè altres màquines puguin extreure:

```bash
sync0 sub 10.0.0.1:9000 dataset ./local_mirror
```

### 4. Sincronització bidireccional de grups de múltiples màquines

Diverses màquines s'uneixen al mateix nom de grup i els canvis en qualsevol directori de nodes es sincronitzaran automàticament amb altres nodes del grup:

```bash
sync0 group team-docs ./docs 10.0.0.1:9000,10.0.0.2:9000
```

### 5. Identitat del node i targeta de visita personal

Cada node admet la configuració del nom d'usuari, la ciutat, el correu electrònic, el lloc web i una signatura personalitzada (màxim 256 caràcters) i passa l'autenticació de signatura de clau privada local:

```bash
# Consulteu l'identificador de la clau pública local (especifiqueu el directori de configuració per mantenir la clau)
sync0 id -c ~/.config/sync0

# Genereu i visualitzeu targetes de visita de nodes amb signatures personalitzades
sync0 profile -c ~/.config/sync0 \
  -u alice \
  --region SHA \
  --email alice@example.com \
  --url https://example.com \
  -b "Shanghai backup node"
```

`share`, `pull`, `sub`, `group` també es poden iniciar amb aquests paràmetres de perfil:

```bash
sync0 share dataset ./data -c ~/.config/sync0 -u alice --region SHA -b "Shanghai backup node"
```

### 6. Canvi multi-idioma

`sync0` Admet 75 idiomes. Per defecte, l'ajuda de la línia d'ordres i les indicacions en l'idioma corresponent es mostren automàticament segons l'entorn d'idioma del sistema operatiu; si l'idioma del sistema no és a la llista admesa, s'utilitza l'anglès per defecte.

Per especificar l'idioma manualment, configureu la variable d'entorn `SYNC0_LANG`:

```bash
SYNC0_LANG=en sync0 --help
SYNC0_LANG=ja sync0 --help
SYNC0_LANG=de sync0 --help
```