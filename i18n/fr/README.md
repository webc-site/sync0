# sync0

`sync0` est un outil de partage de fichiers peer-to-peer (P2P) et de synchronisation multi-machines qui concurrence Resilio Sync.

Chaque machine est à la fois un serveur et un client : n'importe quel nœud peut publier plusieurs groupes de partage de fichiers. Une fois que d'autres nœuds se sont joints, ils peuvent automatiquement obtenir les nœuds de départ disponibles à partir du serveur homologue et les extraire simultanément de plusieurs sources. Une fois la synchronisation terminée, des graines seront automatiquement créées pour davantage de nœuds afin d'accélérer les téléchargements.

## Points forts

- **Tout le monde est à la fois serveur et client** : comparable à Resilio Sync, pas de dépendance au serveur central. N'importe quel nœud peut créer plusieurs groupes de partage de fichiers et peut également rejoindre les groupes de partage d'autres personnes ; les nœuds échangent et obtiennent automatiquement les nœuds de départ disponibles pour obtenir une extraction simultanée et une synchronisation en temps réel de plusieurs machines.
- **Adresse de nœud sans IP** : par défaut, la clé de nœud et l'adresse `base64url` codée `iroh` sont automatiquement générées dans `~/.config/sync0` sans qu'il soit nécessaire de mémoriser ou d'exposer l'adresse IP publique.
- **Synchronisation à sélection multiple à la demande** : prend en charge la synchronisation de l'ensemble du groupe de partage et prend également en charge la synchronisation à la demande en sélectionnant plusieurs fichiers ou sous-répertoires en même temps.
- **Autorisations hiérarchiques des membres du groupe et contrôle du répertoire de téléchargement** : prend en charge les autorisations de lecture seule (`ro`), de téléchargement en lecture-écriture (`rw`) et de gestion (`admin`), et peut spécifier individuellement le préfixe de chemin que chaque membre est autorisé à télécharger (par défaut, le téléchargement vers le répertoire racine du partage est autorisé).
- **Synchronisation incrémentielle et à pleine vitesse sur les réseaux** : transmission simultanée de fichiers volumineux, agrégation de petits fichiers massifs en mémoire et blocs de données synchronisés uniquement lorsque les fichiers sont partiellement modifiés.

## Comparaison de vitesse avec rsync

### Mesure réelle à source unique du réseau transocéanique (Chine → États-Unis, délai 260 ms)

| Scénario de transmission | Taille des données | modèle | `sync0` | `rsync -az` | Augmenter le multiplicateur |
| :--- | :---: | :---: | :---: | :---: | :---: |
| 500 petits fichiers | 1.91 MB | Synchronisation complète | 18.84 Mbps | 5.32 Mbps | 3.5x |
| 500 petits fichiers | 1.91 MB | Synchronisation incrémentielle | 27.15 Mbps | 3.83 Mbps | 7.1x |
| Packages de code et d'archives (`.tar`) | 10.0 MB | Synchronisation complète | 139.70 Mbps | 24.97 Mbps | 5.6x |
| Packages de code et d'archives (`.tar`) | 10.0 MB | Synchronisation incrémentielle | 83.38 Mbps | 26.02 Mbps | 3.2x |
| Fichier vidéo (`.mp4`) | 15.0 MB | Synchronisation complète | 54.06 Mbps | 27.04 Mbps | 2.0x |
| Fichier vidéo (`.mp4`) | 15.0 MB | Synchronisation incrémentielle | 216.83 Mbps | 21.51 Mbps | 10.1x |

### Test d'extraction multi-nœuds P2P du réseau transocéanique (nœuds doubles Chine → États-Unis, délai 260 ms)

Comparaison mesurée de l'extraction simultanée par bandes de nœuds d'amorçage multi-sources :

| Scénario de transmission | Taille des données | Ensemencement à un seul nœud | Ensemencement simultané P2P à deux nœuds | Augmenter le multiplicateur |
| :--- | :---: | :---: | :---: | :---: |
| 500 petits fichiers | 1.97 MB | 5.28 Mbps | 7.87 Mbps | 1.5x |
| Packages de code et d'archives (`.tar`) | 30.0 MB | 56.35 Mbps | 83.17 Mbps | 1.5x |
| Fichier vidéo (`.mp4`) | 30.0 MB | 27.00 Mbps | 39.46 Mbps | 1.5x |

