# sync0

`sync0` je alat naredbenog retka za prijenos datoteka preko mreža, postupnu sinkronizaciju direktorija i istovremenu distribuciju podataka na više strojeva.

## Izvorna namjera dizajna

U sobama s više strojeva, zemljama ili okruženjima s mrežnim kašnjenjima i gubitkom paketa, alati kao što su `rsync` i `scp` temelje se na potvrdi slijeda jedne veze, što ograničava korištenje propusnosti; kada više strojeva povlači podatke s istog poslužitelja u isto vrijeme, mrežna kartica izvornog poslužitelja može lako postati usko grlo.

Cilj `sync0` je riješiti dva problema:

1. Neka sinkronizacija datoteka i direktorija između dva računala radi punom fizičkom propusnošću i prenesite samo promijenjeni dio kada je datoteka djelomično izmijenjena.
2. Više strojeva može međusobno prenositi blokove podataka prilikom sinkronizacije istog direktorija, smanjujući opterećenje izvornog poslužitelja i skraćujući ukupno vrijeme sinkronizacije.

## Usporedba brzine s rsync

Testno okruženje: europski čvor $\leftrightarrow$ sjevernoamerički čvor, povratno kašnjenje 86 ms, mjerenje cjelokupnog vremena uključujući skeniranje direktorija, usporedbu deduplikacije, mrežni prijenos i provjeru postavljanja diska:

| Scenarij prijenosa | Veličina podataka | `sync0` | `rsync` | višestruki |
| :--- | :---: | :---: | :---: | :---: |
| 10 000 malih datoteka | 77.6 MB | 0.90 s (688 Mbps) | 4.86 s (128 Mbps) | 5.4x |
| Paketi koda i arhive (`.tar`) | 512 MB | 1.94 s (2111 Mbps) | 9.36 s (438 Mbps) | 4.8x |
| Video datoteka (`.mp4`) | 512 MB | 7.36 s (556 Mbps) | 12.11 s (338 Mbps) | 1.6x |
| Inkrementalna sinkronizacija (izmijenite 1% sadržaja) | 512 MB | 0.12 s | 1.85 s | 15.4x |

## Metode poboljšanja performansi

- **UDP istodobni prijenos**: Pošaljite blokove podataka putem UDP-a i ponovno pošaljite samo izgubljene fragmente kako biste izbjegli gubitak jednog paketa koji blokira sljedeće prijenose datoteka.
- **Segmentacija sadržaja i deduplikacija**: Podijelite datoteku u podatkovne blokove prema sadržaju i izračunajte kontrolnu vrijednost. Isti sadržaj koji je već na prijemnoj strani ponovno se koristi izravno lokalno bez dupliciranja mreže.
- **Automatski prilagodite kompresiju prema brzini mreže**: uključite kompresiju za sadržaj koji se može komprimirati kao što su tekst i kod; automatski preskoči kompresiju za video zapise, slike, komprimirane pakete ili pod brzom LAN-om kako bi se smanjila potrošnja procesora.
- **Vučenje više čvorova u blizini**: kada više strojeva povuče isti direktorij, podatkovni blokovi će se istovremeno povući iz preuzetih čvorova u istom gradu ili uz manju latenciju.

## Instalirati

### Metoda 1: Instalirajte putem Carga

```bash
cargo install sync0
```

### Metoda 2: preuzmite unaprijed kompajlirane binarne datoteke

