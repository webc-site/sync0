# sync0

`sync0` to narzędzie do udostępniania plików w trybie peer-to-peer (P2P) i synchronizacji na wielu komputerach, które konkuruje z Resilio Sync.

Każda maszyna jest zarówno serwerem, jak i klientem: każdy węzeł może publikować wiele grup udostępniania plików. Po dołączeniu innych węzłów mogą one automatycznie uzyskać dostępne węzły początkowe z serwera równorzędnego i pobrać je jednocześnie z wielu źródeł. Po zakończeniu synchronizacji automatycznie zostaną utworzone nasiona dla większej liczby węzłów, aby przyspieszyć pobieranie.

## Najważniejsze informacje

- **Każdy jest zarówno serwerem, jak i klientem**: Porównywalne z Resilio Sync, brak zależności od serwera centralnego. Każdy węzeł może utworzyć wiele grup udostępniania plików, a także może dołączać do grup udostępniania innych osób; węzły automatycznie wymieniają się i uzyskują dostępne węzły początkowe, aby osiągnąć jednoczesne ciągnięcie i synchronizację wielu maszyn w czasie rzeczywistym.
- **Adres węzła bez adresu IP**: Domyślnie klucz węzła i zakodowany `base64url` adres `iroh` są automatycznie generowane w `~/.config/sync0` bez konieczności zapamiętywania lub ujawniania publicznego adresu IP.
- **Wiele synchronizacji na żądanie**: Obsługuje synchronizację całej grupy udostępniania, a także obsługuje synchronizację na żądanie poprzez jednoczesne wybranie wielu plików lub podkatalogów.
- **Uprawnienia hierarchiczne członków grupy i kontrola katalogu przesyłania**: Obsługuje uprawnienia tylko do odczytu (`ro`), przesyłania do odczytu i zapisu (`rw`) oraz uprawnienia do zarządzania (`admin`) i może indywidualnie określić przedrostek ścieżki, który każdy członek może przesyłać (domyślnie dozwolone jest przesyłanie do katalogu głównego udziału).
- **Pełna prędkość i przyrostowa synchronizacja w sieciach**: Jednoczesna transmisja dużych plików, agregacja ogromnych małych plików w pamięci i synchronizacja bloków danych tylko wtedy, gdy pliki są częściowo zmodyfikowane.

## Porównanie prędkości z rsync

### Rzeczywisty pomiar z jednego źródła w sieci transoceanicznej (Chiny → Stany Zjednoczone, opóźnienie 260 ms)

| Scenariusz transmisji | Rozmiar danych | model | `sync0` | `rsync -az` | Zwiększ mnożnik |
| :--- | :---: | :---: | :---: | :---: | :---: |
| 500 małych plików | 1.91 MB | Pełna synchronizacja | 18.84 Mbps | 5.32 Mbps | 3.5x |
| 500 małych plików | 1.91 MB | Synchronizacja przyrostowa | 27.15 Mbps | 3.83 Mbps | 7.1x |
| Pakiety kodu i archiwum (`.tar`) | 10.0 MB | Pełna synchronizacja | 139.70 Mbps | 24.97 Mbps | 5.6x |
| Pakiety kodu i archiwum (`.tar`) | 10.0 MB | Synchronizacja przyrostowa | 83.38 Mbps | 26.02 Mbps | 3.2x |
| Plik wideo (`.mp4`) | 15.0 MB | Pełna synchronizacja | 54.06 Mbps | 27.04 Mbps | 2.0x |
| Plik wideo (`.mp4`) | 15.0 MB | Synchronizacja przyrostowa | 216.83 Mbps | 21.51 Mbps | 10.1x |

### Test ściągania wielu węzłów sieci transoceanicznej P2P (podwójne węzły Chiny → Stany Zjednoczone, opóźnienie 260 ms)

Zmierzone porównanie jednoczesnego przyciągania pasiastego węzła zarodkowego z wielu źródeł:

| Scenariusz transmisji | Rozmiar danych | Zasiew pojedynczego węzła | Współbieżne udostępnianie dwóch węzłów P2P | Zwiększ mnożnik |
| :--- | :---: | :---: | :---: | :---: |
| 500 małych plików | 1.97 MB | 5.28 Mbps | 7.87 Mbps | 1.5x |
| Pakiety kodu i archiwum (`.tar`) | 30.0 MB | 56.35 Mbps | 83.17 Mbps | 1.5x |
| Plik wideo (`.mp4`) | 30.0 MB | 27.00 Mbps | 39.46 Mbps | 1.5x |