### Mesure réelle de la ligne dédiée transcontinentale à grande vitesse (Europe → Amérique du Nord, délai 86 ms)

| Scénario de transmission | Taille des données | `sync0` | `rsync` | Augmenter le multiplicateur |
| :--- | :---: | :---: | :---: | :---: |
| 10 000 petits fichiers | 77.6 MB | 688 Mbps | 128 Mbps | 5.4x |
| Packages de code et d'archives (`.tar`) | 512 MB | 2111 Mbps | 438 Mbps | 4.8x |
| Fichier vidéo (`.mp4`) | 512 MB | 556 Mbps | 338 Mbps | 1.6x |
| Synchronisation incrémentale (modifier 1 % du contenu) | 512 MB | 34133 Mbps | 2214 Mbps | 15.4x |

## Installer

### Méthode 1 : Installer via Cargo

```bash
cargo install sync0
```

### Méthode 2 : Télécharger les binaires précompilés

Accédez à [GitHub Releases](https://github.com/webc-site/sync0/releases) pour télécharger le package compressé `.tar.xz` pour le système correspondant (prend en charge Linux, macOS, Windows, Android, FreeBSD, NetBSD), décompressez-le et placez-le dans le répertoire `PATH` :

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Commencez rapidement

`sync0` utilise `~/.config/sync0` comme répertoire de configuration par défaut (si vous devez basculer vers d'autres répertoires, vous pouvez ajouter `-c <directory>`).

### 1. Afficher l'adresse du nœud local et définir la carte de visite

```bash
# Afficher l'adresse du nœud local (la clé est automatiquement générée dans ~/.config/sync0 pour la première fois)
sync0 id

# Générez une carte de visite de nœud personnel (prend en charge le nom d'utilisateur, la ville, l'e-mail, le site Web et la signature dans 256 mots)
sync0 profile -u alice --region SHA --email alice@example.com --url https://example.com -b "Shanghai node"
```

### 2. Publier plusieurs groupes de partage de fichiers et définir les autorisations des membres du groupe (`.nt` configuration)

`sync0` est configuré au format [NestedText](https://nestedtext.org) (`.nt`). Chaque utilisateur dispose d'un fichier de configuration `.nt` distinct. Vous pouvez spécifier le répertoire du profil utilisateur via `users_dir` dans `config.nt` (qui inclut automatiquement tous les fichiers `.nt` de ce répertoire). L'utilisateur et le préfixe de chemin autorisés à être téléchargés utilisent une liste non ordonnée (`-`). **Lorsque le préfixe du chemin de téléchargement n'est pas renseigné, le téléchargement vers le répertoire racine du partage est autorisé par défaut**.

1. Créez des fichiers de configuration `.nt` distincts pour chaque membre dans le répertoire `~/.config/sync0/users/` :

`~/.config/sync0/users/alice.nt` (membre en lecture seule, taux limité `80MB`, `rate` prend en charge les unités `GB`, `MB`, `KB`) :

```yaml
id: 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c
perm: ro
rate: 80MB
```

`~/.config/sync0/users/bob.nt` (membres de téléchargement restreint : uniquement autorisés à télécharger vers les répertoires `uploads/bob` et `shared/common`) :

```yaml
id: 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f
perm: rw
write_prefix:
  - uploads/bob
  - shared/common
```

`~/.config/sync0/users/charlie.nt` (Téléchargement de membre par défaut : lorsque `write_prefix` est omis, le téléchargement vers le répertoire racine du partage est autorisé par défaut) :

```yaml
id: 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
perm: rw
```

2. Configurez l'annuaire des utilisateurs et plusieurs groupes de partage de fichiers dans `~/.config/sync0/config.nt` :

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

Démarrer le service de partage :

```bash
# Démarrage direct (charge automatiquement toutes les configurations utilisateur .nt dans les répertoires ~/.config/sync0/config.nt et users_dir)
sync0 share --region SHA

# Ou publiez rapidement plusieurs groupes de partage et autorisez les membres du groupe directement via la ligne de commande
sync0 share dataset:./data team-vault:./vault \
  --auth \
  --allow-ro 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c \
  --allow-rw 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f:uploads/bob,shared/common \
  --allow-rw 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
```

### 3. Extrayez tout le contenu ou sélectionnez plusieurs fichiers et répertoires (`sync0 pull`)

Connectez-vous à n'importe quel nœud homologue pour obtenir automatiquement tous les nœuds de départ disponibles dans le groupe de partage et extrayez simultanément plusieurs nœuds de départ :

```bash
# Extrayez l'intégralité du groupe de partage d'ensemble de données vers le ./local_data local
sync0 pull dataset:// ./local_data -p <adresse du nœud>

# Synchronisation à sélection multiple : extrayez simultanément le fichier unique docs/readme.md et les sous-répertoires assets/icons, src/core
sync0 pull dataset://docs/readme.md,assets/icons,src/core ./local_data -p <adresse du nœud>
```

### 4. Télécharger un fichier ou un répertoire vers un groupe de partage (`sync0 push`)

Les membres disposant des autorisations de téléchargement (`rw` ou `admin`) peuvent télécharger des fichiers ou des répertoires locaux vers des groupes de partage distants :

```bash
# Membres disposant de l'autorisation de téléchargement dans le répertoire racine : télécharger directement vers le répertoire racine partagé
sync0 push team-vault:// ./new_release -p <adresse du nœud>

# Uniquement les membres autorisés à utiliser un préfixe spécifique (tel que uploads/bob) : télécharger vers le préfixe de chemin autorisé
sync0 push team-vault://uploads/bob/report.pdf ./report.pdf -p <adresse du nœud>
```

### 5. Abonnez-vous à la synchronisation et à l'amorçage automatiques (`sync0 sub`)

Après s'être abonné au groupe de partage d'un certain nœud, le serveur local est à la fois un client récepteur et un nouveau serveur d'amorçage ; lorsque le fichier du nœud source est mis à jour, il est automatiquement synchronisé et fournit des téléchargements fragmentés pour les autres nœuds :

```bash
# Abonnez-vous à l'intégralité du groupe de partage et démarrez automatiquement
sous-ensemble de données sync0:// ./local_mirror -p <adresse du nœud>

# Uniquement sélection multiple pour vous abonner aux fichiers et sous-répertoires spécifiés et les amorcer
sous-ensemble de données sync0://models/llama.onnx,configs ./local_mirror -p <adresse du nœud>
```

### 6. Synchronisation homologue de groupe multi-machines (`sync0 group`)

Plusieurs machines rejoignent le même groupe de partage, les unes les autres découvrent automatiquement les nœuds de départ disponibles et maintiennent la synchronisation bidirectionnelle (prend en charge la synchronisation complète ou la sélection multiple de fichiers et de répertoires) :

```bash
# Rejoignez pleinement le groupe et synchronisation bidirectionnelle
groupe sync0 team-docs:// ./docs -p <adresse du nœud>

# Uniquement le répertoire wal et le fichier meta.json dans le groupe de synchronisation à sélection multiple
groupe sync0 team-docs://wal,meta.json ./docs -p <adresse du nœud>
```

### 7. Transmission directe point à point entre deux machines (`send` / `recv`)

Il n'est pas nécessaire de préconfigurer un groupe et vous pouvez transférer temporairement des fichiers ou des répertoires directement entre deux machines :

```bash
# Surveillance du récepteur
sync0 recv 0.0.0.0:9000 ./backup

# L'expéditeur transfère directement des fichiers ou des répertoires (optionnel --delete miroir pour nettoyer les fichiers en excès, --rate vitesse limite)
sync0 send ./project 192.168.1.10:9000
```

### 8. Commutation multilingue

`sync0` prend en charge 75 langues et change automatiquement par défaut en fonction de l'environnement linguistique du système ; il peut également être spécifié manuellement via `SYNC0_LANG` :

```bash
SYNC0_LANG=en sync0 --help
SYNC0_LANG=ja sync0 --help
SYNC0_LANG=de sync0 --help
```