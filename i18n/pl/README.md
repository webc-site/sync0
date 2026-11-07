# sync0

`sync0` to narzędzie wiersza poleceń umożliwiające przesyłanie plików przez sieci, przyrostową synchronizację katalogów i równoczesną dystrybucję danych do wielu komputerów.

## Oryginalna intencja projektowa

W pomieszczeniach, w których występuje wiele maszyn, w krajach lub środowiskach, w których występują opóźnienia w sieci i utrata pakietów, narzędzia takie jak `rsync` i `scp` opierają się na potwierdzeniu pojedynczej sekwencji połączenia, co ogranicza wykorzystanie przepustowości; gdy wiele maszyn pobiera dane z tego samego serwera w tym samym czasie, karta sieciowa serwera źródłowego może łatwo stać się wąskim gardłem.

Celem `sync0` jest rozwiązanie dwóch problemów:

1. Pozwól, aby synchronizacja plików i katalogów między dwoma komputerami przebiegała z pełną przepustowością fizyczną i przesyłaj zmienioną część tylko wtedy, gdy plik jest częściowo zmodyfikowany.
2. Wiele maszyn może przesyłać między sobą bloki danych podczas synchronizacji tego samego katalogu, zmniejszając obciążenie serwera źródłowego i skracając całkowity czas synchronizacji.

## Porównanie prędkości z rsync

Środowisko testowe: węzeł europejski $\leftrightarrow$ węzeł północnoamerykański, opóźnienie w obie strony 86 ms, pomiar całkowitej czasochłonności, w tym skanowanie katalogów, porównanie deduplikacji, transmisja sieciowa i weryfikacja rozmieszczenia dysków:

| Scenariusz transmisji | Rozmiar danych | `sync0` | `rsync` | wiele  |
| :--- | :---: | :---: | :---: | :---: |
| 10 000 małych plików | 77.6 MB | 0.90 s (688 Mbps) | 4.86 s (128 Mbps) | 5.4x |
| Pakiety kodu i archiwum (`.tar`) | 512 MB | 1.94 s (2111 Mbps) | 9.36 s (438 Mbps) | 4.8x |
| Plik wideo (`.mp4`) | 512 MB | 7.36 s (556 Mbps) | 12.11 s (338 Mbps) | 1.6x |
| Synchronizacja przyrostowa (zmodyfikuj zawartość 1%) | 512 MB | 0.12 s | 1.85 s | 15.4x |

## Metody poprawy wydajności

- **Wspólna transmisja UDP**: Wysyłaj bloki danych przez UDP i retransmituj tylko utracone fragmenty, aby uniknąć zablokowania przez utratę pojedynczego pakietu kolejnych transferów plików.
- **Segmentacja treści i deduplikacja**: Podziel plik na bloki danych zgodnie z treścią i oblicz wartość kontrolną. Ta sama treść już po stronie odbiorcy jest ponownie wykorzystywana bezpośrednio lokalnie, bez duplikowania sieci.
- **Automatycznie dostosuj kompresję do szybkości sieci**: Włącz kompresję dla treści ściśliwych, takich jak tekst i kod; automatycznie pomija kompresję filmów, zdjęć, skompresowanych pakietów lub w przypadku szybkiej sieci LAN, aby zmniejszyć zużycie procesora.
- **Wyciąganie z wielu węzłów w pobliżu**: Gdy wiele komputerów pobiera ten sam katalog, bloki danych będą pobierane jednocześnie z pobranych węzłów w tym samym mieście lub z mniejszym opóźnieniem.

## Zainstalować

### Metoda 1: Zainstaluj przez Cargo

```bash
cargo install sync0
```

### Metoda 2: Pobierz prekompilowane pliki binarne

