# sync0

`sync0` faylları şəbəkələr arasında ötürmək, qovluqları tədricən sinxronlaşdırmaq və eyni zamanda məlumatları birdən çox maşına paylamaq üçün əmr xətti alətidir.

## Orijinal dizayn niyyəti

Maşınlar arası otaqlarda, ölkələrdə və ya şəbəkə gecikmələri və paket itkisi olan mühitlərdə `rsync` və `scp` kimi alətlər bant genişliyindən istifadəni məhdudlaşdıran tək qoşulma ardıcıllığının təsdiqinə əsaslanır; birdən çox maşın eyni serverdən məlumatları eyni anda çəkdikdə, mənbə server şəbəkə kartı asanlıqla darboğaza çevrilə bilər.

`sync0`-in məqsədi iki problemi həll etməkdir:

1. İki maşın arasında fayl və qovluq sinxronizasiyasının tam fiziki bant genişliyinə icazə verin və yalnız fayl qismən dəyişdirildikdə dəyişdirilmiş hissəni köçürün.
2. Bir neçə maşın eyni kataloqu sinxronlaşdırarkən məlumat bloklarını bir-birinə ötürə bilər, mənbə serverindəki yükü azaldır və ümumi sinxronizasiya vaxtını qısaldır.

## Rsync ilə sürətin müqayisəsi

Test mühiti: Avropa node $\leftrightarrow$ Şimali Amerika qovşağı, gediş-gəliş gecikməsi 86 ms, kataloqun skan edilməsi, təkmilləşdirmənin müqayisəsi, şəbəkə ötürülməsi və disk yerləşdirilməsinin yoxlanılması daxil olmaqla, tam vaxt aparan vaxtın ölçülməsi:

| Transmissiya ssenarisi | Məlumat ölçüsü | `sync0` | `rsync` | çoxsaylı |
| :--- | :---: | :---: | :---: | :---: |
| 10.000 kiçik fayl | 77.6 MB | 0.90 s (688 Mbps) | 4.86 s (128 Mbps) | 5.4x |
| Kod və arxiv paketləri (`.tar`) | 512 MB | 1.94 s (2111 Mbps) | 9.36 s (438 Mbps) | 4.8x |
| Video faylı (`.mp4`) | 512 MB | 7.36 s (556 Mbps) | 12.11 s (338 Mbps) | 1.6x |
| Artan sinxronizasiya (1% məzmunu dəyişdirin) | 512 MB | 0.12 s | 1.85 s | 15.4x |

## Performansın yaxşılaşdırılması üsulları

- **UDP paralel ötürülməsi**: UDP vasitəsilə məlumat bloklarını göndərin və sonrakı fayl köçürmələrini bloklayan bir paket itkisinin qarşısını almaq üçün yalnız itirilmiş fraqmentləri təkrar ötürün.
- **Məzmun seqmentasiyası və təkmilləşdirmə**: Faylı məzmuna görə məlumat bloklarına bölün və yoxlama dəyərini hesablayın. Artıq qəbuledici tərəfdə olan eyni məzmun şəbəkəni təkrarlamadan birbaşa yerli olaraq yenidən istifadə olunur.
- **Şəbəkə sürətinə uyğun olaraq sıxılmanı avtomatik tənzimləyin**: Mətn və kod kimi sıxıla bilən məzmun üçün sıxılmanı yandırın; CPU istehlakını azaltmaq üçün videolar, şəkillər, sıxılmış paketlər və ya yüksək sürətli LAN altında sıxılmanı avtomatik olaraq atlayın.
- **Yaxınlıqda çox qovşaq çəkmə**: Birdən çox maşın eyni kataloqu çəkdikdə, verilənlər blokları eyni şəhərdə və ya daha az gecikmə ilə endirilmiş qovşaqlardan eyni vaxtda çəkiləcək.

## Quraşdırın

### Metod 1: Yük vasitəsilə quraşdırın

```bash
cargo install sync0
```

### Metod 2: Əvvəlcədən tərtib edilmiş ikili faylları yükləyin