### Rzeczywisty pomiar transkontynentalnej szybkiej linii dedykowanej (Europa → Ameryka Północna, opóźnienie 86 ms)

| Scenariusz transmisji | Rozmiar danych | `sync0` | `rsync` | Zwiększ mnożnik |
| :--- | :---: | :---: | :---: | :---: |
| 10 000 małych plików | 77.6 MB | 688 Mbps | 128 Mbps | 5.4x |
| Pakiety kodu i archiwum (`.tar`) | 512 MB | 2111 Mbps | 438 Mbps | 4.8x |
| Plik wideo (`.mp4`) | 512 MB | 556 Mbps | 338 Mbps | 1.6x |
| Synchronizacja przyrostowa (zmodyfikuj zawartość 1%) | 512 MB | 34133 Mbps | 2214 Mbps | 15.4x |

## Zainstalować

### Metoda 1: Zainstaluj przez Cargo

```bash
cargo install sync0
```

### Metoda 2: Pobierz prekompilowane pliki binarne

Przejdź do [GitHub Releases](https://github.com/webc-site/sync0/releases), aby pobrać `.tar.xz` skompresowany pakiet dla odpowiedniego systemu (obsługuje Linux, macOS, Windows, Android, FreeBSD, NetBSD), rozpakuj i umieść go w katalogu `PATH`:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Zacznij szybko

`sync0` domyślnie używa `~/.config/sync0` jako katalogu konfiguracyjnego (jeśli chcesz przełączyć się do innych katalogów, możesz dodać `-c <katalog>`).

### 1. Wyświetl adres węzła lokalnego i wizytówkę ustawień

```bash
# Wyświetl adres węzła lokalnego (klucz jest generowany automatycznie w ~/.config/sync0 po raz pierwszy)
sync0 id

# Wygeneruj osobistą wizytówkę węzła (obsługuje nazwę użytkownika, miasto, adres e-mail, stronę internetową i podpis w 256 słowach)
sync0 profile -u alice --region SHA --email alice@example.com --url https://example.com -b "Shanghai node"
```

### 2. Publikuj wiele grup udostępniania plików i ustawiaj uprawnienia członków grupy (`.nt` Konfiguracja)

`sync0` jest skonfigurowany w formacie [NestedText](https://nestedtext.org) (`.nt`). Każdy użytkownik ma oddzielny plik konfiguracyjny `.nt`. Możesz określić katalog profilu użytkownika poprzez `users_dir` w `config.nt` (który automatycznie zawiera wszystkie pliki `.nt` w tym katalogu). Zarówno użytkownik, jak i prefiks ścieżki, który można przesłać, używają listy nieuporządkowanej (`-`). **Jeśli prefiks ścieżki przesyłania nie jest wypełniony, domyślnie dozwolone jest przesyłanie do katalogu głównego udziału**.

1. Utwórz osobne pliki konfiguracyjne `.nt` dla każdego elementu w katalogu `~/.config/sync0/users/`:

`~/.config/sync0/users/alice.nt` (członek tylko do odczytu, prędkość ograniczona `80MB`, `rate` obsługuje jednostki `GB`, `MB`, `KB`):

```yaml
id: 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c
perm: ro
rate: 80MB
```

`~/.config/sync0/users/bob.nt` (członkowie z ograniczonym przesyłaniem: mogą przesyłać tylko do katalogów `uploads/bob` i `shared/common`):

```yaml
id: 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f
perm: rw
write_prefix:
  - uploads/bob
  - shared/common
```

`~/.config/sync0/users/charlie.nt` (Domyślne przesyłanie członków: jeśli pominięto `write_prefix`, domyślnie dozwolone jest przesyłanie do katalogu głównego udziału):

```yaml
id: 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
perm: rw
```

2. Skonfiguruj katalog użytkowników i wiele grup udostępniania plików w `~/.config/sync0/config.nt`:

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

Rozpocznij usługę udostępniania:

```bash
# Uruchamianie bezpośrednie (automatycznie ładuje wszystkie konfiguracje użytkowników .nt w katalogach ~/.config/sync0/config.nt i users_dir)
sync0 share --region SHA

# Możesz też szybko opublikować wiele grup udostępniania i autoryzować członków grupy bezpośrednio za pomocą wiersza poleceń
sync0 share dataset:./data team-vault:./vault \
  --auth \
  --allow-ro 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c \
  --allow-rw 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f:uploads/bob,shared/common \
  --allow-rw 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
```

### 3. Pobierz całą zawartość lub wybierz wiele plików i katalogów (`sync0 pull`)

Połącz się z dowolnym węzłem równorzędnym, aby automatycznie uzyskać wszystkie dostępne węzły początkowe w grupie udostępniania i pobierać dane z wielu węzłów początkowych jednocześnie:

```bash
# Przeciągnij całą grupę udostępniania zestawu danych do lokalnego pliku ./local_data
sync0 pull dataset:// ./local_data -p <adres węzła>

# Synchronizacja wielokrotnego wyboru: jednocześnie pobierz pojedynczy plik docs/readme.md i podkatalogi Assets/icons, src/core
sync0 pull dataset://docs/readme.md,assets/icons,src/core ./local_data -p <adres węzła>
```

### 4. Prześlij plik lub katalog do grupy udostępniania (`sync0 push`)

Członkowie z uprawnieniami do przesyłania (`rw` lub `admin`) mogą przesyłać lokalne pliki lub katalogi do zdalnych grup udostępniania:

```bash
# Członkowie z uprawnieniami do przesyłania do katalogu głównego: Prześlij bezpośrednio do udostępnionego katalogu głównego
sync0 push team-vault:// ./new_release -p <adres węzła>

# Tylko członkowie autoryzowani do określonego prefiksu (takiego jak przesyłanie/bob): przesyłaj do dozwolonego prefiksu ścieżki
sync0 push team-vault://uploads/bob/report.pdf ./report.pdf -p <adres węzła>
```

### 5. Subskrybuj automatyczną synchronizację i udostępnianie (`sync0 sub`)

Po zapisaniu się do grupy udostępniania określonego węzła serwer lokalny jest zarówno klientem odbierającym, jak i nowym serwerem inicjującym; po zaktualizowaniu pliku węzła źródłowego jest on automatycznie synchronizowany i zapewnia pobieranie fragmentów dla innych węzłów:

```bash
# Subskrybuj całą grupę udostępniania i automatycznie wysyłaj
sync0 podzbiór danych:// ./local_mirror -p <adres węzła>

# Tylko wybór wielokrotny umożliwia subskrybowanie określonych plików i podkatalogów oraz ich umieszczanie
sync0 subdataset://models/llama.onnx,configs ./local_mirror -p <adres węzła>
```

### 6. Synchronizacja równorzędna grupy wielu komputerów (`sync0 group`)

Wiele komputerów dołącza do tej samej grupy udostępniania, wzajemnie automatycznie odkrywają dostępne węzły początkowe i utrzymują dwukierunkową synchronizację (obsługuje pełną synchronizację lub wielokrotny wybór plików i katalogów):

```bash
# Pełne dołączenie do grupy i dwukierunkowa synchronizacja
grupa sync0 zespół-docs:// ./docs -p <adres węzła>

# Tylko katalog wal i plik meta.json w grupie synchronizacji wielokrotnego wyboru
grupa sync0 zespół-docs://wal,meta.json ./docs -p <adres węzła>
```

### 7. Bezpośrednia transmisja punkt-punkt pomiędzy dwoma maszynami (`send` / `recv`)

Nie ma potrzeby wstępnej konfiguracji grupy i możesz tymczasowo przesyłać pliki lub katalogi bezpośrednio między dwoma komputerami:

```bash
# Monitorowanie odbiornika
sync0 recv 0.0.0.0:9000 ./backup

# Strona wysyłająca bezpośrednio przesyła pliki lub katalogi (opcjonalnie --delete Mirror w celu usunięcia nadmiaru plików, --rate ograniczenie prędkości)
sync0 send ./project 192.168.1.10:9000
```

### 8. Przełączanie wielu języków

`sync0` obsługuje 75 języków i domyślnie przełącza się automatycznie zgodnie ze środowiskiem językowym systemu; można to również określić ręcznie za pomocą `SYNC0_LANG`:

```bash
SYNC0_LANG=en sync0 --help
SYNC0_LANG=ja sync0 --help
SYNC0_LANG=de sync0 --help
```