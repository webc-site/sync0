# sync0

`sync0` este un instrument de linie de comandă pentru transferul de fișiere în rețele, sincronizarea progresivă a directoarelor și distribuirea concomitentă a datelor pe mai multe mașini.

## Intenția originală de design

În sălile între mașini, țări sau medii cu întârzieri de rețea și pierderi de pachete, instrumente precum `rsync` și `scp` se bazează pe confirmarea unei singure secvențe de conexiune, ceea ce limitează utilizarea lățimii de bandă; atunci când mai multe mașini extrag date de pe același server în același timp, placa de rețea a serverului sursă poate deveni cu ușurință un blocaj.

Scopul `sync0` este de a rezolva două probleme:

1. Lăsați sincronizarea fișierelor și a directoarelor dintre cele două mașini să ruleze lățimea de bandă fizică completă și transferați doar partea modificată atunci când fișierul este modificat parțial.
2. Mai multe mașini pot transfera blocuri de date între ele atunci când sincronizează același director, reducând sarcina pe serverul sursă și scurtând timpul general de sincronizare.

## Comparație de viteză cu rsync

Mediu de testare: nod european $\leftrightarrow$ nod nord-american, întârziere dus-întors 86 ms, măsurarea consumului complet de timp, inclusiv scanarea directoarelor, compararea deduplicarii, transmisia în rețea și verificarea plasării discului:

| Scenariul transmisiei | Dimensiunea datelor | `sync0` | `rsync` | multiplu |
| :--- | :---: | :---: | :---: | :---: |
| 10.000 de fișiere mici | 77.6 MB | 0.90 s (688 Mbps) | 4.86 s (128 Mbps) | 5.4x |
| Pachete de cod și arhivare (`.tar`) | 512 MB | 1.94 s (2111 Mbps) | 9.36 s (438 Mbps) | 4.8x |
| Fișier video (`.mp4`) | 512 MB | 7.36 s (556 Mbps) | 12.11 s (338 Mbps) | 1.6x |
| Sincronizare incrementală (modificare 1% conținut) | 512 MB | 0.12 s | 1.85 s | 15.4x |

## Metode de îmbunătățire a performanței

- **Transmisie simultană UDP**: Trimiteți blocuri de date prin UDP și retransmiteți numai fragmentele pierdute pentru a evita pierderea unui singur pachet care blochează transferurile ulterioare de fișiere.
- **Segmentarea și deduplicarea conținutului**: Împărțiți fișierul în blocuri de date în funcție de conținut și calculați valoarea de verificare. Același conținut aflat deja la capătul de recepție este reutilizat direct local, fără a duplica rețeaua.
- **Ajustați automat compresia în funcție de viteza rețelei**: activați compresia pentru conținutul compresibil, cum ar fi textul și codul; săriți automat compresia pentru videoclipuri, imagini, pachete comprimate sau în LAN de mare viteză pentru a reduce consumul CPU.
- **Atragere cu mai multe noduri din apropiere**: atunci când mai multe mașini trag același director, blocurile de date vor fi extrase simultan din nodurile descărcate din același oraș sau cu o latență mai mică.

## Instala

### Metoda 1: Instalați prin Cargo

```bash
cargo install sync0
```

### Metoda 2: Descărcați fișiere binare precompilate

Accesați [GitHub Releases](https://github.com/webc-site/sync0/releases) pentru a descărca pachetul `.tar.xz` comprimat pentru sistemul corespunzător (accepta Linux x86_64/ARM64, macOS Apple Silicon/Intel, Windows x64/ARM64, Android ARM64/x86_64, FreeBSD, NetBSD). După dezarhivare, plasați `sync0` în directorul `PATH`:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Ghidul utilizatorului

### 1. Transferați direct fișiere sau directoare între două mașini

Începeți monitorizarea pe mașina de recepție și specificați calea de primire și salvare:

```bash
sync0 recv 0.0.0.0:9000 ./backup
```

Trimiteți un singur fișier sau un director întreg de la mașina expeditoare:

```bash
sync0 send ./project 192.168.1.10:9000
```

Parametrii utilizați în mod obișnuit:

```bash
# Ștergeți fișierele redundante care există la capătul de recepție, dar nu la capătul de expediere (sincronizare în oglindă)
sync0 send ./project 192.168.1.10:9000 --delete

# Limitați rata de trimitere (unitate: octeți/secundă, de exemplu 50 MB/s)
sync0 send ./project 192.168.1.10:9000 --rate 50000000

# Mod silențios (nu este afișată bara de progres)
sync0 send ./project 192.168.1.10:9000 -q
```

### 2. Publicarea directoarelor partajate și extragerea de la mai multe noduri

Publicați un director partajat (numit `dataset`) pe serverul de origine:

```bash
sync0 share dataset ./data --region SHA
```

Trageți directorul partajat pe alte mașini (puteți completa mai multe adrese de noduri în același timp, separate prin virgule pentru a le extrage simultan):

```bash
sync0 pull 10.0.0.1:9000,10.0.0.2:9000 dataset ./local_data
```

Trageți doar un anumit subdirector din directorul partajat:

```bash
sync0 pull 10.0.0.1:9000 dataset ./local_data --subpath 2026/logs
```

Împingeți modificările locale înapoi în directorul partajat de la distanță:

```bash
sync0 push 10.0.0.1:9000 dataset ./local_data
```

### 3. Abonați-vă la actualizările automate ale catalogului

Abonați-vă la directorul partajat al nodului sursă. Când un fișier este adăugat sau modificat în directorul sursă, acesta va fi sincronizat automat cu computerul local. În același timp, mașina locală va servi automat ca nod pentru ca alte mașini să tragă:

```bash
sync0 sub 10.0.0.1:9000 dataset ./local_mirror
```

### 4. Sincronizare bidirecțională a grupurilor cu mai multe mașini

Mai multe mașini se alătură aceluiași nume de grup, iar modificările din orice director de noduri vor fi sincronizate automat cu alte noduri din grup:

```bash
sync0 group team-docs ./docs 10.0.0.1:9000,10.0.0.2:9000
```

### 5. Identitatea nodului și cartea de vizită personală

Fiecare nod acceptă setarea numelui de utilizator, orașului, e-mailului, site-ului web și a unei semnături personalizate (maximum 256 de caractere) și transmite autentificarea semnăturii cu cheia privată locală:

```bash
# Vizualizați ID-ul cheii publice locale (specificați directorul de configurare pentru a persista cheia)
sync0 id -c ~/.config/sync0

# Generați și afișați cărți de vizită nod cu semnături personalizate
sync0 profile -c ~/.config/sync0 \
  -u alice \
  --region SHA \
  --email alice@example.com \
  --url https://example.com \
  -b "Shanghai backup node"
```

`share`, `pull`, `sub`, `group` pot fi lansate și cu acești parametri de profil:

```bash
sync0 share dataset ./data -c ~/.config/sync0 -u alice --region SHA -b "Shanghai backup node"
```

### 6. Comutare în mai multe limbi

`sync0` Acceptă 75 de limbi. În mod implicit, ajutorul liniei de comandă și solicitările în limba corespunzătoare sunt afișate automat în funcție de mediul de limbă al sistemului de operare; dacă limba sistemului nu este în lista acceptată, engleza este utilizată implicit.

Pentru a specifica limba manual, setați variabila de mediu `SYNC0_LANG`:

```bash
SYNC0_LANG=en sync0 --help
SYNC0_LANG=ja sync0 --help
SYNC0_LANG=de sync0 --help
```