Müvafiq sistem (Linux x86_64/ARM64, macOS Apple Silicon/Intel, Windows x64/ARM64, Android ARM64/x86_64, FreeBSD, NetBSD dəstəkləyir) üçün `.tar.xz` sıxılmış paketi endirmək üçün [GitHub Releases](https://github.com/webc-site/sync0/releases) səhifəsinə keçin. Zipni açdıqdan sonra `sync0` qovluğunu `PATH` qovluğuna yerləşdirin:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## İstifadəçi təlimatı

### 1. Faylları və ya qovluqları iki maşın arasında birbaşa köçürün

Qəbul edən maşında monitorinqə başlayın və qəbul və saxlama yolunu təyin edin:

```bash
sync0 recv 0.0.0.0:9000 ./backup
```

Göndərən maşından tək bir fayl və ya bütöv bir kataloq göndərin:

```bash
sync0 send ./project 192.168.1.10:9000
```

Tez-tez istifadə olunan parametrlər:

```bash
# Göndərən tərəfdə olmayan, qəbuledici tərəfdə mövcud olan lazımsız faylları silin (güzgü sinxronizasiyası)
sync0 send ./project 192.168.1.10:9000 --delete

# Göndərmə sürətini məhdudlaşdırın (vahid: bayt/saniyə, məsələn 50 MB/s)
sync0 send ./project 192.168.1.10:9000 --rate 50000000

# Səssiz rejim (heç bir irəliləyiş çubuğu göstərilmir)
sync0 send ./project 192.168.1.10:9000 -q
```

### 2. Paylaşılan qovluqların nəşri və bir çox qovşaqdan çəkilməsi

Ortaq kataloqu (`dataset` adlı) mənbə serverində dərc edin:

```bash
sync0 share dataset ./data --region SHA
```

Paylaşılan kataloqu digər maşınlarda çəkin (eyni anda çəkmək üçün vergüllə ayrılmış birdən çox qovşaq ünvanını doldura bilərsiniz):

```bash
sync0 pull 10.0.0.1:9000,10.0.0.2:9000 dataset ./local_data
```

Paylaşılan kataloqda yalnız müəyyən bir alt kataloqu çəkin:

```bash
sync0 pull 10.0.0.1:9000 dataset ./local_data --subpath 2026/logs
```

Yerli dəyişiklikləri uzaq paylaşılan qovluğa geri itələyin:

```bash
sync0 push 10.0.0.1:9000 dataset ./local_data
```

### 3. Kataloqun avtomatik yeniləmələrinə abunə olun

Mənbə qovşağının paylaşılan kataloquna abunə olun. Mənbə kataloquna fayl əlavə edildikdə və ya dəyişdirildikdə, o, avtomatik olaraq yerli kompüterlə sinxronlaşdırılacaq. Eyni zamanda, yerli maşın avtomatik olaraq digər maşınların çəkməsi üçün bir qovşaq kimi xidmət edəcəkdir:

```bash
sync0 sub 10.0.0.1:9000 dataset ./local_mirror
```

### 4. Çox maşın qruplarının ikitərəfli sinxronizasiyası

Bir neçə maşın eyni qrup adına qoşulur və istənilən qovşaq kataloqunda dəyişikliklər avtomatik olaraq qrupdakı digər qovşaqlarla sinxronlaşdırılacaq:

```bash
sync0 group team-docs ./docs 10.0.0.1:9000,10.0.0.2:9000
```

### 5. Node şəxsiyyəti və şəxsi vizit kartı

Hər bir qovşaq istifadəçi adının, şəhərin, e-poçtun, vebsaytın və fərdiləşdirilmiş imzanın (maksimum 256 simvol) qurulmasını dəstəkləyir və yerli şəxsi açar imza autentifikasiyasını keçir:

```bash
# Yerli ictimai açar identifikatoruna baxın (açarı saxlamaq üçün konfiqurasiya qovluğunu göstərin)
sync0 id -c ~/.config/sync0

# Fərdi imzalarla node biznes kartları yaradın və göstərin
sync0 profile -c ~/.config/sync0 \
  -u alice \
  --region SHA \
  --email alice@example.com \
  --url https://example.com \
  -b "Shanghai backup node"
```

`share`, `pull`, `sub`, `group` də bu profil parametrləri ilə işə salına bilər:

```bash
sync0 share dataset ./data -c ~/.config/sync0 -u alice --region SHA -b "Shanghai backup node"
```

### 6. Çox dilli keçid

`sync0` 75 dili dəstəkləyir. Varsayılan olaraq, əməliyyat sisteminin dil mühitinə uyğun olaraq əmr satırı yardımı və müvafiq dildə göstərişlər avtomatik olaraq göstərilir; sistem dili dəstəklənən siyahıda deyilsə, standart olaraq ingilis dili istifadə olunur.

Dili əl ilə təyin etmək üçün mühit dəyişənini təyin edin `SYNC0_LANG`:

```bash
SYNC0_LANG=en sync0 --help
SYNC0_LANG=ja sync0 --help
SYNC0_LANG=de sync0 --help
```