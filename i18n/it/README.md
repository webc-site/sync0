# sync0

`sync0` è uno strumento di condivisione file peer-to-peer (P2P) e di sincronizzazione di più macchine che compete con Resilio Sync.

Ogni macchina è sia un server che un client: qualsiasi nodo può pubblicare più gruppi di condivisione file. Dopo che altri nodi si sono uniti, possono ottenere automaticamente i nodi seed disponibili dal server peer ed estrarli contemporaneamente da più fonti. Una volta completata la sincronizzazione, verranno creati automaticamente i seed per più nodi per accelerare i download.

## Punti salienti principali

- **Ognuno è sia un server che un client**: paragonabile a Resilio Sync, nessuna dipendenza dal server centrale. Qualsiasi nodo può creare più gruppi di condivisione file e può anche unirsi ai gruppi di condivisione di altre persone; i nodi si scambiano e ottengono automaticamente i nodi seed disponibili per ottenere l'estrazione simultanea e la sincronizzazione in tempo reale di più macchine.
- **Indirizzo del nodo senza IP**: per impostazione predefinita, la chiave del nodo e l'indirizzo `iroh` codificato `base64url` vengono generati automaticamente in `~/.config/sync0` senza la necessità di ricordare o esporre l'IP pubblico.
- **Sincronizzazione a selezione multipla su richiesta**: supporta la sincronizzazione dell'intero gruppo di condivisione e supporta anche la sincronizzazione su richiesta selezionando più file o sottodirectory contemporaneamente.
- **Autorizzazioni gerarchiche dei membri del gruppo e controllo della directory di caricamento**: supporta autorizzazioni di sola lettura (`ro`), caricamento di lettura-scrittura (`rw`) e gestione (`admin`) e può specificare individualmente il prefisso del percorso che ciascun membro può caricare (per impostazione predefinita, è consentito il caricamento nella directory principale della condivisione).
- **Sincronizzazione incrementale e a piena velocità tra reti**: trasmissione simultanea di file di grandi dimensioni, aggregazione di file di piccole dimensioni in memoria e blocchi di dati sincronizzati solo quando i file vengono parzialmente modificati.

## Confronto di velocità con rsync

### Misurazione effettiva da un'unica fonte della rete transoceanica (Cina → Stati Uniti, ritardo 260 ms)

| Scenario di trasmissione | Dimensione dei dati | modello | `sync0` | `rsync -az` | Aumenta il moltiplicatore |
| :--- | :---: | :---: | :---: | :---: | :---: |
| 500 piccoli file | 1.91 MB | Sincronizzazione completa | 18.84 Mbps | 5.32 Mbps | 3.5x |
| 500 piccoli file | 1.91 MB | Sincronizzazione incrementale | 27.15 Mbps | 3.83 Mbps | 7.1x |
| Pacchetti di codice e archivio (`.tar`) | 10.0 MB | Sincronizzazione completa | 139.70 Mbps | 24.97 Mbps | 5.6x |
| Pacchetti di codice e archivio (`.tar`) | 10.0 MB | Sincronizzazione incrementale | 83.38 Mbps | 26.02 Mbps | 3.2x |
| File video (`.mp4`) | 15.0 MB | Sincronizzazione completa | 54.06 Mbps | 27.04 Mbps | 2.0x |
| File video (`.mp4`) | 15.0 MB | Sincronizzazione incrementale | 216.83 Mbps | 21.51 Mbps | 10.1x |

### Test pull multi-nodo P2P della rete transoceanica (Doppio nodo Cina → Stati Uniti, ritardo 260 ms)

Confronto misurato del pull simultaneo striped di nodi seed multi-sorgente:

| Scenario di trasmissione | Dimensione dei dati | Seeding a nodo singolo | Seeding simultaneo P2P a due nodi | Aumenta il moltiplicatore |
| :--- | :---: | :---: | :---: | :---: |
| 500 piccoli file | 1.97 MB | 5.28 Mbps | 7.87 Mbps | 1.5x |
| Pacchetti di codice e archivio (`.tar`) | 30.0 MB | 56.35 Mbps | 83.17 Mbps | 1.5x |
| File video (`.mp4`) | 30.0 MB | 27.00 Mbps | 39.46 Mbps | 1.5x |

