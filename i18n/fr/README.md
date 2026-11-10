# sync0

`sync0` est un outil décentralisé de partage de fichiers P2P, de synchronisation en temps réel et de distribution multi-source utilisé pour remplacer `rsync` - la vitesse mesurée du réseau étendu transcontinental est jusqu'à **6,95×** de `rsync` (vitesse moyenne géométrique à un seul point **2,01×**, vitesse moyenne géométrique à double source P2P **3,33×**), et la synchronisation de petits fichiers massifs à travers les océans est aussi rapide que **7,19×**.

Chaque machine est à la fois un serveur et un client : il n'y a pas besoin d'un serveur central ou d'une adresse de réseau public, la perforation automatique est basée sur l'adresse de clé publique du réseau distribué et prend en charge la transmission directe entre deux machines, l'hébergement d'autorisations de groupe multi-partage, la sélection multiple de sous-répertoires à la demande, la synchronisation incrémentielle en temps réel au niveau de la milliseconde et l'accélération simultanée multi-source P2P.

## Installer

### Méthode 1 : Installation de la cargaison

```bash
cargo install sync0
```

### Méthode 2 : Télécharger les binaires précompilés

Accédez à [GitHub Releases](https://github.com/webc-site/sync0/releases) pour télécharger le package compressé `.tar.xz` correspondant au système (prend en charge Linux, macOS, Windows, Android, iOS, FreeBSD), décompressez-le et placez-le dans `PATH` :

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Commencez rapidement

### 1. Transmission directe sans configuration entre deux machines (`send` / `recv`)

Plus besoin de configurer un groupe de partage, transférez directement des fichiers ou des répertoires entre deux machines à haut débit :

```bash
# L'extrémité réceptrice écoute sur le port et écrit dans ./backup
sync0 recv 0.0.0.0:9000 ./backup

# Transmission directe depuis l'expéditeur (en option --delete miroir pour nettoyer les fichiers en excès, --rate limite de vitesse de 50 Mo)
sync0 send ./project 192.168.1.10:9000
```

### 2. Partage de publications et extraction/importation à la demande (`share` / `pull` / `push`)

```bash
# Serveur : publiez le groupe de partage avec une seule ligne de commande (affichez l'adresse du nœud iroh sans IP après le démarrage)
sync0 share dataset:./data

# Client : extrayez l'intégralité du groupe de partage d'ensemble de données vers le ./local_data local
sync0 pull dataset:// ./local_data -p <adresse du nœud>

# Client : extrayez plusieurs sélections à la demande (synchronisez uniquement les fichiers et sous-répertoires spécifiés)
sync0 pull dataset://docs/readme.md,assets/icons,src/core ./local_data -p <adresse du nœud>

# Client : Téléchargez des fichiers ou des répertoires vers des groupes de partage autorisés
sync0 push dataset://releases/v1 ./build_output -p <adresse du nœud>
```

### 3. Distribution des abonnements en temps réel et synchronisation bidirectionnelle multi-machines (`sub` / `group`)

Après avoir établi une longue connexion, si un fichier **ajoutant, modifiant, supprimant, modifiant les autorisations ou l'heure de modification** se produit à l'extrémité distante, le répertoire local sera mis à jour de manière synchrone en millisecondes en temps réel (les gros fichiers nouveaux/modifiés extrairont uniquement les blocs de différence et les mettront à jour en place, les fichiers supprimés nettoieront automatiquement les fichiers locaux et les répertoires vides, et ne modifieront que les autorisations ou horodateront directement les blocs de données zéro) :

- **`sync0 sub`(Mise en miroir unidirectionnelle en temps réel + amorçage automatique)** : le temps réel local suit les ajouts, suppressions et modifications de l'extrémité de partage et sert automatiquement de source de départ P2P pour fournir une accélération simultanée pour d'autres nœuds d'extraction.
- **`sync0 group`(Synchronisation bidirectionnelle en temps réel multi-machines peer-to-peer)** : si des fichiers sont ajoutés, supprimés ou modifiés sur n'importe quelle machine du groupe, tous les autres nœuds en ligne seront mis à jour de manière synchrone en temps réel et serviront de graines les uns aux autres.

```bash
# Abonnez-vous à l'intégralité du groupe de partage : les ajouts, suppressions et modifications à distance sont synchronisés localement en temps réel et automatiquement amorcés
sous-ensemble de données sync0:// ./local_mirror -p <adresse du nœud>

# Abonnement à sélection multiple à la demande : synchronisez et amorcez uniquement les fichiers et sous-répertoires spécifiques spécifiés en temps réel
sous-ensemble de données sync0://models/llama.onnx,configs ./local_mirror -p <adresse du nœud>

# Synchronisation bidirectionnelle en temps réel de groupes de pairs multi-machines (prend également en charge les chemins à sélection multiple complets ou par virgules)
groupe sync0 team-docs:// ./docs -p <adresse du nœud>
```

### 4. Configuration des autorisations de groupe et de partage multipartage d'équipe (`.nt` configuration)

Le répertoire de configuration par défaut est `~/.config/sync0` (peut être commuté via `-c <directory>`), en utilisant le format minimaliste [NestedText](https://nestedtext.org) (`.nt`) :
- **Prend en charge la restriction de la lecture sur le réseau public (seuls les utilisateurs autorisés peuvent lire)** : tant que le groupe de partage est configuré avec une liste de membres `users:` (ou que la ligne de commande spécifie `--allow-r` / `--allow-rw` / `--allow-w` / `--auth`), le système **active automatiquement l'authentification privée (`auth: true`)** et interdit l'énumération ou la lecture des nœuds anonymes du réseau public, ainsi que le P2P. L'abonnement aux nœuds d'amorçage héritera également simultanément de la liste blanche, permettant uniquement aux utilisateurs autorisés de lire ; Les groupes de partage qui ne sont pas configurés avec `users` sont publics et en lecture seule par défaut (si vous souhaitez être publics et en lecture seule et ouverts aux téléchargements uniquement aux membres spécifiés, vous pouvez définir explicitement `auth: false`).
- **Autorisations précises de lecture-écriture et de livraison de sous-répertoire** : les membres sont en lecture seule par défaut (`r` n'a pas besoin d'être écrit explicitement) et prennent en charge `r` (lecture seule), `w` (écriture uniquement dans la boîte de dépôt, non lisible), `rw` (lecture-écriture) et `write_prefix` (limite le préfixe de sous-répertoire autorisé pour le téléchargement).

**Exemple de configuration de membre (`~/.config/sync0/users/`) :**

```yaml
# ~/.config/sync0/users/alice.nt (lecture seule par défaut, limite de vitesse 80 Mo)
id: 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c
rate: 80MB
```

```yaml
# ~/.config/sync0/users/bob.nt (autorisations de lecture et d'écriture, uniquement autorisées à télécharger vers le sous-répertoire spécifié)
id: 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f
perm: rw
write_prefix:
  - uploads/bob
  - shared/common
```

```yaml
# ~/.config/sync0/users/charlie.nt (autorisation de livraison en écriture uniquement, la lecture est interdite)
id: 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
perm: w
```

**Exemple de configuration de groupe de partage (`~/.config/sync0/config.nt`) :**

```yaml
users_dir: ./users

groups:
  # Utilisateurs non configurés : réseau public par défaut en lecture seule
  dataset:
    path: ./data
    rate: 50MB

  # Utilisateurs configurés : restreindre automatiquement la lecture sur le réseau public (seuls Alice et Bob peuvent lire, Charlie ne peut que livrer et télécharger)
  team-vault:
    path: ./vault
    users:
      - alice
      - bob
      - charlie
```

Démarrez le service (chargez le fichier de configuration ou utilisez directement les paramètres de ligne de commande pour restreindre la lecture du réseau public et autoriser les membres) :

```bash
# Chargez ~/.config/sync0/config.nt pour démarrer
sync0 share --region SHA

# Ou publiez un groupe de partage privé via une ligne de commande pure (spécifiez --allow-* pour restreindre automatiquement la lecture sur le réseau public et seuls les utilisateurs autorisés peuvent y accéder)
sync0 share team-vault:./vault \
  --allow-r 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c \
  --allow-rw 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f:uploads/bob,shared/common \
  --allow-w 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
```

### 5. Adresse du nœud, carte de visite et multilingue

```bash
# Afficher l'adresse du nœud local sans IP (la clé est automatiquement générée pour la première exécution)
sync0 id

# Générez une carte de visite de nœud auto-signée Ed25519 (pseudonyme, code de ville, e-mail, site Web, introduction)
sync0 profile -u alice --region SHA --email alice@example.com -b "Shanghai node"

# Il existe 75 langues intégrées, qui suivent la langue du système par défaut et peuvent être commutées via des variables d'environnement.
SYNC0_LANG=en sync0 --help
```

## Performances et fonctionnalités de base

### Mesure réelle du WAN intercontinental (Comparaison `rsync`)

Environnement de mesure réel : États-Unis `c1` → Europe `n1` (RTT 65 ms), l'ensemble de données est le noyau Linux `linux-7.3.3` :

#### 1) Mode à la carte vs `rsync`

| Ensemble de données | taille | `sync0` Point unique<br>(Mbps) | `rsync`<br>(Mbps) | Accélération (`sync0` / `rsync`) |
| :--- | ---: | ---: | ---: | ---: |
| `linux-7.3.3.tar.xz`(non compressible) | 155 MB | **121.07** | 107.89 | **1,12×** |
| `linux-7.3.3.tar`(compressible) | 1.6 GB | **442.07** | 506.62 | **0,87×** |
| `linux-7.3.3.tar`(incrément de modification locale) | 1.6 GB | **8,123.87** | 1,169.49 | **6,95×** |
| `linux-7.3.3/` (95 979 petits fichiers) | 1.5 GB | **565.49** | 235.27 | **2,40×** |
| **moyenne géométrique** | — | **704.17** | 350.19 | **2,01×** |

#### 2) Mode double source P2P vs `rsync`

| Ensemble de données | taille | `sync0` Double source P2P<br>(Mbps) | `rsync`<br>(Mbps) | Accélération (`sync0` / `rsync`) |
| :--- | ---: | ---: | ---: | ---: |
| `linux-7.3.3.tar.xz`(non compressible) | 155 MB | **466.85** | 107.89 | **4,33×** |
| `linux-7.3.3.tar`(compressible) | 1.6 GB | **992.17** | 506.62 | **1,96×** |
| `linux-7.3.3.tar`(incrément de modification locale) | 1.6 GB | **5,666.84** | 1,169.49 | **4,85×** |
| `linux-7.3.3/` (95 979 petits fichiers) | 1.5 GB | **703.30** | 235.27 | **2,99×** |
| **moyenne géométrique** | — | **1,165.63** | 350.19 | **3,33×** |

> Remarque : En mode double source P2P, les nœuds européens `s1` collaborent pour l'amorçage, et le RTT du réseau fédérateur intercontinental est de 65 ms.

### Caractéristiques principales du produit

- **Perforation automatique d'adressage de clé publique** : Pas besoin d'un serveur central ni d'une adresse de réseau public, perforation automatique de trous NAT basée sur l'adresse de clé publique du réseau distribué.
- **Synchronisation en temps réel au niveau de la milliseconde et connaissance des métadonnées** : basé sur la surveillance native des événements de fichiers (`FSEvents` / `inotify` / `ReadDirectoryChangesW`), la diffusion en temps réel des ajouts, suppressions de fichiers, autorisations POSIX (`mode`) et le temps de modification au niveau de la milliseconde (`mtime`) ; zéro transmission de bloc de données lorsque seules les autorisations ou l’heure sont modifiées.
- **Téléchargement et téléchargement simultanés de style BT et planification d'affinité régionale** : le nœud de téléchargement n'a pas besoin d'attendre que le montant total soit téléchargé. Tous les intervalles de bloc (`Op::Have`) qui ont réussi la vérification du hachage de 128 bits sont immédiatement verrouillés dans le pool mémoire/temporaire pour que d'autres nœuds puissent les extraire simultanément. Avec l'échange dynamique de nœuds PEX et la demi-préemption lente des nœuds (Work-Stealing), le débit du cluster est maximisé.
- **Immunité contre la falsification des nœuds malveillants et l'empoisonnement des clusters** : liste de sources faisant autorité `root_hash` Verrouillage auto-cohérent + accélération matérielle obligatoire `gxhash128` vérification digest avant que chaque bloc ne soit placé sur le disque et amorcé en externe ; si un bloc falsifié est trouvé, le disjoncteur au niveau de la milliseconde mettra hors tension le nœud malveillant et récupérera automatiquement à partir de la source honnête.
- **Incrément in situ FastCDC et agrégation de petits fichiers** : seuls les blocs différentiels sont transférés pour la modification partielle des fichiers volumineux et placés sur le disque ; Le conditionnement massif de la mémoire de petits fichiers et la compression différentielle des préfixes éliminent la surcharge aller-retour fichier par fichier.
- **Auto-réparation après une panne de courant et reprise après redémarrage** : le format hybride RDB + AOF de type Redis (`.s0_subs.bin`) est utilisé pour conserver l'état d'amorçage et la liste des fichiers, les modifications quotidiennes sont ajoutées en microsecondes et sont automatiquement réécrites et organisées régulièrement. Une fois le serveur redémarré, les modifications hors ligne sont automatiquement transmises au nœud d'amorçage.

## Architecture technique et documentation

- **Protocole de découplage double plan** :
  - **Contrôle et surface de modification en temps réel (`iroh` QUIC / TCP, prise de contact du premier octet `PROTO_VERSION` version) ** : responsable de la vérification de la carte de visite Ed25519, de la vérification des autorisations, de la découverte des graines Swarm et de la prise en charge du traitement par lots de flux de modifications en masse au niveau `u64::MAX` et du `mzstd` compressé `SubPush` push en temps réel (`FsOp::Upsert` / `FsOp::Remove` / `FsOp::Meta` Trois primitives).
  - **Surface de distribution de données (UDP natif + `compio` E/S asynchrones) ** : en-tête compact de 20 B, retour périodique TACK de 18 B, contrôle de congestion hybride BBRv3/Copa, récupération de perte de paquets RACK-TLP, contournement de compression Zstd adaptatif à l'entropie Shannon de premier bloc et `gxhash128` vérification de bout en bout du bloc.
- **Apprendre encore plus**:
  - [Rapport de mesure du rendement et script de reproduction](https://github.com/webc-site/sync0/blob/main/doc/zh/benchmark.md)
  - [Conception d'architecture système](https://github.com/webc-site/sync0/blob/main/doc/zh/design_v4.md)
  - [Protocole de fil binaire et spécification de persistance](https://github.com/webc-site/sync0/blob/main/doc/zh/proto.md)
  - [Authentification d'identité, contrôle d'autorité et vérification d'intégrité](https://github.com/webc-site/sync0/blob/main/doc/zh/crypto.md)