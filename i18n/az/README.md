# sync0

`sync0` Resilio Sync ilə rəqabət aparan həmyaşıd (P2P) fayl paylaşımı və çox maşınlı sinxronizasiya alətidir.

Hər bir maşın həm server, həm də müştəridir: istənilən qovşaq çoxlu fayl paylaşma qruplarını dərc edə bilər. Digər qovşaqlar qoşulduqdan sonra onlar avtomatik olaraq həmyaşıd serverdən mövcud toxum qovşaqlarını əldə edə və eyni vaxtda bir neçə mənbədən çəkə bilərlər. Sinxronizasiya tamamlandıqdan sonra yükləmələri sürətləndirmək üçün daha çox qovşaq üçün toxumlar avtomatik olaraq yaradılacaq.

## Əsas məqamlar

- **Hər kəs həm server, həm də müştəridir**: Resilio Sync ilə müqayisə oluna bilər, mərkəzi serverdən asılılıq yoxdur. İstənilən qovşaq çoxlu fayl paylaşma qrupları yarada, həmçinin digər insanların paylaşma qruplarına qoşula bilər; qovşaqlar birdən çox maşının eyni vaxtda çəkilməsi və real vaxt sinxronizasiyasına nail olmaq üçün mövcud toxum qovşaqlarını avtomatik olaraq dəyişdirir və əldə edir.
- **IP-siz node ünvanı**: Defolt olaraq, qovşaq açarı və `base64url` kodlanmış `iroh` ünvanı ictimai IP-ni yadda saxlamağa və ya ifşa etməyə ehtiyac olmadan avtomatik olaraq `~/.config/sync0`-də yaradılır.
- **Tələb üzrə çox seçimli sinxronizasiya**: Bütün paylaşma qrupunun sinxronizasiyasını dəstəkləyir və eyni zamanda birdən çox fayl və ya alt kataloqu seçərək tələb üzrə sinxronizasiyanı dəstəkləyir.
- **Qrup üzvlərinin iyerarxik icazələri və yükləmə qovluğuna nəzarət**: Yalnız oxumaq üçün (`ro`), oxumaq-yazmaq yükləmək (`rw`) və idarəetmə (`admin`) icazələrini dəstəkləyir və hər bir üzvün yükləməsinə icazə verilən yol prefiksini fərdi olaraq təyin edə bilər (defolt olaraq, paylaşımın kök rejissora yüklənməsinə icazə verilir).
- **Şəbəkələr arasında tam sürətli və artımlı sinxronizasiya**: Böyük faylların eyni vaxtda ötürülməsi, yaddaşda kütləvi kiçik faylların birləşdirilməsi və fayllar qismən dəyişdirildikdə yalnız sinxronlaşdırılmış məlumat blokları.

## Rsync ilə sürətin müqayisəsi

### Transoceanic şəbəkə tək mənbə faktiki ölçmə (Çin → ABŞ, gecikmə 260 ms)

| Transmissiya ssenarisi | Məlumat ölçüsü | model | `sync0` | `rsync -az` | Multiplikatoru artırın |
| :--- | :---: | :---: | :---: | :---: | :---: |
| 500 kiçik fayl | 1.91 MB | Tam sinxronizasiya | 18.84 Mbps | 5.32 Mbps | 3.5x |
| 500 kiçik fayl | 1.91 MB | Artan sinxronizasiya | 27.15 Mbps | 3.83 Mbps | 7.1x |
| Kod və arxiv paketləri (`.tar`) | 10.0 MB | Tam sinxronizasiya | 139.70 Mbps | 24.97 Mbps | 5.6x |
| Kod və arxiv paketləri (`.tar`) | 10.0 MB | Artan sinxronizasiya | 83.38 Mbps | 26.02 Mbps | 3.2x |
| Video faylı (`.mp4`) | 15.0 MB | Tam sinxronizasiya | 54.06 Mbps | 27.04 Mbps | 2.0x |
| Video faylı (`.mp4`) | 15.0 MB | Artan sinxronizasiya | 216.83 Mbps | 21.51 Mbps | 10.1x |

### Transoceanic şəbəkə P2P çox node çəkmə testi (Çin → ABŞ ikili qovşaq, gecikmə 260 ms)

Çox mənbəli toxum node zolaqlı paralel çəkmənin ölçülən müqayisəsi:

| Transmissiya ssenarisi | Məlumat ölçüsü | Tək düyün toxumu | İki qovşaqlı P2P paralel səpmə | Multiplikatoru artırın |
| :--- | :---: | :---: | :---: | :---: |
| 500 kiçik fayl | 1.97 MB | 5.28 Mbps | 7.87 Mbps | 1.5x |
| Kod və arxiv paketləri (`.tar`) | 30.0 MB | 56.35 Mbps | 83.17 Mbps | 1.5x |
| Video faylı (`.mp4`) | 30.0 MB | 27.00 Mbps | 39.46 Mbps | 1.5x |

