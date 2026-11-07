# sync0

`sync0` è uno strumento da riga di comando per trasferire file attraverso reti, sincronizzare in modo incrementale le directory e distribuire contemporaneamente dati su più macchine.

## Intenzione progettuale originale

In sale, paesi o ambienti tra macchine diverse con ritardi di rete e perdita di pacchetti, strumenti come `rsync` e `scp` si basano sulla conferma di una singola sequenza di connessione, che limita l'utilizzo della larghezza di banda; quando più macchine estraggono dati dallo stesso server contemporaneamente, la scheda di rete del server di origine può facilmente diventare un collo di bottiglia.

L'obiettivo di `sync0` è risolvere due problemi:

1. Lascia che la sincronizzazione di file e directory tra le due macchine esegua l'intera larghezza di banda fisica e trasferisci la parte modificata solo quando il file viene parzialmente modificato.
2. Più macchine possono trasferire tra loro blocchi di dati durante la sincronizzazione della stessa directory, riducendo il carico sul server di origine e abbreviando il tempo complessivo di sincronizzazione.

## Confronto di velocità con rsync

Ambiente di test: nodo europeo $\leftrightarrow$ nodo nordamericano, ritardo di andata e ritorno 86 ms, misurazione del tempo impiegato completo inclusa la scansione delle directory, il confronto della deduplicazione, la trasmissione di rete e la verifica del posizionamento del disco:

| Scenario di trasmissione | Dimensione dei dati | `sync0` | `rsync` | multiplo |
| :--- | :---: | :---: | :---: | :---: |
| 10.000 piccoli file | 77.6 MB | 0.90 s (688 Mbps) | 4.86 s (128 Mbps) | 5.4x |
| Pacchetti di codice e archivio (`.tar`) | 512 MB | 1.94 s (2111 Mbps) | 9.36 s (438 Mbps) | 4.8x |
| File video (`.mp4`) | 512 MB | 7.36 s (556 Mbps) | 12.11 s (338 Mbps) | 1.6x |
| Sincronizzazione incrementale (modifica 1% del contenuto) | 512 MB | 0.12 s | 1.85 s | 15.4x |

## Metodi di miglioramento delle prestazioni

- **Trasmissione simultanea UDP**: invia blocchi di dati tramite UDP e ritrasmetti solo i frammenti persi per evitare che la perdita di un singolo pacchetto blocchi i successivi trasferimenti di file.
- **Segmentazione e deduplicazione del contenuto**: suddividi il file in blocchi di dati in base al contenuto e calcola il valore di controllo. Lo stesso contenuto già presente sul lato ricevente viene riutilizzato direttamente a livello locale senza duplicare la rete.
- **Regola automaticamente la compressione in base alla velocità della rete**: attiva la compressione per contenuti comprimibili come testo e codice; salta automaticamente la compressione per video, immagini, pacchetti compressi o in LAN ad alta velocità per ridurre il consumo della CPU.
- **Estrazione multi-nodo nelle vicinanze**: quando più macchine estraggono la stessa directory, i blocchi di dati verranno estratti contemporaneamente dai nodi scaricati nella stessa città o con una latenza inferiore.

## Installare

### Metodo 1: installazione tramite Cargo

```bash
cargo install sync0
```

### Metodo 2: scaricare i file binari precompilati

Vai a [GitHub Releases](https://github.com/webc-site/sync0/releases) per scaricare il pacchetto compresso `.tar.xz` per il sistema corrispondente (supporta Linux x86_64/ARM64, macOS Apple Silicon/Intel, Windows x64/ARM64, Android ARM64/x86_64, FreeBSD, NetBSD). Dopo aver decompresso, inserisci `sync0` nella directory `PATH`:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Guida per l'utente

### 1. Trasferisci direttamente file o directory tra due macchine

Avvia il monitoraggio sulla macchina ricevente e specifica il percorso di ricezione e salvataggio:

```bash
sync0 recv 0.0.0.0:9000 ./backup
```

Invia un singolo file o un'intera directory dalla macchina mittente:

```bash
sync0 send ./project 192.168.1.10:9000
```

Parametri comunemente usati:

```bash
# Elimina i file ridondanti che esistono sul lato ricevente ma non sul lato mittente (sincronizzazione mirror)
sync0 send ./project 192.168.1.10:9000 --delete

# Limita la velocità di invio (unità: byte/secondo, ad esempio 50 MB/s)
sync0 send ./project 192.168.1.10:9000 --rate 50000000

# Modalità silenziosa (nessuna barra di avanzamento mostrata)
sync0 send ./project 192.168.1.10:9000 -q
```

### 2. Pubblicazione di directory condivise ed estrazione da più nodi

Pubblica una directory condivisa (denominata `dataset`) sul server di origine:

```bash
sync0 share dataset ./data --region SHA
```

Estrai la directory condivisa su altre macchine (puoi inserire più indirizzi di nodo contemporaneamente, separati da virgole per eseguire l'estrazione contemporaneamente):

```bash
sync0 pull 10.0.0.1:9000,10.0.0.2:9000 dataset ./local_data
```

Estrai solo una determinata sottodirectory nella directory condivisa:

```bash
sync0 pull 10.0.0.1:9000 dataset ./local_data --subpath 2026/logs
```

Ripristina le modifiche locali alla directory condivisa remota:

```bash
sync0 push 10.0.0.1:9000 dataset ./local_data
```

### 3. Iscriviti agli aggiornamenti automatici del catalogo

Sottoscrivere la directory condivisa del nodo di origine. Quando un file viene aggiunto o modificato nella directory di origine, verrà automaticamente sincronizzato con il computer locale. Allo stesso tempo, la macchina locale fungerà automaticamente da nodo per il pull di altre macchine:

```bash
sync0 sub 10.0.0.1:9000 dataset ./local_mirror
```

### 4. Sincronizzazione bidirezionale di gruppi multi-macchina

Più macchine si uniscono allo stesso nome di gruppo e le modifiche in qualsiasi directory del nodo verranno automaticamente sincronizzate con gli altri nodi del gruppo:

```bash
sync0 group team-docs ./docs 10.0.0.1:9000,10.0.0.2:9000
```

### 5. Identità del nodo e biglietto da visita personale

Ogni nodo supporta l'impostazione del nome utente, della città, dell'e-mail, del sito Web e di una firma personalizzata (massimo 256 caratteri) e supera l'autenticazione della firma della chiave privata locale:

```bash
# Visualizza l'ID della chiave pubblica locale (specifica la directory di configurazione per rendere persistente la chiave)
sync0 id -c ~/.config/sync0

# Genera e visualizza biglietti da visita del nodo con firme personalizzate
sync0 profile -c ~/.config/sync0 \
  -u alice \
  --region SHA \
  --email alice@example.com \
  --url https://example.com \
  -b "Shanghai backup node"
```

`share`, `pull`, `sub`, `group` possono anche essere avviati con questi parametri del profilo:

```bash
sync0 share dataset ./data -c ~/.config/sync0 -u alice --region SHA -b "Shanghai backup node"
```

### 6. Commutazione multilingue

`sync0` Supporta 75 lingue. Per impostazione predefinita, la guida della riga di comando e i prompt nella lingua corrispondente vengono visualizzati automaticamente in base all'ambiente linguistico del sistema operativo; se la lingua del sistema non è nell'elenco supportato, per impostazione predefinita viene utilizzato l'inglese.

Per specificare manualmente la lingua, imposta la variabile di ambiente `SYNC0_LANG`:

```bash
SYNC0_LANG=en sync0 --help
SYNC0_LANG=ja sync0 --help
SYNC0_LANG=de sync0 --help
```