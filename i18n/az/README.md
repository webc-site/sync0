# sync0

`sync0` mərkəzləşdirilməmiş P2P fayl paylaşımı, real vaxt sinxronizasiyası və `rsync` əvəz etmək üçün istifadə edilən çoxmənbəli paylama alətidir - transkontinental WAN-ın ölçülmüş sürəti `rsync`-ün **5.89×** (bir nöqtəli həndəsi orta sürət **2.67×1) və həndəsi orta sürət **2.67×1-**, P2.P du×al **-ə qədərdir. transoceanic yüksək gecikmə əlaqəsi P2P sürətləndirilməsi **7,68×**-ə qədərdir (Tam keçid 24-ssenarili hərtərəfli həndəsi orta sürət **2,56×**).

Hər bir maşın həm server, həm də müştəridir: mərkəzi serverə və ya ictimai şəbəkə ünvanına ehtiyac yoxdur, avtomatik deşik açma paylanmış şəbəkənin ictimai açar ünvanına əsaslanır və o, iki maşın arasında birbaşa ötürməni, çoxlu paylaşma qruplarına icazə hostinqini, tələb üzrə alt kataloqların çoxlu seçimini, millisaniyə səviyyəli real vaxt artımlı çoxmənbəli sinxronizasiyanı və P2P konkurentini dəstəkləyir.

## Quraşdırın

### Metod 1: Yüklərin quraşdırılması

```bash
cargo install sync0
```

### Metod 2: Əvvəlcədən tərtib edilmiş ikili faylları yükləyin