Idite na [GitHub Releases](https://github.com/webc-site/sync0/releases) da preuzmete `.tar.xz` komprimirani paket za odgovarajući sustav (podržava Linux x86_64/ARM64, macOS Apple Silicon/Intel, Windows x64/ARM64, Android ARM64/x86_64, FreeBSD, NetBSD). Nakon raspakiranja, smjestite `sync0` u direktorij `PATH`:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Korisnički priručnik

### 1. Izravan prijenos datoteka ili direktorija između dva računala

Pokrenite nadzor na prijemnom stroju i odredite put primanja i spremanja:

```bash
sync0 recv 0.0.0.0:9000 ./backup
```

Pošaljite jednu datoteku ili cijeli direktorij sa stroja koji šalje:

```bash
sync0 send ./project 192.168.1.10:9000
```

Često korišteni parametri:

```bash
# Izbrišite suvišne datoteke koje postoje na strani primatelja, ali ne i na strani pošiljatelja (zrcalna sinkronizacija)
sync0 send ./project 192.168.1.10:9000 --delete

# Ograničite brzinu slanja (jedinica: bajt/sekunda, na primjer 50 MB/s)
sync0 send ./project 192.168.1.10:9000 --rate 50000000

# Tihi način rada (ne prikazuje se traka napretka)
sync0 send ./project 192.168.1.10:9000 -q
```

### 2. Objavljivanje zajedničkih direktorija i povlačenje iz više čvorova

Objavite dijeljeni direktorij (nazvan `dataset`) na izvornom poslužitelju:

```bash
sync0 share dataset ./data --region SHA
```

Povucite dijeljeni direktorij na drugim strojevima (možete ispuniti više adresa čvorova u isto vrijeme, odvojenih zarezima za istodobno povlačenje):

```bash
sync0 pull 10.0.0.1:9000,10.0.0.2:9000 dataset ./local_data
```

Povucite samo određeni poddirektorij u zajedničkom direktoriju:

```bash
sync0 pull 10.0.0.1:9000 dataset ./local_data --subpath 2026/logs
```

Gurnite lokalne promjene natrag u udaljeni dijeljeni direktorij:

```bash
sync0 push 10.0.0.1:9000 dataset ./local_data
```

### 3. Pretplatite se na automatska ažuriranja kataloga

Pretplatite se na zajednički direktorij izvornog čvora. Kada se datoteka doda ili izmijeni u izvornom direktoriju, automatski će se sinkronizirati s lokalnim računalom. U isto vrijeme, lokalni stroj će automatski služiti kao čvor za druge strojeve koji mogu povlačiti:

```bash
sync0 sub 10.0.0.1:9000 dataset ./local_mirror
```

### 4. Dvosmjerna sinkronizacija višestrojnih grupa

Više strojeva pridružuje se istom nazivu grupe, a promjene u bilo kojem direktoriju čvorova automatski će se sinkronizirati s drugim čvorovima u grupi:

```bash
sync0 group team-docs ./docs 10.0.0.1:9000,10.0.0.2:9000
```

### 5. Identitet čvora i osobna posjetnica

Svaki čvor podržava postavljanje korisničkog imena, grada, e-pošte, web stranice i personaliziranog potpisa (maksimalno 256 znakova), te prolazi provjeru autentičnosti potpisa lokalnog privatnog ključa:

```bash
# Prikaz ID-a lokalnog javnog ključa (odredite konfiguracijski direktorij da biste zadržali ključ)
sync0 id -c ~/.config/sync0

# Generirajte i prikažite posjetnice čvorova s ​​personaliziranim potpisima
sync0 profile -c ~/.config/sync0 \
  -u alice \
  --region SHA \
  --email alice@example.com \
  --url https://example.com \
  -b "Shanghai backup node"
```

`share`, `pull`, `sub`, `group` također se mogu pokrenuti s ovim parametrima profila:

```bash
sync0 share dataset ./data -c ~/.config/sync0 -u alice --region SHA -b "Shanghai backup node"
```

### 6. Prebacivanje na više jezika

`sync0` Podržava 75 jezika. Prema zadanim postavkama pomoć naredbenog retka i upiti na odgovarajućem jeziku automatski se prikazuju u skladu s jezičnim okruženjem operativnog sustava; ako jezik sustava nije na popisu podržanih, engleski se koristi prema zadanim postavkama.

Da biste ručno odredili jezik, postavite varijablu okruženja `SYNC0_LANG`:

```bash
SYNC0_LANG=en sync0 --help
SYNC0_LANG=ja sync0 --help
SYNC0_LANG=de sync0 --help
```