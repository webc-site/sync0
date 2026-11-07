# sync0

`sync0` je nástroj příkazového řádku pro přenos souborů přes sítě, postupnou synchronizaci adresářů a současnou distribuci dat na více počítačů.

## Originální designový záměr

V místnostech s více stroji, v zemích nebo prostředích se zpožděním sítě a ztrátou paketů jsou nástroje jako `rsync` a `scp` založeny na potvrzení sekvence jednoho připojení, což omezuje využití šířky pásma; když více počítačů stahuje data ze stejného serveru současně, síťová karta zdrojového serveru se může snadno stát úzkým hrdlem.

Cílem `sync0` je vyřešit dva problémy:

1. Nechte synchronizaci souborů a adresářů mezi dvěma počítači běžet s plnou fyzickou šířkou pásma a přeneste změněnou část pouze tehdy, když je soubor částečně upraven.
2. Více počítačů může při synchronizaci stejného adresáře přenášet datové bloky mezi sebou, což snižuje zatížení zdrojového serveru a zkracuje celkovou dobu synchronizace.

## Porovnání rychlosti s rsync

Testovací prostředí: Evropský uzel $\leftrightarrow$ Severoamerický uzel, zpáteční zpoždění 86 ms, měření kompletní časové náročnosti včetně skenování adresářů, porovnání deduplikace, síťového přenosu a ověření umístění disku:

| Scénář přenosu | Velikost dat | `sync0` | `rsync` | násobek |
| :--- | :---: | :---: | :---: | :---: |
| 10 000 malých souborů | 77.6 MB | 0.90 s (688 Mbps) | 4.86 s (128 Mbps) | 5.4x |
| Balíčky kódu a archivace (`.tar`) | 512 MB | 1.94 s (2111 Mbps) | 9.36 s (438 Mbps) | 4.8x |
| Video soubor (`.mp4`) | 512 MB | 7.36 s (556 Mbps) | 12.11 s (338 Mbps) | 1.6x |
| Přírůstková synchronizace (upravit 1% obsah) | 512 MB | 0.12 s | 1.85 s | 15.4x |

## Metody zlepšování výkonu

- **Souběžný přenos UDP**: Odesílejte bloky dat prostřednictvím protokolu UDP a znovu přenášejte pouze ztracené fragmenty, aby se zabránilo ztrátě jednoho paketu blokujícímu následné přenosy souborů.
- **Segmentace obsahu a deduplikace**: Rozdělte soubor do datových bloků podle obsahu a vypočítejte kontrolní hodnotu. Stejný obsah, který je již na přijímací straně, je znovu použit přímo lokálně bez duplikace sítě.
- **Automaticky upravit kompresi podle rychlosti sítě**: Zapněte kompresi pro komprimovatelný obsah, jako je text a kód; automaticky přeskakuje kompresi pro videa, obrázky, komprimované balíčky nebo pod vysokorychlostní LAN, aby se snížila spotřeba procesoru.
- **Stahování více uzlů v okolí**: Když více počítačů stahuje stejný adresář, budou datové bloky staženy současně ze stažených uzlů ve stejném městě nebo s nižší latencí.

## Instalovat

### Metoda 1: Instalace přes Cargo

```bash
cargo install sync0
```

### Metoda 2: Stáhněte si předkompilované binární soubory

