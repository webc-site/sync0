# sync0

`sync0` je alat komandne linije za prijenos datoteka preko mreža, postepeno sinhroniziranje direktorija i istovremenu distribuciju podataka na više strojeva.

## Originalna namjera dizajna

U više-mašinskim sobama, zemljama ili okruženjima sa kašnjenjima na mreži i gubitkom paketa, alati kao što su `rsync` i `scp` se zasnivaju na potvrdi sekvence jedne veze, što ograničava korištenje propusnog opsega; kada više mašina povlači podatke sa istog servera u isto vreme, mrežna kartica izvornog servera može lako postati usko grlo.

Cilj `sync0` je riješiti dva problema:

1. Neka sinhronizacija fajlova i direktorijuma između dve mašine radi punu fizičku propusnost i prenesite promenjeni deo samo kada je fajl delimično izmenjen.
2. Više mašina može prenositi blokove podataka jedan drugom prilikom sinhronizacije istog direktorija, smanjujući opterećenje na izvornom serveru i skraćujući ukupno vrijeme sinhronizacije.

## Poređenje brzine sa rsync

Testno okruženje: evropski čvor $\leftrightarrow$ Sjevernoamerički čvor, kašnjenje povratnog puta 86 ms, mjerenje potpunog vremena uključujući skeniranje direktorija, poređenje deduplikacije, mrežni prijenos i verifikaciju postavljanja diska:

| Scenario prijenosa | Veličina podataka | `sync0` | `rsync` | višestruko |
| :--- | :---: | :---: | :---: | :---: |
| 10.000 malih fajlova | 77.6 MB | 0.90 s (688 Mbps) | 4.86 s (128 Mbps) | 5.4x |
| Kod i arhivski paketi (`.tar`) | 512 MB | 1.94 s (2111 Mbps) | 9.36 s (438 Mbps) | 4.8x |
| Video fajl (`.mp4`) | 512 MB | 7.36 s (556 Mbps) | 12.11 s (338 Mbps) | 1.6x |
| Inkrementalna sinhronizacija (izmijeniti 1% sadržaja) | 512 MB | 0.12 s | 1.85 s | 15.4x |

## Metode poboljšanja performansi

- **UDP istovremeni prijenos**: Šaljite blokove podataka putem UDP-a i ponovno prenosite samo izgubljene fragmente kako biste izbjegli da gubitak jednog paketa blokira naredne prijenose datoteka.
- **Segmentacija sadržaja i deduplikacija**: Podijelite datoteku u blokove podataka prema sadržaju i izračunajte vrijednost provjere. Isti sadržaj koji se već nalazi na prijemnoj strani ponovo se koristi direktno lokalno bez dupliciranja mreže.
- **Automatski prilagodite kompresiju prema brzini mreže**: Uključite kompresiju za kompresibilne sadržaje kao što su tekst i kod; automatski preskoči kompresiju za video zapise, slike, komprimirane pakete ili pod brzim LAN-om da smanji potrošnju CPU-a.
- **Povlačenje više čvorova u blizini**: Kada više mašina povuče isti direktorij, blokovi podataka će se istovremeno povlačiti iz preuzetih čvorova u istom gradu ili sa manjim kašnjenjem.

## Instaliraj

### Metoda 1: Instalirajte preko Cargo-a

```bash
cargo install sync0
```

### Metoda 2: Preuzmite unaprijed kompajlirane binarne datoteke

