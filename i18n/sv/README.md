# sync0

`sync0` är ett kommandoradsverktyg för att överföra filer över nätverk, stegvis synkronisera kataloger och samtidigt distribuera data till flera maskiner.

## Original design avsikt

I maskinöverskridande rum, länder eller miljöer med nätverksfördröjningar och paketförluster är verktyg som `rsync` och `scp` baserade på bekräftelse av en enda anslutningssekvens, vilket begränsar användningen av bandbredd; när flera maskiner drar data från samma server samtidigt, kan källserverns nätverkskort lätt bli en flaskhals.

Målet med `sync0` är att lösa två problem:

1. Låt fil- och katalogsynkroniseringen mellan de två maskinerna köra full fysisk bandbredd och överför endast den ändrade delen när filen är delvis modifierad.
2. Flera maskiner kan överföra datablock till varandra när man synkroniserar samma katalog, vilket minskar belastningen på källservern och förkortar den totala synkroniseringstiden.

## Hastighetsjämförelse med rsync

Testmiljö: Europeisk nod $\leftrightarrow$ Nordamerikansk nod, fördröjning tur och retur 86 ms, mätning av den fullständiga tidskrävande inklusive katalogskanning, dedupliceringsjämförelse, nätverksöverföring och verifiering av diskplacering:

| Överföringsscenario | Datastorlek | `sync0` | `rsync` | multipel |
| :--- | :---: | :---: | :---: | :---: |
| 10 000 små filer | 77.6 MB | 0.90 s (688 Mbps) | 4.86 s (128 Mbps) | 5.4x |
| Kod- och arkivpaket (`.tar`) | 512 MB | 1.94 s (2111 Mbps) | 9.36 s (438 Mbps) | 4.8x |
| Videofil (`.mp4`) | 512 MB | 7.36 s (556 Mbps) | 12.11 s (338 Mbps) | 1.6x |
| Inkrementell synkronisering (ändra 1 % innehåll) | 512 MB | 0.12 s | 1.85 s | 15.4x |

## Prestandaförbättringsmetoder

- **UDP samtidig överföring**: Skicka datablock genom UDP och sänd bara om förlorade fragment för att undvika att en enda paketförlust blockerar efterföljande filöverföringar.
- **Innehållssegmentering och deduplicering**: Dela upp filen i datablock enligt innehåll och beräkna kontrollvärdet. Samma innehåll som redan finns på den mottagande sidan återanvänds direkt lokalt utan att nätverket dupliceras.
- **Justera komprimering automatiskt efter nätverkshastighet**: Aktivera komprimering för komprimerbart innehåll som text och kod; automatiskt hoppa över komprimering för videor, bilder, komprimerade paket eller under höghastighets-LAN för att minska CPU-förbrukningen.
- **Närliggande dragning av flera noder**: När flera maskiner drar samma katalog kommer datablock att hämtas samtidigt från nedladdade noder i samma stad eller med lägre latens.

## Installera

### Metod 1: Installera via Cargo

```bash
cargo install sync0
```

### Metod 2: Ladda ner förkompilerade binärer

Gå till [GitHub Releases](https://github.com/webc-site/sync0/releases) för att ladda ner det `.tar.xz` komprimerade paketet för motsvarande system (stöder Linux x86_64/ARM64, macOS Apple Silicon/Intel, Windows x64/ARM64, Android ARM64/x86_64, FreeBSD, NetBSD). Efter att ha packat upp, placera `sync0` i `PATH` katalogen:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Användarhandbok

### 1. Överför filer eller kataloger direkt mellan två maskiner

Starta övervakningen på den mottagande maskinen och ange mottagnings- och sparvägen:

```bash
sync0 recv 0.0.0.0:9000 ./backup
```

Skicka en enskild fil eller en hel katalog från den sändande maskinen:

```bash
sync0 send ./project 192.168.1.10:9000
```

Vanligt använda parametrar:

```bash
# Ta bort redundanta filer som finns på den mottagande sidan men inte på den sändande sidan (spegelsynkronisering)
sync0 send ./project 192.168.1.10:9000 --delete

# Begränsa sändningshastigheten (enhet: byte/sekund, till exempel 50 MB/s)
sync0 send ./project 192.168.1.10:9000 --rate 50000000

# Tyst läge (ingen förloppsindikator visas)
sync0 send ./project 192.168.1.10:9000 -q
```

### 2. Publicera delade kataloger och hämta från flera noder

Publicera en delad katalog (med namnet `dataset`) på ursprungsservern:

```bash
sync0 share dataset ./data --region SHA
```

Dra den delade katalogen på andra maskiner (du kan fylla i flera nodadresser samtidigt, separerade med kommatecken för att dra samtidigt):

```bash
sync0 pull 10.0.0.1:9000,10.0.0.2:9000 dataset ./local_data
```

Dra bara en viss underkatalog i den delade katalogen:

```bash
sync0 pull 10.0.0.1:9000 dataset ./local_data --subpath 2026/logs
```

Tryck tillbaka lokala ändringar till den delade fjärrkatalogen:

```bash
sync0 push 10.0.0.1:9000 dataset ./local_data
```

### 3. Prenumerera på automatiska uppdateringar av katalogen

Prenumerera på den delade katalogen för källnoden. När en fil läggs till eller ändras i källkatalogen synkroniseras den automatiskt till den lokala datorn. Samtidigt kommer den lokala maskinen automatiskt att fungera som en nod för andra maskiner att dra:

```bash
sync0 sub 10.0.0.1:9000 dataset ./local_mirror
```

### 4. Tvåvägssynkronisering av flermaskinsgrupper

Flera maskiner går med i samma gruppnamn och ändringar i valfri nodkatalog synkroniseras automatiskt med andra noder i gruppen:

```bash
sync0 group team-docs ./docs 10.0.0.1:9000,10.0.0.2:9000
```

### 5. Nodidentitet och personligt visitkort

Varje nod stöder inställning av användarnamn, stad, e-post, webbplats och en personlig signatur (max 256 tecken), och godkänner den lokala privata nyckelsignaturverifieringen:

```bash
# Visa den lokala publika nyckel-ID (ange konfigurationskatalogen för att bevara nyckeln)
sync0 id -c ~/.config/sync0

# Generera och visa nodvisitkort med personliga signaturer
sync0 profile -c ~/.config/sync0 \
  -u alice \
  --region SHA \
  --email alice@example.com \
  --url https://example.com \
  -b "Shanghai backup node"
```

`share`, `pull`, `sub`, `group` kan också startas med dessa profilparametrar:

```bash
sync0 share dataset ./data -c ~/.config/sync0 -u alice --region SHA -b "Shanghai backup node"
```

### 6. Flerspråksväxling

`sync0` Stöder 75 språk. Som standard visas kommandoradshjälp och uppmaningar på motsvarande språk automatiskt i enlighet med operativsystemets språkmiljö; om systemspråket inte finns i listan som stöds används engelska som standard.

För att ange språket manuellt, ställ in miljövariabeln `SYNC0_LANG`:

```bash
SYNC0_LANG=en sync0 --help
SYNC0_LANG=ja sync0 --help
SYNC0_LANG=de sync0 --help
```