Sistemə uyğun (Linux, macOS, Windows, Android, iOS, FreeBSD-ni dəstəkləyir) `.tar.xz` sıxılmış paketi endirmək üçün [GitHub Releases](https://github.com/webc-site/sync0/releases) səhifəsinə keçin, arxivdən çıxarın və `PATH`-ə qoyun:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Tez başlayın

### 1. İki maşın arasında sıfır konfiqurasiyalı birbaşa ötürmə (`send` / `recv`)

Paylaşım qrupunu konfiqurasiya etməyə, faylları və ya qovluqları iki maşın arasında yüksək sürətlə birbaşa ötürməyə ehtiyac yoxdur:

```bash
# Qəbul edən tərəf portu dinləyir və ./backup-a yazır
sync0 recv 0.0.0.0:9000 ./backup

# Göndərəndən birbaşa ötürmə (istəyə görə - artıq faylları təmizləmək üçün güzgü silin, - 50 MB sürət həddi)
sync0 send ./project 192.168.1.10:9000
```

### 2. Post paylaşımı və tələb üzrə çəkmə/yükləmə (`share` / `pull` / `push`)

```bash
# Server: Paylaşım qrupunu bir əmr xətti ilə dərc edin (başladıqdan sonra IP-siz iroh node ünvanını çıxarın)
sync0 share dataset:./data

# Müştəri: Bütün verilənlər toplusu paylaşma qrupunu yerli ./local_data-ya çəkin
sync0 pull dataset:// ./local_data -p <node ünvanı>

# Müştəri: Tələb üzrə çoxlu seçimləri çəkin (yalnız müəyyən edilmiş tək faylları və alt kataloqları sinxronlaşdırın)
sync0 pull dataset://docs/readme.md,assets/icons,src/core ./local_data -p <node ünvanı>

# Müştəri: Faylları və ya qovluqları səlahiyyətli paylaşma qruplarına yükləyin
sync0 push dataset://releases/v1 ./build_output -p <node ünvanı>
```

### 3. Real vaxtda abunə səpilməsi və çox maşınlı ikitərəfli sinxronizasiya (`sub` / `group`)

Uzun bir əlaqə qurduqdan sonra, uzaq uçda hər hansı bir fayl **əlavə etmək, dəyişdirmək, silmək, icazələrin dəyişdirilməsi və ya modifikasiya vaxtı** baş verərsə, yerli kataloq real vaxt rejimində millisaniyələrdə sinxron şəkildə yenilənəcək (yeni/dəyişdirilmiş böyük fayllar yalnız fərq bloklarını çəkəcək və onları yerində yeniləyəcək, faylları silmək yerli faylları avtomatik olaraq təmizləyəcək və yalnız bloklar üçün məlumat bloklarını avtomatik olaraq dəyişdirəcək),

- **`sync0 sub`(Birtərəfli real vaxt güzgüsü + avtomatik toxumlama)**: Yerli real vaxt paylaşma ucunun əlavələri, silinmələri və modifikasiyalarını izləyir və avtomatik olaraq digər çəkmə qovşaqları üçün paralel sürətləndirməni təmin etmək üçün P2P toxum mənbəyi kimi xidmət edir.
- **`sync0 group`(Çox maşınlı peer-to-peer ikitərəfli real vaxt sinxronizasiyası)**: Qrupdakı hər hansı maşında fayllar əlavə edilərsə, silinərsə və ya dəyişdirilərsə, bütün digər onlayn qovşaqlar real vaxt rejimində sinxron şəkildə yenilənəcək və bir-biri üçün toxum rolunu oynayacaq.

```bash
# Bütün paylaşma qrupuna abunə olun: uzaqdan əlavələr, silmələr və dəyişikliklər yerli olaraq real vaxtda sinxronlaşdırılır və avtomatik olaraq səpilir.
sync0 sub dataset:// ./local_mirror -p <node ünvanı>

# Tələb üzrə çox seçimli abunə: real vaxtda yalnız müəyyən edilmiş tək faylları və alt kataloqları sinxronlaşdırın və toxumlayın
sync0 alt verilənlər dəsti://models/llama.onnx,configs ./local_mirror -p <node ünvanı>

# Çox maşınlı həmyaşıd qrupu ikitərəfli real vaxt sinxronizasiyası (həmçinin tam və ya vergüllə çox seçimli yolları dəstəkləyir)
sync0 group team-docs:// ./docs -p <node ünvanı>
```

### 4. Komandanın çoxlu paylaşım qrupu və üzv icazəsi konfiqurasiyası (`.nt` konfiqurasiyası)

Defolt konfiqurasiya kataloqu minimalist [NestedText](https://nestedtext.org) (`.nt`) formatından istifadə etməklə `~/.config/sync0` (`-c <directory>` vasitəsilə dəyişdirilə bilər):
- **İctimai şəbəkənin oxunmasını məhdudlaşdıran dəstəyi dəstəkləyir (yalnız səlahiyyətli istifadəçilər oxuya bilər)**: Paylaşım qrupu `users:` üzv siyahısı ilə konfiqurasiya edildiyi müddətcə (və ya komanda xətti `--allow-r` / `--allow-rw` / `--allow-w` / `--auth` təyin edir), sistem **özəl şəkildə aktivləşir (atomatik olaraq **"6") və ictimai şəbəkənin anonim qovşağının sadalanması və ya oxunmasını qadağan edir və P2P Toxum qovşaqlarına abunə olmaq eyni vaxtda ağ siyahını miras alacaq və yalnız səlahiyyətli istifadəçilərə oxumağa imkan verəcək; `users` ilə konfiqurasiya olunmayan paylaşma qrupları defolt olaraq ictimaidir və yalnız oxunur (əgər siz ictimai olmaq və yalnız oxunmaq və yalnız müəyyən üzvlər üçün yükləmələrə açıq olmaq istəyirsinizsə, açıq şəkildə `auth: false` təyin edə bilərsiniz).
- **İncə dənəli oxu-yazma və alt kataloq çatdırılması icazələri**: Üzvlər defolt olaraq yalnız oxunur (`r` açıq şəkildə yazılmalıdır) və `r` (yalnız oxumaq üçün), `w` (yalnız açılan qutuya yazın, oxuna bilməz), `write_prefix` və `write_prefix` dəstəkləyir. (yükləməyə icazə verilən alt kataloq prefiksini məhdudlaşdırın).

**Üzv konfiqurasiyası nümunəsi (`~/.config/sync0/users/`):**

```yaml
# ~/.config/sync0/users/alice.nt (defolt yalnız oxumaq üçün, sürət həddi 80MB)
id: 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c
rate: 80MB
```

```yaml
# ~/.config/sync0/users/bob.nt (oxumaq və yazmaq icazələri, yalnız müəyyən edilmiş alt kataloqa yükləməyə icazə verilir)
id: 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f
perm: rw
write_prefix:
  - uploads/bob
  - shared/common
```

```yaml
# ~/.config/sync0/users/charlie.nt (yalnız çatdırılma icazəsini yazın, oxumaq qadağandır)
id: 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
perm: w
```

**Paylaşılan qrup konfiqurasiyası nümunəsi (`~/.config/sync0/config.nt`):**

```yaml
users_dir: ./users

groups:
  # Konfiqurasiya edilməmiş istifadəçilər: defolt ictimai şəbəkə yalnız oxunur
  dataset:
    path: ./data
    rate: 50MB

  # Konfiqurasiya edilmiş istifadəçilər: ictimai şəbəkə oxumağı avtomatik məhdudlaşdırın (yalnız Alice və Bob oxuya bilər, Charlie yalnız çatdıra və yükləyə bilər)
  team-vault:
    path: ./vault
    users:
      - alice
      - bob
      - charlie
```

Xidməti işə salın (konfiqurasiya faylını yükləyin və ya ictimai şəbəkənin oxunmasını məhdudlaşdırmaq və üzvlərə icazə vermək üçün birbaşa komanda xətti parametrlərindən istifadə edin):

```bash
# Başlamaq üçün ~/.config/sync0/config.nt yükləyin
sync0 share --region SHA

# Və ya təmiz komanda xətti ilə şəxsi paylaşma qrupunu dərc edin (ictimai şəbəkənin oxunmasını avtomatik məhdudlaşdırmaq üçün --allow-* işarələyin və yalnız səlahiyyətli istifadəçilər ona daxil ola bilər)
sync0 share team-vault:./vault \
  --allow-r 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c \
  --allow-rw 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f:uploads/bob,shared/common \
  --allow-w 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
```

### 5. Node ünvanı, vizit kartı və çoxdilli

```bash
# Yerli IP-siz qovşaq ünvanına baxın (açar avtomatik olaraq ilk işə salınır)
sync0 id

# Ed25519 öz imzalı node vizit kartı yaradın (ləqəb, şəhər kodu, e-poçt, vebsayt, giriş)
sync0 profile -u alice --region SHA --email alice@example.com -b "Shanghai node"

# Defolt olaraq sistem dilini izləyən və mühit dəyişənləri vasitəsilə dəyişdirilə bilən 75 daxili dil var.
SYNC0_LANG=en sync0 --help
```

## Performans və əsas xüsusiyyətlər

### Qitələrarası WAN faktiki ölçülməsi (Müqayisə `rsync`)

Faktiki ölçmə mühiti: Amerika Birləşmiş Ştatları `c1` → Avropa `n1` (RTT 65 ms), məlumat dəsti Linux nüvəsidir `linux-7.3.3`:

#### 1) A la carte rejimi vs `rsync`

| Məlumat dəsti | ölçüsü | `sync0` Tək Nöqtə<br>(Mbps) | `rsync`<br>(Mbps) | Sürətləndirmə (`sync0` / `rsync`) |
| :--- | ---: | ---: | ---: | ---: |
| `linux-7.3.3.tar.xz`(sıxıla bilməz) | 155 MB | **236.48** | 108.74 | **2,17×** |
| `linux-7.3.3.tar`(sıxıla bilən) | 1.6 GB | **843.83** | 385.06 | **2,19×** |
| `linux-7.3.3.tar`(yerli dəyişiklik artımı) | 1.6 GB | **8,014.15** | 1,361.72 | **5,89×** |
| `linux-7.3.3/` (95,979 kiçik fayl) | 1.5 GB | **627.58** | 347.80 | **1,80×** |
| **həndəsi orta** | — | **1,000.91** | 375.26 | **2,67×** |

#### 2) P2P ikili mənbə rejimi və `rsync`

| Məlumat dəsti | ölçüsü | `sync0` P2P İkili Mənbə<br>(Mbps) | `rsync`<br>(Mbps) | Sürətləndirmə (`sync0` / `rsync`) |
| :--- | ---: | ---: | ---: | ---: |
| `linux-7.3.3.tar.xz`(sıxıla bilməz) | 155 MB | **330.09** | 108.74 | **3,04×** |
| `linux-7.3.3.tar`(sıxıla bilən) | 1.6 GB | **1,551.86** | 385.06 | **4.03×** |
| `linux-7.3.3.tar`(yerli dəyişiklik artımı) | 1.6 GB | **5,474.17** | 1,361.72 | **4.02×** |
| `linux-7.3.3/` (95,979 kiçik fayl) | 1.5 GB | **706.56** | 347.80 | **2.03×** |
| **həndəsi orta** | — | **1,186.42** | 375.26 | **3,16×** |

> Qeyd: P2P ikili mənbə rejimində Avropa qovşaqları `s1` əkmək üçün əməkdaşlıq edir və qitələrarası magistral RTT 65 ms-dir.

### Əsas məhsul xüsusiyyətləri

- **Avtomatik deşiklərin açılmasına yönələn açıq açar**: Mərkəzi serverə və ictimai şəbəkə ünvanına ehtiyac yoxdur, paylanmış şəbəkənin ictimai açar ünvanına əsaslanan avtomatik NAT deşikləri.
- **Millisaniyə səviyyəli real vaxt sinxronizasiyası və metadata məlumatlılığı**: Doğma fayl hadisəsi monitorinqi (`FSEvents` / `inotify` / `ReadDirectoryChangesW`), fayl əlavələri, silinmələr, POSIX icazələri (`mode`) və millisaniyəlik modifikasiya vaxtı (`mode`) ilə real vaxtda təkan (<b id="">"<b) əsasında); yalnız icazələr və ya vaxt dəyişdirildikdə məlumat blokunun ötürülməsi sıfırdır.
- **BT tipli eyni vaxtda yükləmə və yükləmə və regional yaxınlıq planlaması**: Yükləmə qovşağının tam məbləğin endirilməsini gözləməsi lazım deyil. 128 bitlik hash yoxlamasından keçmiş bütün blok intervalları (`Op::Have`) digər qovşaqların eyni vaxtda çəkilməsi üçün dərhal yaddaşda/müvəqqəti hovuzda kilidlənir. Dinamik PEX node mübadiləsi və yavaş node yarım preemption (İş-oğurlama) ilə birlikdə klaster ötürmə qabiliyyəti maksimuma çatdırılır.
- **Zərərli qovşaqların dəyişdirilməsi və klasterin zəhərlənməsi toxunulmazlığı**: nüfuzlu mənbə siyahısı `root_hash` Öz-özünə ardıcıl kilidləmə + məcburi aparat sürətləndirilməsi `gxhash128` hər blok diskə yerləşdirilmədən və xaricdən toxumlanmadan əvvəl həzm yoxlaması; dəyişdirilmiş blok aşkar edilərsə, millisaniyə səviyyəli elektrik açarı zərərli qovşağı söndürəcək və dürüst mənbədən avtomatik bərpa olunacaq.
- **FastCDC in-situ artımı və kiçik faylların yığılması**: Böyük faylların qismən dəyişdirilməsi üçün yalnız diferensial bloklar köçürülür və diskə yerləşdirilir; kütləvi kiçik fayl yaddaş qablaşdırma və prefiks diferensial sıxılma fayl-fayl gediş yükü aradan qaldırır.
- **Enerji kəsildikdən sonra özünü sağalma və yenidən işə saldıqdan sonra bərpa**: Redis kimi RDB + AOF hibrid formatı (`.s0_subs.bin`) əkmə statusunu və fayl siyahısını saxlamaq üçün istifadə olunur, gündəlik dəyişikliklər mikrosaniyələrlə əlavə olunur və avtomatik olaraq yenidən yazılır və müntəzəm olaraq təşkil edilir. Server yenidən işə salındıqdan sonra oflayn dəyişikliklər avtomatik olaraq toxum qovşağına ötürülür.

## Texniki memarlıq və sənədlər

- **İkili təyyarə ayırma protokolu**:
  - **Nəzarət və real vaxt dəyişikliyi səthi (`iroh` QUIC / TCP, ilk bayt `PROTO_VERSION` versiyanın əl sıxması) **: Ed25519 vizit kartının yoxlanılması, icazənin yoxlanılması, Swarm toxumu kəşfi və `u64::MAX` səviyyəli kütləvi dəyişiklik axınının id="-" sıxılmış id="> <b"> `FsOp::Upsert` / `FsOp::Remove` / `FsOp::Meta` Üç primitiv).
  - **Məlumatların paylanması səthi (doğma UDP + `compio` asinxron I/O) **: 20B kompakt başlıq, 18B TACK dövri rəy, BBRv3/Copa hibrid tıxac nəzarəti, RACK-TLP paket itkisinin bərpası, birinci blok Shannon entropiya adaptiv Zstassp="-"> və sıxılma blokunun sonu "-" yoxlama.
- **Ətraflı məlumat**:
  - [Performansın ölçülməsi hesabatı və reproduksiya skripti](https://github.com/webc-site/sync0/blob/main/doc/zh/benchmark.md)
  - [Sistem arxitekturasının dizaynı](https://github.com/webc-site/sync0/blob/main/doc/zh/design_v4.md)
  - [Binary Wire Protocol və Persistence Specification](https://github.com/webc-site/sync0/blob/main/doc/zh/proto.md)
  - [Şəxsiyyətin identifikasiyası, səlahiyyətlərə nəzarət və bütövlüyün yoxlanılması](https://github.com/webc-site/sync0/blob/main/doc/zh/crypto.md)