Idite na [GitHub Releases](https://github.com/webc-site/sync0/releases) da preuzmete `.tar.xz` komprimovani paket za odgovarajući sistem (podržava Linux x86_64/ARM64, macOS Apple Silicon/Intel, Windows x64/ARM64, Android ARM64/x86_64, FreeBSD, NetBSD). Nakon raspakivanja, stavite `sync0` u `PATH` direktorij:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Uputstvo za upotrebu

### 1. Direktan prijenos datoteka ili direktorija između dvije mašine

Započnite praćenje na prijemnoj mašini i odredite put primanja i čuvanja:

```bash
sync0 recv 0.0.0.0:9000 ./backup
```

Pošaljite jednu datoteku ili cijeli direktorij sa mašine za slanje:

```bash
sync0 send ./project 192.168.1.10:9000
```

Često korišteni parametri:

```bash
# Izbrišite suvišne datoteke koje postoje na prijemnoj strani, ali ne i na kraju slanju (zrcalna sinkronizacija)
sync0 send ./project 192.168.1.10:9000 --delete

# Ograničenje brzine slanja (jedinica: bajtova u sekundi, na primjer 50 MB/s)
sync0 send ./project 192.168.1.10:9000 --rate 50000000

# Tihi način rada (nije prikazana traka napretka)
sync0 send ./project 192.168.1.10:9000 -q
```

### 2. Objavljivanje zajedničkih direktorija i povlačenje iz više čvorova

Objavite zajednički direktorij (nazvan `dataset`) na izvornom serveru:

```bash
sync0 share dataset ./data --region SHA
```

Povucite dijeljeni direktorij na drugim strojevima (možete popuniti više adresa čvorova u isto vrijeme, odvojenih zarezima da biste ih povukli istovremeno):

```bash
sync0 pull 10.0.0.1:9000,10.0.0.2:9000 dataset ./local_data
```

Povucite samo određeni poddirektorij u dijeljenom direktoriju:

```bash
sync0 pull 10.0.0.1:9000 dataset ./local_data --subpath 2026/logs
```

Vratite lokalne promjene u udaljeni dijeljeni direktorij:

```bash
sync0 push 10.0.0.1:9000 dataset ./local_data
```

### 3. Pretplatite se na automatsko ažuriranje kataloga

Pretplatite se na zajednički direktorij izvornog čvora. Kada se datoteka doda ili izmijeni u izvornom direktoriju, automatski će se sinkronizirati s lokalnim računalom. U isto vrijeme, lokalna mašina će automatski služiti kao čvor za druge mašine za povlačenje:

```bash
sync0 sub 10.0.0.1:9000 dataset ./local_mirror
```

### 4. Dvosmjerna sinhronizacija grupa sa više mašina

Više mašina se pridružuje istom imenu grupe, a promjene u bilo kojem direktoriju čvorova će se automatski sinkronizirati s drugim čvorovima u grupi:

```bash
sync0 group team-docs ./docs 10.0.0.1:9000,10.0.0.2:9000
```

### 5. Identitet čvora i lična vizit karta

Svaki čvor podržava postavljanje korisničkog imena, grada, e-pošte, web stranice i personaliziranog potpisa (maksimalno 256 znakova) i prolazi autentifikaciju potpisa lokalnog privatnog ključa:

```bash
# Pogledajte ID lokalnog javnog ključa (navedite konfiguracijski direktorij kako biste zadržali ključ)
sync0 id -c ~/.config/sync0

# Generirajte i prikažite vizitkarte čvora s personaliziranim potpisima
sync0 profile -c ~/.config/sync0 \
  -u alice \
  --region SHA \
  --email alice@example.com \
  --url https://example.com \
  -b "Shanghai backup node"
```

`share`, `pull`, `sub`, `group` se također mogu pokrenuti sa ovim parametrima profila:

```bash
sync0 share dataset ./data -c ~/.config/sync0 -u alice --region SHA -b "Shanghai backup node"
```

### 6. Prebacivanje na više jezika

`sync0` Podržava 75 jezika. Podrazumevano, pomoć komandne linije i upiti na odgovarajućem jeziku se automatski prikazuju u skladu sa jezičkim okruženjem operativnog sistema; ako sistemski jezik nije na listi podržanih, engleski se koristi podrazumevano.

Da biste ručno odredili jezik, postavite varijablu okruženja `SYNC0_LANG`:

```bash
SYNC0_LANG=en sync0 --help
SYNC0_LANG=ja sync0 --help
SYNC0_LANG=de sync0 --help
```