### Transkontinental yüksək sürətli xüsusi xəttin faktiki ölçülməsi (Avropa → Şimali Amerika, gecikmə 86 ms)

| Transmissiya ssenarisi | Məlumat ölçüsü | `sync0` | `rsync` | Multiplikatoru artırın |
| :--- | :---: | :---: | :---: | :---: |
| 10.000 kiçik fayl | 77.6 MB | 688 Mbps | 128 Mbps | 5.4x |
| Kod və arxiv paketləri (`.tar`) | 512 MB | 2111 Mbps | 438 Mbps | 4.8x |
| Video faylı (`.mp4`) | 512 MB | 556 Mbps | 338 Mbps | 1.6x |
| Artan sinxronizasiya (1% məzmunu dəyişdirin) | 512 MB | 34133 Mbps | 2214 Mbps | 15.4x |

## Quraşdırın

### Metod 1: Yük vasitəsilə quraşdırın

```bash
cargo install sync0
```

### Metod 2: Əvvəlcədən tərtib edilmiş ikili faylları yükləyin

Müvafiq sistem üçün (Linux, macOS, Windows, Android, FreeBSD, NetBSD-ni dəstəkləyir) `.tar.xz` sıxılmış paketi endirmək üçün [GitHub Releases](https://github.com/webc-site/sync0/releases) səhifəsinə keçin, arxivi açın və `PATH` kataloquna qoyun:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Tez başlayın

`sync0` defolt olaraq konfiqurasiya kataloqu kimi `~/.config/sync0` istifadə edir (digər kataloqlara keçmək lazımdırsa, siz `-c <directory>` əlavə edə bilərsiniz).

### 1. Yerli node ünvanına və vizit kartına baxın

```bash
# Yerli qovşaq ünvanına baxın (açar ilk dəfə ~/.config/sync0-da avtomatik olaraq yaradılır)
sync0 id

# Şəxsi node vizit kartı yaradın (istifadəçi adı, şəhər, e-poçt, vebsayt və 256 söz ərzində imzanı dəstəkləyir)
sync0 profile -u alice --region SHA --email alice@example.com --url https://example.com -b "Shanghai node"
```

### 2. Çoxsaylı fayl paylaşım qruplarını dərc edin və qrup üzvü icazələri təyin edin (`.nt` Konfiqurasiya)

`sync0` [NestedText](https://nestedtext.org) (`.nt`) formatında konfiqurasiya edilib. Hər bir istifadəçinin ayrıca `.nt` konfiqurasiya faylı var. Siz istifadəçi profili kataloqunu `users_dir` vasitəsilə `config.nt` (avtomatik olaraq bu kataloqdakı bütün `.nt` faylları ehtiva edir) vasitəsilə təyin edə bilərsiniz. Həm istifadəçi, həm də yüklənməsinə icazə verilən yol prefiksi sıralanmamış siyahıdan istifadə edir (`-`). **Yükləmə yolu prefiksi doldurulmadıqda, defolt olaraq paylaşımın kök kataloquna yükləməyə icazə verilir**.

1. `~/.config/sync0/users/` kataloqunda hər bir üzv üçün ayrıca `.nt` konfiqurasiya faylları yaradın:

`~/.config/sync0/users/alice.nt` (yalnız oxumaq üçün üzv, tarif limiti `80MB`, `rate` `GB`, `MB`, `KB` vahidləri dəstəkləyir):

```yaml
id: 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c
perm: ro
rate: 80MB
```

`~/.config/sync0/users/bob.nt` (məhdud yükləmə üzvləri: yalnız `uploads/bob` və `shared/common` kataloqlarına yükləməyə icazə verilir):

```yaml
id: 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f
perm: rw
write_prefix:
  - uploads/bob
  - shared/common
```

`~/.config/sync0/users/charlie.nt` (Defolt üzv yükləməsi: `write_prefix` buraxıldıqda, defolt olaraq paylaşımın kök kataloquna yükləməyə icazə verilir):

```yaml
id: 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
perm: rw
```

2. `~/.config/sync0/config.nt`-də istifadəçi kataloqunu və çoxsaylı fayl paylaşım qruplarını konfiqurasiya edin:

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

Paylaşım xidmətinə başlayın:

```bash
# Birbaşa işə salma (~/.config/sync0/config.nt və users_dir kataloqlarında bütün .nt istifadəçi konfiqurasiyalarını avtomatik yükləyir)
sync0 share --region SHA

# Və ya çoxlu paylaşma qruplarını tez dərc edin və qrup üzvlərinə birbaşa komanda xətti ilə icazə verin
sync0 share dataset:./data team-vault:./vault \
  --auth \
  --allow-ro 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c \
  --allow-rw 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f:uploads/bob,shared/common \
  --allow-rw 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
```

### 3. Bütün məzmunu çəkin və ya birdən çox fayl və kataloq seçin (`sync0 pull`)

Paylaşım qrupunda bütün mövcud toxum qovşaqlarını avtomatik əldə etmək üçün istənilən həmyaşıd qovşağına qoşulun və eyni zamanda bir neçə toxum qovşağından çəkin:

```bash
# Bütün verilənlər toplusu paylaşma qrupunu yerli ./local_data-ya çəkin
sync0 pull dataset:// ./local_data -p <node ünvanı>

# Çox seçimli sinxronizasiya: eyni vaxtda bir fayl docs/readme.md və alt kataloqlar aktivləri/ikonları, src/core çəkin
sync0 pull dataset://docs/readme.md,assets/icons,src/core ./local_data -p <node ünvanı>
```

### 4. Fayl və ya kataloqu paylaşma qrupuna yükləyin (`sync0 push`)

Yükləmə icazəsi olan üzvlər (`rw` və ya `admin`) yerli faylları və ya kataloqları uzaqdan paylaşma qruplarına yükləyə bilər:

```bash
# Kök kataloq yükləmə icazəsi olan üzvlər: Birbaşa paylaşılan kök kataloquna yükləyin
sync0 push team-vault:// ./new_release -p <node ünvanı>

# Yalnız xüsusi prefiksə (yükləmələr/bob kimi) icazə verilmiş üzvlər: icazə verilən yol prefiksinə yükləyin
sync0 push komanda-vault://uploads/bob/report.pdf ./report.pdf -p <node ünvanı>
```

### 5. Avtomatik sinxronizasiya və əkməyə abunə olun (`sync0 sub`)

Müəyyən bir qovşağın paylaşma qrupuna abunə olduqdan sonra, yerli server həm qəbuledici müştəri, həm də yeni toxumçu serverdir; mənbə node faylı yeniləndikdə, o, avtomatik sinxronlaşdırılır və digər qovşaqlar üçün parçalanmış yükləmələri təmin edir:

```bash
# Bütün paylaşma qrupuna abunə olun və avtomatik olaraq toxum atın
sync0 sub dataset:// ./local_mirror -p <node ünvanı>

# Müəyyən edilmiş fayllara və alt kataloqlara abunə olmaq və onları səpmək üçün yalnız çoxlu seçim edin
sync0 alt verilənlər dəsti://models/llama.onnx,configs ./local_mirror -p <node ünvanı>
```

### 6. Çox maşın qrupunun həmyaşıd sinxronizasiyası (`sync0 group`)

Birdən çox maşın eyni paylaşma qrupuna qoşulur, bir-biri avtomatik olaraq mövcud toxum qovşaqlarını aşkar edir və ikitərəfli sinxronizasiyanı təmin edir (tam sinxronizasiyanı və ya bir neçə fayl və qovluq seçimini dəstəkləyir):

```bash
# Qrupa və ikitərəfli sinxronizasiyaya tam qoşulun
sync0 group team-docs:// ./docs -p <node ünvanı>

# Çox seçimli sinxronizasiya qrupunda yalnız wal kataloqu və meta.json faylı
sync0 group team-docs://wal,meta.json ./docs -p <node ünvanı>
```

### 7. İki maşın arasında nöqtədən nöqtəyə birbaşa ötürmə (`send` / `recv`)

Qrupu əvvəlcədən konfiqurasiya etməyə ehtiyac yoxdur və siz faylları və ya qovluqları müvəqqəti olaraq iki maşın arasında birbaşa ötürə bilərsiniz:

```bash
# Qəbuledici monitorinq
sync0 recv 0.0.0.0:9000 ./backup

# Göndərmə sonu birbaşa faylları və ya qovluqları köçürür (isteğe bağlı - artıq faylları təmizləmək üçün güzgü silmək, - sürət həddi)
sync0 send ./project 192.168.1.10:9000
```

### 8. Çox dilli keçid

`sync0` 75 dili dəstəkləyir və defolt olaraq sistem dil mühitinə uyğun olaraq avtomatik keçid edir; `SYNC0_LANG` vasitəsilə də əl ilə müəyyən edilə bilər:

```bash
SYNC0_LANG=en sync0 --help
SYNC0_LANG=ja sync0 --help
SYNC0_LANG=de sync0 --help
```