Przejdź do [GitHub Releases](https://github.com/webc-site/sync0/releases), aby pobrać skompresowany pakiet `.tar.xz` dla odpowiedniego systemu (obsługuje Linux x86_64/ARM64, macOS Apple Silicon/Intel, Windows x64/ARM64, Android ARM64/x86_64, FreeBSD, NetBSD). Po rozpakowaniu umieść `sync0` w katalogu `PATH`:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Podręcznik użytkownika

### 1. Bezpośrednio przesyłaj pliki lub katalogi między dwoma maszynami

Rozpocznij monitorowanie na maszynie odbiorczej i określ ścieżkę odbioru i zapisu:

```bash
sync0 recv 0.0.0.0:9000 ./backup
```

Wyślij pojedynczy plik lub cały katalog z maszyny wysyłającej:

```bash
sync0 send ./project 192.168.1.10:9000
```

Często używane parametry:

```bash
# Usuń zbędne pliki, które istnieją po stronie odbierającej, ale nie po stronie wysyłającej (synchronizacja lustrzana)
sync0 send ./project 192.168.1.10:9000 --delete

# Ogranicz szybkość wysyłania (jednostka: bajty/sekundę, na przykład 50 MB/s)
sync0 send ./project 192.168.1.10:9000 --rate 50000000

# Tryb cichy (nie jest wyświetlany pasek postępu)
sync0 send ./project 192.168.1.10:9000 -q
```

### 2. Publikowanie współdzielonych katalogów i pobieranie z wielu węzłów

Opublikuj katalog współdzielony (o nazwie `dataset`) na serwerze źródłowym:

```bash
sync0 share dataset ./data --region SHA
```

Ściągnij współdzielony katalog na innych komputerach (możesz wpisać wiele adresów węzłów jednocześnie, oddzielając je przecinkami, aby pobierać jednocześnie):

```bash
sync0 pull 10.0.0.1:9000,10.0.0.2:9000 dataset ./local_data
```

Ściągnij tylko określony podkatalog w katalogu współdzielonym:

```bash
sync0 pull 10.0.0.1:9000 dataset ./local_data --subpath 2026/logs
```

Wypchnij lokalne zmiany z powrotem do zdalnego katalogu współdzielonego:

```bash
sync0 push 10.0.0.1:9000 dataset ./local_data
```

### 3. Zapisz się na automatyczne aktualizacje katalogu

Subskrybuj katalog współdzielony węzła źródłowego. Kiedy plik zostanie dodany lub zmodyfikowany w katalogu źródłowym, zostanie on automatycznie zsynchronizowany z komputerem lokalnym. Jednocześnie maszyna lokalna będzie automatycznie służyć jako węzeł, z którego będą mogły korzystać inne maszyny:

```bash
sync0 sub 10.0.0.1:9000 dataset ./local_mirror
```

### 4. Dwukierunkowa synchronizacja grup wielomaszynowych

Wiele komputerów dołącza do tej samej nazwy grupy, a zmiany w dowolnym katalogu węzłów zostaną automatycznie zsynchronizowane z innymi węzłami w grupie:

```bash
sync0 group team-docs ./docs 10.0.0.1:9000,10.0.0.2:9000
```

### 5. Tożsamość węzła i wizytówka osobista

Każdy węzeł obsługuje ustawienie nazwy użytkownika, miasta, adresu e-mail, strony internetowej i spersonalizowanego podpisu (maksymalnie 256 znaków) oraz przechodzi uwierzytelnianie podpisu lokalnego klucza prywatnego:

```bash
# Wyświetl identyfikator lokalnego klucza publicznego (określ katalog konfiguracyjny, w którym chcesz zachować klucz)
sync0 id -c ~/.config/sync0

# Generuj i wyświetlaj wizytówki węzłów ze spersonalizowanymi podpisami
sync0 profile -c ~/.config/sync0 \
  -u alice \
  --region SHA \
  --email alice@example.com \
  --url https://example.com \
  -b "Shanghai backup node"
```

`share`, `pull`, `sub`, `group` można także uruchomić z następującymi parametrami profilu:

```bash
sync0 share dataset ./data -c ~/.config/sync0 -u alice --region SHA -b "Shanghai backup node"
```

### 6. Przełączanie wielu języków

`sync0` Obsługuje 75 języków. Domyślnie pomoc wiersza poleceń i podpowiedzi w odpowiednim języku są wyświetlane automatycznie, zgodnie ze środowiskiem językowym systemu operacyjnego; jeśli języka systemu nie ma na liście obsługiwanych języków, domyślnie używany jest język angielski.

Aby ręcznie określić język, ustaw zmienną środowiskową `SYNC0_LANG`:

```bash
SYNC0_LANG=en sync0 --help
SYNC0_LANG=ja sync0 --help
SYNC0_LANG=de sync0 --help
```