Přejděte na [GitHub Releases](https://github.com/webc-site/sync0/releases) a stáhněte si komprimovaný balíček `.tar.xz` pro odpovídající systém (podporuje Linux x86_64/ARM64, macOS Apple Silicon/Intel, Windows x64/ARM64, Android ARM64/x86_64, FreeBSD, NetBSD). Po rozbalení umístěte `sync0` do adresáře `PATH`:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Uživatelská příručka

### 1. Přímý přenos souborů nebo adresářů mezi dvěma počítači

Spusťte monitorování na přijímajícím počítači a zadejte cestu příjmu a ukládání:

```bash
sync0 recv 0.0.0.0:9000 ./backup
```

Odešlete jeden soubor nebo celý adresář z odesílajícího počítače:

```bash
sync0 send ./project 192.168.1.10:9000
```

Běžně používané parametry:

```bash
# Odstraňte nadbytečné soubory, které existují na přijímající straně, ale ne na straně odesílající (zrcadlová synchronizace)
sync0 send ./project 192.168.1.10:9000 --delete

# Omezit rychlost odesílání (jednotka: bajty/sekundu, například 50 MB/s)
sync0 send ./project 192.168.1.10:9000 --rate 50000000

# Tichý režim (nezobrazuje se žádný ukazatel průběhu)
sync0 send ./project 192.168.1.10:9000 -q
```

### 2. Publikování sdílených adresářů a stahování z více uzlů

Publikování sdíleného adresáře (s názvem `dataset`) na původním serveru:

```bash
sync0 share dataset ./data --region SHA
```

Vytáhněte sdílený adresář na jiných počítačích (můžete vyplnit více adres uzlů současně, oddělených čárkami pro současné stahování):

```bash
sync0 pull 10.0.0.1:9000,10.0.0.2:9000 dataset ./local_data
```

Vytáhněte pouze určitý podadresář ve sdíleném adresáři:

```bash
sync0 pull 10.0.0.1:9000 dataset ./local_data --subpath 2026/logs
```

Přeneste místní změny zpět do vzdáleného sdíleného adresáře:

```bash
sync0 push 10.0.0.1:9000 dataset ./local_data
```

### 3. Přihlaste se k odběru automatických aktualizací katalogu

Přihlaste se k odběru sdíleného adresáře zdrojového uzlu. Když je soubor přidán nebo upraven ve zdrojovém adresáři, bude automaticky synchronizován s místním počítačem. Současně bude místní počítač automaticky sloužit jako uzel pro ostatní stroje, který mohou stáhnout:

```bash
sync0 sub 10.0.0.1:9000 dataset ./local_mirror
```

### 4. Obousměrná synchronizace skupin více strojů

Více počítačů se připojí ke stejnému názvu skupiny a změny v libovolném adresáři uzlů budou automaticky synchronizovány s ostatními uzly ve skupině:

```bash
sync0 group team-docs ./docs 10.0.0.1:9000,10.0.0.2:9000
```

### 5. Identita uzlu a osobní vizitka

Každý uzel podporuje nastavení uživatelského jména, města, e-mailu, webové stránky a personalizovaného podpisu (maximálně 256 znaků) a předává místní ověření podpisu soukromého klíče:

```bash
# Zobrazení místního ID veřejného klíče (určete konfigurační adresář, aby klíč zůstal zachován)
sync0 id -c ~/.config/sync0

# Generujte a zobrazujte vizitky uzlu s personalizovanými podpisy
sync0 profile -c ~/.config/sync0 \
  -u alice \
  --region SHA \
  --email alice@example.com \
  --url https://example.com \
  -b "Shanghai backup node"
```

`share`, `pull`, `sub`, `group` lze také spustit s těmito parametry profilu:

```bash
sync0 share dataset ./data -c ~/.config/sync0 -u alice --region SHA -b "Shanghai backup node"
```

### 6. Vícejazyčné přepínání

`sync0` Podporuje 75 jazyků. Ve výchozím nastavení se nápověda a výzvy příkazového řádku v odpovídajícím jazyce automaticky zobrazují podle jazykového prostředí operačního systému; pokud systémový jazyk není v seznamu podporovaných, použije se ve výchozím nastavení angličtina.

Chcete-li jazyk zadat ručně, nastavte proměnnou prostředí `SYNC0_LANG`:

```bash
SYNC0_LANG=en sync0 --help
SYNC0_LANG=ja sync0 --help
SYNC0_LANG=de sync0 --help
```