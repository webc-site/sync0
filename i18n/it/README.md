# sync0

`sync0` è uno strumento di condivisione file P2P decentralizzato, sincronizzazione in tempo reale e distribuzione multi-sorgente utilizzato per sostituire `rsync`: la velocità misurata della WAN transcontinentale è fino a **5,89×** di `rsync` (velocità media geometrica a punto singolo **2,67×**, velocità media geometrica P2P a doppia sorgente **3,16×**) e accelerazione P2P del collegamento transoceanico ad alta latenza è fino a **7,68×** (velocità media geometrica completa con collegamento completo a 24 scenari **2,56×**).

Ogni macchina è sia un server che un client: non è necessario un server centrale o un indirizzo di rete pubblico, la perforazione automatica si basa sull'indirizzo della chiave pubblica della rete distribuita e supporta la trasmissione diretta tra due macchine, hosting di autorizzazioni di gruppo multi-condivisione, selezione multipla di sottodirectory su richiesta, sincronizzazione incrementale in tempo reale a livello di millisecondo e accelerazione simultanea multi-sorgente P2P.

## Installare

### Metodo 1: installazione del carico

```bash
cargo install sync0
```

### Metodo 2: scaricare i file binari precompilati

Vai su [GitHub Releases](https://github.com/webc-site/sync0/releases) per scaricare il `.tar.xz` pacchetto compresso corrispondente al sistema (supporta Linux, macOS, Windows, Android, iOS, FreeBSD), decomprimilo e inseriscilo in `PATH`:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Inizia rapidamente

### 1. Trasmissione diretta senza configurazione tra due macchine (`send` / `recv`)

Non è necessario configurare un gruppo di condivisione, trasferire direttamente file o directory tra due macchine ad alta velocità:

```bash
# L'estremità ricevente ascolta sulla porta e scrive su ./backup
sync0 recv 0.0.0.0:9000 ./backup

# Trasmissione diretta dal mittente (opzionale --delete mirror per eliminare i file in eccesso, --rate limite di velocità di 50MB)
sync0 send ./project 192.168.1.10:9000
```

### 2. Condivisione dei post e pull/upload su richiesta (`share` / `pull` / `push`)

```bash
# Server: pubblica il gruppo di condivisione con una riga di comando (visualizza l'indirizzo del nodo iroh senza IP dopo l'avvio)
sync0 share dataset:./data

# Client: trascina l'intero gruppo di condivisione del set di dati nel file ./local_data locale
sync0 pull dataset:// ./local_data -p <indirizzo nodo>

# Client: effettua selezioni multiple su richiesta (sincronizza solo i singoli file e le sottodirectory specificati)
sync0 pull dataset://docs/readme.md,assets/icons,src/core ./local_data -p <indirizzo nodo>

# Client: carica file o directory su gruppi di condivisione autorizzati
sync0 push dataset://releases/v1 ./build_output -p <indirizzo nodo>
```

### 3. Seeding dell'abbonamento in tempo reale e sincronizzazione bidirezionale multi-macchina (`sub` / `group`)

Dopo aver stabilito una connessione lunga, se si verifica qualsiasi file **aggiunta, modifica, eliminazione, modifica delle autorizzazioni o dell'ora di modifica** sull'estremità remota, la directory locale verrà aggiornata in modo sincrono in millisecondi in tempo reale (i file di grandi dimensioni nuovi/modificati estrarranno solo i blocchi di differenza e li aggiorneranno al loro posto, l'eliminazione dei file pulirà automaticamente i file locali e le directory vuote e modificherà solo le autorizzazioni o indicherà direttamente l'orario con zero blocchi di dati):

- **`sync0 sub`(Mirroring in tempo reale unidirezionale + seeding automatico)**: il tempo reale locale segue le aggiunte, eliminazioni e modifiche dell'estremità di condivisione e funge automaticamente da sorgente seed P2P per fornire accelerazione simultanea per altri nodi di pull.
- **`sync0 group`(Sincronizzazione bidirezionale in tempo reale peer-to-peer su più macchine)**: se i file vengono aggiunti, eliminati o modificati su qualsiasi macchina del gruppo, tutti gli altri nodi online verranno aggiornati in modo sincrono in tempo reale e fungeranno da seed l'uno per l'altro.

```bash
# Iscriviti all'intero gruppo di condivisione: aggiunte, eliminazioni e modifiche remote vengono sincronizzate localmente in tempo reale e seminate automaticamente
sync0 sub dataset:// ./local_mirror -p <indirizzo nodo>

# Abbonamento a selezione multipla su richiesta: sincronizza e semina solo singoli file e sottodirectory specificati in tempo reale
sync0 sub dataset://models/llama.onnx,configs ./local_mirror -p <indirizzo nodo>

# Sincronizzazione in tempo reale bidirezionale di gruppi peer multi-macchina (supporta anche percorsi di selezione multipla completi o con virgola)
sync0 gruppo team-docs:// ./docs -p <indirizzo nodo>
```

### 4. Configurazione del gruppo di condivisione multipla e delle autorizzazioni dei membri del team (configurazione `.nt`)

La directory di configurazione predefinita è `~/.config/sync0` (può essere cambiata tramite `-c <directory>`), utilizzando il formato minimalista [NestedText](https://nestedtext.org) (`.nt`):
- **Supporta la limitazione della lettura sulla rete pubblica (solo gli utenti autorizzati possono leggere)**: finché il gruppo di condivisione è configurato con un `users:` elenco di membri (o la riga di comando specifica `--allow-r` / `--allow-rw` / `--allow-w` / `--auth`), il sistema **attiva automaticamente l'autenticazione privata (`auth: true`)** e proibisce l'enumerazione o la lettura dei nodi anonimi della rete pubblica e P2P La sottoscrizione ai nodi di seeding erediterà simultaneamente anche la whitelist, consentendo la lettura solo agli utenti autorizzati; i gruppi di condivisione che non sono configurati con `users` sono pubblici e di sola lettura per impostazione predefinita (se vuoi essere pubblici, di sola lettura e aperti ai caricamenti solo per i membri specificati, puoi impostare esplicitamente `auth: false`).
- **Autorizzazioni dettagliate di lettura-scrittura e recapito sottodirectory**: i membri sono di sola lettura per impostazione predefinita (non è necessario scrivere esplicitamente `r`) e supportano `r` (sola lettura), `w` (solo scrittura nella casella di riepilogo, non leggibile), `rw` (lettura-scrittura) e `write_prefix` (limita il prefisso della sottodirectory consentito per il caricamento).

**Esempio di configurazione membro (`~/.config/sync0/users/`):**

```yaml
# ~/.config/sync0/users/alice.nt (sola lettura predefinita, limite di velocità 80 MB)
id: 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c
rate: 80MB
```

```yaml
# ~/.config/sync0/users/bob.nt (permessi di lettura e scrittura, consentito solo il caricamento nella sottodirectory specificata)
id: 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f
perm: rw
write_prefix:
  - uploads/bob
  - shared/common
```

```yaml
# ~/.config/sync0/users/charlie.nt (solo permesso di consegna in scrittura, la lettura è vietata)
id: 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
perm: w
```

**Esempio di configurazione del gruppo di condivisione (`~/.config/sync0/config.nt`):**

```yaml
users_dir: ./users

groups:
  # Utenti non configurati: rete pubblica predefinita di sola lettura
  dataset:
    path: ./data
    rate: 50MB

  # Utenti configurati: limita automaticamente la lettura sulla rete pubblica (solo Alice e Bob possono leggere, Charlie può solo consegnare e caricare)
  team-vault:
    path: ./vault
    users:
      - alice
      - bob
      - charlie
```

Avvia il servizio (carica il file di configurazione o utilizza direttamente i parametri della riga di comando per limitare la lettura della rete pubblica e autorizzare i membri):

```bash
# Carica ~/.config/sync0/config.nt per iniziare
sync0 share --region SHA

# Oppure pubblica un gruppo di condivisione privato tramite la pura riga di comando (specifica --allow-* per limitare automaticamente la lettura della rete pubblica e solo gli utenti autorizzati possono accedervi)
sync0 share team-vault:./vault \
  --allow-r 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c \
  --allow-rw 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f:uploads/bob,shared/common \
  --allow-w 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
```

### 5. Indirizzo del nodo, biglietto da visita e multilingua

```bash
# Visualizza l'indirizzo del nodo locale senza IP (la chiave viene generata automaticamente per la prima esecuzione)
sync0 id

# Genera biglietto da visita del nodo autofirmato Ed25519 (soprannome, codice città, email, sito web, presentazione)
sync0 profile -u alice --region SHA --email alice@example.com -b "Shanghai node"

# Sono disponibili 75 lingue integrate, che seguono la lingua del sistema per impostazione predefinita e possono essere cambiate tramite variabili di ambiente.
SYNC0_LANG=en sync0 --help
```

## Prestazioni e funzionalità principali

### Misurazione effettiva della WAN intercontinentale (confronto `rsync`)

Ambiente di misurazione effettivo: Stati Uniti `c1` → Europa `n1` (RTT 65 ms), il set di dati è il kernel Linux `linux-7.3.3`:

#### 1) Modalità à la carte vs `rsync`

| Insieme di dati | misurare | `sync0` Punto singolo<br>(Mbps) | `rsync`<br>(Mbps) | Accelerazione (`sync0` / `rsync`) |
| :--- | ---: | ---: | ---: | ---: |
| `linux-7.3.3.tar.xz`(non comprimibile) | 155 MB | **236.48** | 108.74 | **2.17×** |
| `linux-7.3.3.tar`(comprimibile) | 1.6 GB | **843.83** | 385.06 | **2.19×** |
| `linux-7.3.3.tar`(incremento modifica locale) | 1.6 GB | **8,014.15** | 1,361.72 | **5,89×** |
| `linux-7.3.3/` (95.979 file piccoli) | 1.5 GB | **627.58** | 347.80 | **1,80×** |
| **media geometrica** | — | **1,000.91** | 375.26 | **2,67×** |

#### 2) Modalità P2P a doppia sorgente rispetto a `rsync`

| Insieme di dati | misurare | `sync0` Doppia sorgente P2P<br>(Mbps) | `rsync`<br>(Mbps) | Accelerazione (`sync0` / `rsync`) |
| :--- | ---: | ---: | ---: | ---: |
| `linux-7.3.3.tar.xz`(non comprimibile) | 155 MB | **330.09** | 108.74 | **3.04×** |
| `linux-7.3.3.tar`(comprimibile) | 1.6 GB | **1,551.86** | 385.06 | **4.03×** |
| `linux-7.3.3.tar`(incremento modifica locale) | 1.6 GB | **5,474.17** | 1,361.72 | **4.02×** |
| `linux-7.3.3/` (95.979 file piccoli) | 1.5 GB | **706.56** | 347.80 | **2.03×** |
| **media geometrica** | — | **1,186.42** | 375.26 | **3.16×** |

> Nota: nella modalità P2P a doppia sorgente, i nodi europei `s1` collaborano per il seeding e l'RTT del backbone transcontinentale è di 65 ms.

### Caratteristiche principali del prodotto

- **Perforazione automatica dell'indirizzamento a chiave pubblica**: non è necessario un server centrale e un indirizzo di rete pubblica, perforazione NAT automatica basata sull'indirizzo della chiave pubblica della rete distribuita.
- **Sincronizzazione in tempo reale a livello di millisecondi e riconoscimento dei metadati**: basato sul monitoraggio degli eventi di file nativi (`FSEvents` / `inotify` / `ReadDirectoryChangesW`), push in tempo reale di aggiunte, eliminazioni di file, autorizzazioni POSIX (`mode`) e tempo di modifica a livello di millisecondi (`mtime`); zero trasmissione di blocchi dati quando vengono modificati solo i permessi o l'ora.
- **Download e caricamento simultanei in stile BT e pianificazione di affinità regionale**: il nodo di download non deve attendere che venga scaricato l'intero importo. Tutti gli intervalli di blocco (`Op::Have`) che hanno superato il controllo dell'hash a 128 bit vengono immediatamente bloccati nel pool di memoria/temporaneo affinché altri nodi possano eseguire il pull contemporaneamente. Insieme allo scambio dinamico dei nodi PEX e alla mezza prelazione dei nodi lenti (Work-Stealing), il throughput del cluster viene massimizzato.
- **Immunità anti-manomissione dei nodi dannosi e avvelenamento dei cluster**: elenco di fonti autorevoli `root_hash` Blocco autoconsistente + accelerazione hardware obbligatoria `gxhash128` verifica del digest prima che ogni blocco venga posizionato sul disco e sottoposto a seeding esterno; se viene trovato un blocco manomesso, l'interruttore di livello millisecondo oscurerà il nodo dannoso e si ripristinerà automaticamente dalla fonte onesta.
- **Incremento in situ FastCDC e aggregazione di piccoli file**: solo i blocchi differenziali vengono trasferiti per la modifica parziale di file di grandi dimensioni e inseriti su disco; il massiccio confezionamento della memoria per file di piccole dimensioni e la compressione differenziale del prefisso eliminano il sovraccarico di andata e ritorno file per file.
- **Autoriparazione dopo interruzione di corrente e ripresa dopo il riavvio**: il formato ibrido RDB + AOF simile a Redis (`.s0_subs.bin`) viene utilizzato per rendere persistente lo stato del seeding e l'elenco dei file, le modifiche giornaliere vengono aggiunte in microsecondi e vengono automaticamente riscritte e organizzate regolarmente. Dopo il riavvio del server, le modifiche offline vengono automaticamente inviate al nodo di seeding.

## Architettura tecnica e documentazione

- **Protocollo di disaccoppiamento a doppio piano**:
  - **Controllo e superficie di modifica in tempo reale (`iroh` QUIC / TCP, primo byte `PROTO_VERSION` handshake versione) **: Responsabile della verifica dei biglietti da visita Ed25519, della verifica delle autorizzazioni, del rilevamento dei semi di Swarm e del supporto per `u64::MAX` batch di streaming di modifiche di massa a livello e `mzstd` push in tempo reale `SubPush` compresso (`FsOp::Upsert` / `FsOp::Remove` / `FsOp::Meta` Tre primitive).
  - **Superficie di distribuzione dei dati (UDP nativo + `compio` I/O asincrono) **: intestazione compatta da 20 B, feedback periodico TACK da 18 B, controllo della congestione ibrido BBRv3/Copa, recupero della perdita di pacchetti RACK-TLP, bypass di compressione Zstd adattivo entropico Shannon del primo blocco e verifica end-to-end del blocco `gxhash128`.
- **Saperne di più**:
  - [Report di misurazione delle prestazioni e script di riproduzione](https://github.com/webc-site/sync0/blob/main/doc/zh/benchmark.md)
  - [Progettazione dell'architettura del sistema](https://github.com/webc-site/sync0/blob/main/doc/zh/design_v4.md)
  - [Protocollo di cablaggio binario e specifica di persistenza](https://github.com/webc-site/sync0/blob/main/doc/zh/proto.md)
  - [Autenticazione dell'identità, controllo dell'autorità e verifica dell'integrità](https://github.com/webc-site/sync0/blob/main/doc/zh/crypto.md)