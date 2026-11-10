# sync0

`sync0` to zdecentralizowane narzędzie do udostępniania plików P2P, synchronizacji w czasie rzeczywistym i dystrybucji z wielu źródeł, służące do zastąpienia `rsync` - zmierzona prędkość transkontynentalnej sieci rozległej wynosi do **6,95×** `rsync` (jednopunktowa średnia geometryczna prędkość **2,01×**, średnia geometryczna prędkości P2P z dwóch źródeł **3,33×**), a synchronizacja masowych małych plików przez oceany jest tak szybki, jak **7,19×**.

Każda maszyna jest zarówno serwerem, jak i klientem: nie jest potrzebny centralny serwer ani publiczny adres sieciowy, automatyczne dziurkowanie opiera się na adresie klucza publicznego sieci rozproszonej i obsługuje bezpośrednią transmisję między dwoma maszynami, hosting uprawnień grupowych z wieloma współużytkowaniami, wielokrotny wybór podkatalogów na żądanie, przyrostową synchronizację na poziomie milisekund w czasie rzeczywistym i współbieżne przyspieszenie P2P z wieloma źródłami.

## Zainstalować

### Metoda 1: Instalacja ładunku

```bash
cargo install sync0
```

### Metoda 2: Pobierz prekompilowane pliki binarne

