# sync0

`sync0` est un outil de ligne de commande permettant de transférer des fichiers sur des réseaux, de synchroniser progressivement des répertoires et de distribuer simultanément des données à plusieurs machines.

## Intention de conception originale

Dans les salles multi-machines, les pays ou les environnements présentant des retards de réseau et des pertes de paquets, des outils tels que `rsync` et `scp` sont basés sur une confirmation de séquence de connexion unique, ce qui limite l'utilisation de la bande passante ; Lorsque plusieurs machines extraient des données du même serveur en même temps, la carte réseau du serveur source peut facilement devenir un goulot d'étranglement.

Le but de `sync0` est de résoudre deux problèmes :

1. Laissez la synchronisation des fichiers et des répertoires entre les deux machines exécuter toute la bande passante physique et transférez la partie modifiée uniquement lorsque le fichier est partiellement modifié.
2. Plusieurs machines peuvent transférer des blocs de données entre elles lors de la synchronisation du même répertoire, réduisant ainsi la charge sur le serveur source et raccourcissant le temps global de synchronisation.

## Comparaison de vitesse avec rsync

Environnement de test : nœud européen $\leftrightarrow$ nœud nord-américain, délai aller-retour de 86 ms, mesurant le temps complet, y compris l'analyse des répertoires, la comparaison des déduplications, la transmission réseau et la vérification de l'emplacement des disques :

| Scénario de transmission | Taille des données | `sync0` | `rsync` | multiple |
| :--- | :---: | :---: | :---: | :---: |
| 10 000 petits fichiers | 77.6 MB | 0.90 s (688 Mbps) | 4.86 s (128 Mbps) | 5.4x |
| Packages de code et d'archives (`.tar`) | 512 MB | 1.94 s (2111 Mbps) | 9.36 s (438 Mbps) | 4.8x |
| Fichier vidéo (`.mp4`) | 512 MB | 7.36 s (556 Mbps) | 12.11 s (338 Mbps) | 1.6x |
| Synchronisation incrémentale (modifier 1 % du contenu) | 512 MB | 0.12 s | 1.85 s | 15.4x |

## Méthodes d'amélioration des performances

- **Transmission simultanée UDP** : envoyez des blocs de données via UDP et retransmettez uniquement les fragments perdus pour éviter qu'une seule perte de paquet ne bloque les transferts de fichiers ultérieurs.
- **Segmentation et déduplication de contenu** : divisez le fichier en blocs de données en fonction du contenu et calculez la valeur de contrôle. Le même contenu déjà présent à la réception est réutilisé directement localement sans duplication du réseau.
- **Ajuster automatiquement la compression en fonction de la vitesse du réseau** : activez la compression pour le contenu compressible tel que le texte et le code ; ignore automatiquement la compression pour les vidéos, les images, les packages compressés ou sous un réseau local haut débit afin de réduire la consommation du processeur.
- **Extraction multi-nœuds à proximité** : lorsque plusieurs machines extraient le même répertoire, les blocs de données seront extraits simultanément des nœuds téléchargés dans la même ville ou avec une latence plus faible.

## Installer

### Méthode 1 : Installer via Cargo

```bash
cargo install sync0
```

### Méthode 2 : Télécharger les binaires précompilés

Accédez à [GitHub Releases](https://github.com/webc-site/sync0/releases) pour télécharger le package compressé `.tar.xz` pour le système correspondant (prend en charge Linux x86_64/ARM64, macOS Apple Silicon/Intel, Windows x64/ARM64, Android ARM64/x86_64, FreeBSD, NetBSD). Après la décompression, placez `sync0` dans le répertoire `PATH` :

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Guide de l'utilisateur

### 1. Transférer directement des fichiers ou des répertoires entre deux machines

Démarrez la surveillance sur la machine réceptrice et spécifiez le chemin de réception et de sauvegarde :

```bash
sync0 recv 0.0.0.0:9000 ./backup
```

Envoyez un seul fichier ou un répertoire entier depuis la machine émettrice :

```bash
sync0 send ./project 192.168.1.10:9000
```

Paramètres couramment utilisés :

```bash
# Supprimer les fichiers redondants qui existent du côté récepteur mais pas du côté expéditeur (synchronisation miroir)
sync0 send ./project 192.168.1.10:9000 --delete

# Limiter le débit d'envoi (unité : octets/seconde, par exemple 50 Mo/s)
sync0 send ./project 192.168.1.10:9000 --rate 50000000

# Mode silencieux (aucune barre de progression affichée)
sync0 send ./project 192.168.1.10:9000 -q
```

### 2. Publication de répertoires partagés et extraction à partir de plusieurs nœuds

Publiez un répertoire partagé (nommé `dataset`) sur le serveur d'origine :

```bash
sync0 share dataset ./data --region SHA
```

Extrayez le répertoire partagé sur d'autres machines (vous pouvez remplir plusieurs adresses de nœuds en même temps, séparées par des virgules pour les extraire simultanément) :

```bash
sync0 pull 10.0.0.1:9000,10.0.0.2:9000 dataset ./local_data
```

Extrayez uniquement un certain sous-répertoire du répertoire partagé :

```bash
sync0 pull 10.0.0.1:9000 dataset ./local_data --subpath 2026/logs
```

Renvoyez les modifications locales vers le répertoire partagé distant :

```bash
sync0 push 10.0.0.1:9000 dataset ./local_data
```

### 3. Abonnez-vous aux mises à jour automatiques du catalogue

Abonnez-vous au répertoire partagé du nœud source. Lorsqu'un fichier est ajouté ou modifié dans le répertoire source, il sera automatiquement synchronisé avec l'ordinateur local. Dans le même temps, la machine locale servira automatiquement de nœud pour que les autres machines puissent extraire :

```bash
sync0 sub 10.0.0.1:9000 dataset ./local_mirror
```

### 4. Synchronisation bidirectionnelle de groupes multi-machines

Plusieurs machines rejoignent le même nom de groupe et les modifications apportées à n'importe quel répertoire de nœuds seront automatiquement synchronisées avec les autres nœuds du groupe :

```bash
sync0 group team-docs ./docs 10.0.0.1:9000,10.0.0.2:9000
```

### 5. Identité du nœud et carte de visite personnelle

Chaque nœud prend en charge la définition du nom d'utilisateur, de la ville, de l'e-mail, du site Web et d'une signature personnalisée (maximum 256 caractères), et transmet l'authentification de signature par clé privée locale :

```bash
# Afficher l'ID de la clé publique locale (spécifiez le répertoire de configuration pour conserver la clé)
sync0 id -c ~/.config/sync0

# Générez et affichez des cartes de visite de nœuds avec des signatures personnalisées
sync0 profile -c ~/.config/sync0 \
  -u alice \
  --region SHA \
  --email alice@example.com \
  --url https://example.com \
  -b "Shanghai backup node"
```

`share`, `pull`, `sub`, `group` peuvent également être lancés avec ces paramètres de profil :

```bash
sync0 share dataset ./data -c ~/.config/sync0 -u alice --region SHA -b "Shanghai backup node"
```

### 6. Commutation multilingue

`sync0` Prend en charge 75 langues. Par défaut, l'aide et les invites de ligne de commande dans la langue correspondante sont automatiquement affichées en fonction de l'environnement linguistique du système d'exploitation ; si la langue du système ne figure pas dans la liste prise en charge, l'anglais est utilisé par défaut.

Pour spécifier la langue manuellement, définissez la variable d'environnement `SYNC0_LANG` :

```bash
SYNC0_LANG=en sync0 --help
SYNC0_LANG=ja sync0 --help
SYNC0_LANG=de sync0 --help
```