### Misura effettiva della linea dedicata transcontinentale ad alta velocità (Europa → Nord America, ritardo 86 ms)

| Scenario di trasmissione | Dimensione dei dati | `sync0` | `rsync` | Aumenta il moltiplicatore |
| :--- | :---: | :---: | :---: | :---: |
| 10.000 piccoli file | 77.6 MB | 688 Mbps | 128 Mbps | 5.4x |
| Pacchetti di codice e archivio (`.tar`) | 512 MB | 2111 Mbps | 438 Mbps | 4.8x |
| File video (`.mp4`) | 512 MB | 556 Mbps | 338 Mbps | 1.6x |
| Sincronizzazione incrementale (modifica 1% del contenuto) | 512 MB | 34133 Mbps | 2214 Mbps | 15.4x |

## Installare

### Metodo 1: installazione tramite Cargo

```bash
cargo install sync0
```

### Metodo 2: scaricare i file binari precompilati

Vai a [GitHub Releases](https://github.com/webc-site/sync0/releases) per scaricare il pacchetto compresso `.tar.xz` per il sistema corrispondente (supporta Linux, macOS, Windows, Android, FreeBSD, NetBSD), decomprimilo e inseriscilo nella directory `PATH`:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Inizia rapidamente

`sync0` utilizza `~/.config/sync0` come directory di configurazione per impostazione predefinita (se hai bisogno di passare ad altre directory, puoi aggiungere `-c <directory>`).

### 1. Visualizza l'indirizzo del nodo locale e l'impostazione del biglietto da visita

```bash
# Visualizza l'indirizzo del nodo locale (la chiave viene generata automaticamente in ~/.config/sync0 per la prima volta)
sync0 id

# Genera un biglietto da visita del nodo personale (supporta nome utente, città, e-mail, sito Web e firma entro 256 parole)
sync0 profile -u alice --region SHA --email alice@example.com --url https://example.com -b "Shanghai node"
```

### 2. Pubblica più gruppi di condivisione file e imposta le autorizzazioni dei membri del gruppo (`.nt` Configurazione)

`sync0` è configurato nel formato [Testo annidato](https://nestedtext.org) (`.nt`). Ogni utente ha un file di configurazione `.nt` separato. È possibile specificare la directory del profilo utente tramite `users_dir` in `config.nt` (che include automaticamente tutti i file `.nt` in questa directory). Sia l'utente che il prefisso del percorso che è consentito caricare utilizzano un elenco non ordinato (`-`). **Quando il prefisso del percorso di caricamento non viene compilato, il caricamento nella directory principale della condivisione è consentito per impostazione predefinita**.

1. Crea file di configurazione `.nt` separati per ciascun membro nella directory `~/.config/sync0/users/`:

`~/.config/sync0/users/alice.nt` (membro di sola lettura, tariffa limitata `80MB`, `rate` supporta `GB`, `MB`, `KB` unità):

```yaml
id: 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c
perm: ro
rate: 80MB
```

`~/.config/sync0/users/bob.nt` (membri che caricano con limitazioni: possono caricare solo nelle directory `uploads/bob` e `shared/common`):

```yaml
id: 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f
perm: rw
write_prefix:
  - uploads/bob
  - shared/common
```

`~/.config/sync0/users/charlie.nt` (Caricamento membro predefinito: quando `write_prefix` viene omesso, il caricamento nella directory principale della condivisione è consentito per impostazione predefinita):

```yaml
id: 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
perm: rw
```

2. Configura la directory utente e più gruppi di condivisione file in `~/.config/sync0/config.nt`:

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

Inizia a condividere il servizio:

```bash
# Avvio diretto (carica automaticamente tutte le configurazioni utente .nt nelle directory ~/.config/sync0/config.nt e users_dir)
sync0 share --region SHA

# Oppure pubblica rapidamente più gruppi di condivisione e autorizza i membri del gruppo direttamente tramite la riga di comando
sync0 share dataset:./data team-vault:./vault \
  --auth \
  --allow-ro 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c \
  --allow-rw 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f:uploads/bob,shared/common \
  --allow-rw 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
```

### 3. Estrai tutto il contenuto o seleziona più file e directory (`sync0 pull`)

Connettiti a qualsiasi nodo peer per ottenere automaticamente tutti i nodi seed disponibili nel gruppo di condivisione ed eseguire il pull da più nodi seed contemporaneamente:

```bash
# Estrarre l'intero gruppo di condivisione del set di dati nel file ./local_data locale
sync0 pull dataset:// ./local_data -p <indirizzo nodo>

# Sincronizzazione a selezione multipla: estrae contemporaneamente il singolo file docs/readme.md e le sottodirectory asset/icons, src/core
sync0 pull dataset://docs/readme.md,assets/icons,src/core ./local_data -p <indirizzo nodo>
```

### 4. Carica un file o una directory in un gruppo di condivisione (`sync0 push`)

I membri con autorizzazioni di caricamento (`rw` o `admin`) possono caricare file o directory locali su gruppi di condivisione remoti:

```bash
# Membri con autorizzazione al caricamento della directory root: caricano direttamente nella directory root condivisa
sync0 push team-vault:// ./new_release -p <indirizzo nodo>

# Solo i membri autorizzati a un prefisso specifico (come uploads/bob): carica nel prefisso del percorso consentito
sync0 push team-vault://uploads/bob/report.pdf ./report.pdf -p <indirizzo nodo>
```

### 5. Iscriviti alla sincronizzazione e al seeding automatici (`sync0 sub`)

Dopo la sottoscrizione al gruppo di condivisione di un determinato nodo, il server locale è sia un client ricevente che un nuovo server di seeding; quando il file del nodo di origine viene aggiornato, viene automaticamente sincronizzato e fornisce download in blocchi per altri nodi:

```bash
# Iscriviti all'intero gruppo di condivisione e semina automaticamente
sync0 sub dataset:// ./local_mirror -p <indirizzo nodo>

# Effettua solo la selezione multipla per iscriverti a file e sottodirectory specificati ed eseguirne il seeding
sync0 sub dataset://models/llama.onnx,configs ./local_mirror -p <indirizzo nodo>
```

### 6. Sincronizzazione peer di gruppo su più macchine (`sync0 group`)

Più macchine si uniscono allo stesso gruppo di condivisione, le altre rilevano automaticamente i nodi seed disponibili e mantengono la sincronizzazione bidirezionale (supporta la sincronizzazione completa o la selezione multipla di file e directory):

```bash
# Partecipa completamente al gruppo e sincronizzazione bidirezionale
sync0 gruppo team-docs:// ./docs -p <indirizzo nodo>

# Solo la directory wal e il file meta.json nel gruppo di sincronizzazione a selezione multipla
gruppo sync0 team-docs://wal,meta.json ./docs -p <indirizzo nodo>
```

### 7. Trasmissione diretta punto a punto tra due macchine (`send` / `recv`)

Non è necessario preconfigurare un gruppo ed è possibile trasferire temporaneamente file o directory direttamente tra due macchine:

```bash
# Monitoraggio del ricevitore
sync0 recv 0.0.0.0:9000 ./backup

# L'estremità di invio trasferisce direttamente file o directory (opzionale --delete mirror per eliminare i file in eccesso, --rate speed limit)
sync0 send ./project 192.168.1.10:9000
```

### 8. Commutazione multilingue

`sync0` supporta 75 lingue e cambia automaticamente in base all'ambiente linguistico del sistema per impostazione predefinita; può anche essere specificato manualmente tramite `SYNC0_LANG`:

```bash
SYNC0_LANG=en sync0 --help
SYNC0_LANG=ja sync0 --help
SYNC0_LANG=de sync0 --help
```