Przejdź do [GitHub Releases](https://github.com/webc-site/sync0/releases), aby pobrać `.tar.xz` skompresowany pakiet odpowiadający systemowi (obsługuje Linux, macOS, Windows, Android, iOS, FreeBSD), rozpakuj i umieść w `PATH`:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Zacznij szybko

### 1. Bezpośrednia transmisja bez konfiguracji pomiędzy dwoma maszynami (`send` / `recv`)

Nie ma potrzeby konfigurowania grupy udostępniania, bezpośrednio przesyłaj pliki lub katalogi między dwoma komputerami z dużą szybkością:

```bash
# Strona odbiorcza nasłuchuje na porcie i zapisuje do ./backup
sync0 recv 0.0.0.0:9000 ./backup

# Bezpośrednia transmisja od nadawcy (opcjonalnie --delete Mirror w celu usunięcia nadmiaru plików, --rate ograniczenie prędkości 50MB)
sync0 send ./project 192.168.1.10:9000
```

### 2. Udostępnianie postów i pobieranie/przesyłanie na żądanie (`share` / `pull` / `push`)

```bash
# Serwer: Opublikuj grupę udostępniania za pomocą jednej linii poleceń (po uruchomieniu wypisz adres węzła iroh bez adresu IP)
sync0 share dataset:./data

# Klient: przeciągnij całą grupę udostępniania zestawu danych do lokalnego pliku ./local_data
sync0 pull dataset:// ./local_data -p <adres węzła>

# Klient: pobieraj wiele zaznaczeń na żądanie (synchronizuj tylko określone pojedyncze pliki i podkatalogi)
sync0 pull dataset://docs/readme.md,assets/icons,src/core ./local_data -p <adres węzła>

# Klient: przesyłaj pliki lub katalogi do autoryzowanych grup udostępniania
sync0 push zestaw danych://releases/v1 ./build_output -p <adres węzła>
```

### 3. Rozpoczęcie subskrypcji w czasie rzeczywistym i dwukierunkowa synchronizacja na wielu komputerach (`sub` / `group`)

Jeśli po nawiązaniu długiego połączenia na zdalnym końcu wystąpi jakikolwiek plik **dodawanie, modyfikowanie, usuwanie, zmiana uprawnień lub czas modyfikacji**, katalog lokalny zostanie synchronicznie zaktualizowany w milisekundach w czasie rzeczywistym (nowe/zmodyfikowane duże pliki będą pobierać tylko bloki różnicowe i aktualizować je na miejscu, usuwanie plików automatycznie czyści pliki lokalne i puste katalogi i bezpośrednio zmienia tylko uprawnienia lub znaczniki czasu zerowych bloków danych):

- **`sync0 sub`(Jednokierunkowe tworzenie kopii lustrzanych w czasie rzeczywistym + automatyczne udostępnianie)**: Lokalny czas rzeczywisty śledzi dodawanie, usuwanie i modyfikacje strony współdzielonej i automatycznie służy jako źródło początkowe P2P, aby zapewnić równoczesne przyspieszenie dla innych węzłów ciągnących.
- **`sync0 group`(Dwukierunkowa synchronizacja w czasie rzeczywistym na wielu komputerach typu peer-to-peer)**: Jeśli pliki zostaną dodane, usunięte lub zmodyfikowane na dowolnym komputerze w grupie, wszystkie pozostałe węzły online będą aktualizowane synchronicznie w czasie rzeczywistym i będą służyć sobie nawzajem jako nasiona.

```bash
# Subskrybuj całą grupę udostępniania: zdalne dodawanie, usuwanie i modyfikacje są synchronizowane lokalnie w czasie rzeczywistym i automatycznie umieszczane
sync0 podzbiór danych:// ./local_mirror -p <adres węzła>

# Subskrypcja wielokrotnego wyboru na żądanie: synchronizuj i udostępniaj tylko określone pojedyncze pliki i podkatalogi w czasie rzeczywistym
sync0 subdataset://models/llama.onnx,configs ./local_mirror -p <adres węzła>

# Dwukierunkowa synchronizacja w czasie rzeczywistym w grupie równorzędnej na wielu komputerach (obsługuje również ścieżki wielokrotnego wyboru pełne lub przecinkowe)
grupa sync0 zespół-docs:// ./docs -p <adres węzła>
```

### 4. Konfiguracja uprawnień grupy i członka zespołu z wieloma udostępnianiami (konfiguracja `.nt`)

Domyślny katalog konfiguracyjny to `~/.config/sync0` (można go przełączyć za pomocą `-c <katalog>`), używając minimalistycznego formatu [NestedText](https://nestedtext.org) (`.nt`):
- **Obsługuje ograniczenie odczytu w sieci publicznej (czytać mogą tylko autoryzowani użytkownicy)**: Tak długo, jak grupa udostępniania jest skonfigurowana z listą członków `users:` (lub w wierszu poleceń określono `--allow-r` / `--allow-rw` / `--allow-w` / `--auth`), system **automatycznie włącza uwierzytelnianie prywatne (`auth: true`)** i zabrania anonimowego wyliczania węzłów sieci publicznej lub czytanie i subskrypcja P2P do węzłów inicjujących również odziedziczy jednocześnie białą listę, umożliwiając czytanie tylko autoryzowanym użytkownikom; grupy udostępniania, które nie są skonfigurowane z `users`, są domyślnie publiczne i tylko do odczytu (jeśli chcesz być publiczne i tylko do odczytu oraz otwarte na przesyłanie tylko dla określonych członków, możesz jawnie ustawić `auth: false`).
- **Dokładne uprawnienia do odczytu i zapisu oraz dostarczania do podkatalogów**: Członkowie są domyślnie tylko do odczytu (`r` nie musi być wpisany jawnie) i obsługują `r` (tylko do odczytu), `w` (tylko zapis do skrzynki wrzutowej, nieczytelny), `rw` (odczyt i zapis) oraz `write_prefix` (ograniczenie dozwolonego przedrostka podkatalogu do przesłania).

**Przykład konfiguracji elementu (`~/.config/sync0/users/`):**

```yaml
# ~/.config/sync0/users/alice.nt (domyślnie tylko do odczytu, ograniczenie prędkości 80MB)
id: 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c
rate: 80MB
```

```yaml
# ~/.config/sync0/users/bob.nt (uprawnienia do odczytu i zapisu, dozwolone tylko przesyłanie do określonego podkatalogu)
id: 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f
perm: rw
write_prefix:
  - uploads/bob
  - shared/common
```

```yaml
# ~/.config/sync0/users/charlie.nt (tylko uprawnienia do zapisu, czytanie jest zabronione)
id: 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
perm: w
```

**Przykład konfiguracji grupy udostępniania (`~/.config/sync0/config.nt`):**

```yaml
users_dir: ./users

groups:
  # Nieskonfigurowani użytkownicy: domyślna sieć publiczna tylko do odczytu
  dataset:
    path: ./data
    rate: 50MB

  # Skonfigurowani użytkownicy: automatycznie ograniczaj odczyt w sieci publicznej (tylko Alicja i Bob mogą czytać, Charlie może tylko dostarczać i przesyłać)
  team-vault:
    path: ./vault
    users:
      - alice
      - bob
      - charlie
```

Uruchom usługę (załaduj plik konfiguracyjny lub bezpośrednio użyj parametrów wiersza poleceń, aby ograniczyć odczyt sieci publicznej i autoryzować członków):

```bash
# Aby rozpocząć, załaduj plik ~/.config/sync0/config.nt
sync0 share --region SHA

# Lub opublikuj prywatną grupę udostępniania za pomocą samego wiersza poleceń (podaj --allow-*, aby automatycznie ograniczyć odczyt w sieci publicznej i mieć do niej dostęp tylko autoryzowani użytkownicy)
sync0 share team-vault:./vault \
  --allow-r 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c \
  --allow-rw 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f:uploads/bob,shared/common \
  --allow-w 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
```

### 5. Adres węzła, wizytówka i wielojęzyczność

```bash
# Wyświetl adres lokalnego węzła wolnego od adresu IP (klucz jest generowany automatycznie przy pierwszym uruchomieniu)
sync0 id

# Wygeneruj wizytówkę węzła z podpisem własnym Ed25519 (pseudonim, kod miasta, adres e-mail, strona internetowa, wprowadzenie)
sync0 profile -u alice --region SHA --email alice@example.com -b "Shanghai node"

# Istnieje 75 wbudowanych języków, które domyślnie podążają za językiem systemu i można je przełączać za pomocą zmiennych środowiskowych.
SYNC0_LANG=en sync0 --help
```

## Wydajność i podstawowe funkcje

### Rzeczywisty pomiar międzykontynentalnej sieci WAN (porównanie `rsync`)

Rzeczywiste środowisko pomiarowe: Stany Zjednoczone `c1` → Europa `n1` (RTT 65 ms), zestaw danych to jądro Linuksa `linux-7.3.3`:

#### 1) Tryb a la carte vs `rsync`

| Zbiór danych | rozmiar | `sync0` Pojedynczy punkt<br>(Mb/s) | `rsync`<br>(Mbps) | Przyspieszenie (`sync0` / `rsync`) |
| :--- | ---: | ---: | ---: | ---: |
| `linux-7.3.3.tar.xz`(nie kompresowalne) | 155 MB | **121.07** | 107.89 | **1,12×** |
| `linux-7.3.3.tar`(kompresowalny) | 1.6 GB | **442.07** | 506.62 | **0,87×** |
| `linux-7.3.3.tar`(przyrost zmiany lokalnej) | 1.6 GB | **8,123.87** | 1,169.49 | **6,95×** |
| `linux-7.3.3/` (95 979 małych plików) | 1.5 GB | **565.49** | 235.27 | **2,40×** |
| **średnia geometryczna** | — | **704.17** | 350.19 | **2,01×** |

#### 2) Tryb dwóch źródeł P2P vs `rsync`

| Zbiór danych | rozmiar | `sync0` Podwójne źródło P2P<br>(Mbps) | `rsync`<br>(Mbps) | Przyspieszenie (`sync0` / `rsync`) |
| :--- | ---: | ---: | ---: | ---: |
| `linux-7.3.3.tar.xz`(nie kompresowalne) | 155 MB | **466.85** | 107.89 | **4,33×** |
| `linux-7.3.3.tar`(kompresowalny) | 1.6 GB | **992.17** | 506.62 | **1,96×** |
| `linux-7.3.3.tar`(przyrost zmiany lokalnej) | 1.6 GB | **5,666.84** | 1,169.49 | **4,85×** |
| `linux-7.3.3/` (95 979 małych plików) | 1.5 GB | **703.30** | 235.27 | **2,99×** |
| **średnia geometryczna** | — | **1,165.63** | 350.19 | **3,33×** |

> Uwaga: w trybie podwójnego źródła P2P węzły europejskie `s1` współpracują przy inicjowaniu, a czas RTT międzykontynentalnej sieci szkieletowej wynosi 65 ms.

### Podstawowe cechy produktu

- **Automatyczne dziurkowanie adresowania klucza publicznego**: Nie ma potrzeby stosowania centralnego serwera i adresu sieci publicznej, automatyczne dziurkowanie NAT w oparciu o adres klucza publicznego sieci rozproszonej.
- **Synchronizacja w czasie rzeczywistym na poziomie milisekundowym i świadomość metadanych**: Na podstawie natywnego monitorowania zdarzeń w plikach (`FSEvents` / `inotify` / `ReadDirectoryChangesW`), dodawania i usuwania plików w czasie rzeczywistym, uprawnień POSIX (`mode`) i czasu modyfikacji na poziomie milisekund (`mtime`); transmisja zerowych bloków danych, gdy modyfikowane są tylko uprawnienia lub czas.
- **Jednoczesne pobieranie i wysyłanie w stylu BT oraz planowanie powinowactwa regionalnego**: Węzeł pobierania nie musi czekać na pobranie pełnej ilości. Wszystkie interwały bloków (`Op::Have`), które przeszły 128-bitową kontrolę skrótu, są natychmiast blokowane w pamięci/puli tymczasowej, aby inne węzły mogły je jednocześnie pobierać. Wraz z dynamiczną wymianą węzłów PEX i powolnym wywłaszczaniem połowy węzła (kradzież pracy) przepustowość klastra jest maksymalizowana.
- **Odporność na manipulowanie węzłami i zatruwanie klastrów przez złośliwe oprogramowanie**: autorytatywna lista źródeł `root_hash` Samospójne blokowanie + obowiązkowe przyspieszenie sprzętowe `gxhash128` weryfikacja podsumowania przed umieszczeniem każdego bloku na dysku i zaszczepieniem zewnętrznym; jeśli zostanie znaleziony naruszony blok, wyłącznik automatyczny działający na poziomie milisekundy wyłączy złośliwy węzeł i automatycznie odzyska sprawność z uczciwego źródła.
- **Szybka inkrementacja in-situ i agregacja małych plików poprzez FastCDC**: Tylko bloki różnicowe są przesyłane w celu częściowej modyfikacji dużych plików i umieszczane na dysku; ogromne pakowanie pamięci małych plików i kompresja różnicowa przedrostków eliminują narzut związany z przesyłaniem plików w obie strony plik po pliku.
- **Samonaprawa po przerwie w zasilaniu i wznowieniu po ponownym uruchomieniu**: Format hybrydowy RDB + AOF podobny do Redis (`.s0_subs.bin`) służy do utrwalania stanu początkowego i listy plików, codzienne zmiany są dołączane w mikrosekundach i są automatycznie przepisywane i regularnie organizowane. Po ponownym uruchomieniu serwera zmiany w trybie offline są automatycznie przekazywane do węzła inicjującego.

## Architektura techniczna i dokumentacja

- **Protokół odsprzęgania dwupłaszczyznowego**:
  - **Kontrola i powierzchnia zmian w czasie rzeczywistym (`iroh` QUIC / TCP, pierwszy bajt `PROTO_VERSION` uzgadnianie wersji) **: Odpowiedzialny za weryfikację wizytówek Ed25519, weryfikację uprawnień, wykrywanie nasion roju i obsługę wsadowego przesyłania strumieniowego `u64::MAX` poziomu masowych zmian i `mzstd` skompresowanego `SubPush` push w czasie rzeczywistym (`FsOp::Upsert` / `FsOp::Remove` / `FsOp::Meta` Trzy elementy pierwotne).
  - **Powierzchnia dystrybucji danych (natywny UDP + `compio` asynchroniczne we/wy) **: kompaktowy nagłówek 20B, okresowe sprzężenie zwrotne 18B TACK, hybrydowa kontrola przeciążenia BBRv3/Copa, odzyskiwanie utraconych pakietów RACK-TLP, adaptacyjne obejście kompresji Zstd w pierwszym bloku entropii Shannona i `gxhash128` kompleksowa weryfikacja bloków.
- **Dowiedz się więcej**:
  - [Raport z pomiaru wydajności i skrypt reprodukcji](https://github.com/webc-site/sync0/blob/main/doc/zh/benchmark.md)
  - [Projekt architektury systemu](https://github.com/webc-site/sync0/blob/main/doc/zh/design_v4.md)
  - [Protokół binarny i specyfikacja trwałości](https://github.com/webc-site/sync0/blob/main/doc/zh/proto.md)
  - [Uwierzytelnianie tożsamości, kontrola uprawnień i weryfikacja integralności](https://github.com/webc-site/sync0/blob/main/doc/zh/crypto.md)