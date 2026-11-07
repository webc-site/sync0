# sync0

`sync0` - fayllarni tarmoqlar bo'ylab uzatish, kataloglarni bosqichma-bosqich sinxronlashtirish va bir vaqtning o'zida bir nechta mashinalarga ma'lumotlarni tarqatish uchun buyruq qatori vositasi.

## Asl dizayn maqsadi

O'zaro mashinalar xonalarida, mamlakatlarda yoki tarmoq kechikishlari va paketlar yo'qolgan muhitlarda `rsync` va `scp` kabi vositalar tarmoqli kengligidan foydalanishni cheklaydigan yagona ulanish ketma-ketligini tasdiqlashga asoslangan; bir vaqtning o'zida bir nechta mashinalar bir xil serverdan ma'lumotlarni tortib olganda, manba server tarmoq kartasi osongina darboğazga aylanishi mumkin.

`sync0` ning maqsadi ikkita muammoni hal qilishdir:

1. Ikki mashina o'rtasida fayl va katalog sinxronizatsiyasi to'liq jismoniy tarmoqli kengligi bilan ishlasin va faqat fayl qisman o'zgartirilganda o'zgartirilgan qismni o'tkazing.
2. Bir nechta mashinalar bir xil katalogni sinxronlashda ma'lumotlar bloklarini bir-biriga uzatishi mumkin, bu manba serverdagi yukni kamaytiradi va umumiy sinxronlash vaqtini qisqartiradi.

## Rsync bilan tezlikni taqqoslash

Sinov muhiti: Yevropa tuguni $\leftrightarrow$ Shimoliy Amerika tugun, aylanib-o‘tish uchun kechikish 86 ms, to‘liq vaqt sarfini o‘lchash, shu jumladan kataloglarni skanerlash, detuplikatsiyani taqqoslash, tarmoqni uzatish va diskni joylashtirishni tekshirish:

| Transmissiya stsenariysi | Ma'lumotlar hajmi | `sync0` | `rsync` | bir nechta |
| :--- | :---: | :---: | :---: | :---: |
| 10 000 ta kichik fayllar | 77.6 MB | 0.90 s (688 Mbps) | 4.86 s (128 Mbps) | 5.4x |
| Kod va arxiv paketlari (`.tar`) | 512 MB | 1.94 s (2111 Mbps) | 9.36 s (438 Mbps) | 4.8x |
| Video fayl (`.mp4`) | 512 MB | 7.36 s (556 Mbps) | 12.11 s (338 Mbps) | 1.6x |
| Incremental sinxronizatsiya (1% tarkibni o'zgartirish) | 512 MB | 0.12 s | 1.85 s | 15.4x |

## Samaradorlikni oshirish usullari

- **UDP bir vaqtning o'zida uzatish**: UDP orqali ma'lumotlar bloklarini yuboring va keyingi fayl uzatishni blokirovka qiluvchi bitta paket yo'qotilishining oldini olish uchun faqat yo'qolgan qismlarni qayta uzating.
- **Kontentni segmentatsiyalash va deuplikatsiya qilish**: Faylni mazmuniga ko'ra ma'lumotlar bloklariga bo'ling va tekshirish qiymatini hisoblang. Qabul qiluvchi tomonda bo'lgan bir xil tarkib tarmoqni takrorlamasdan to'g'ridan-to'g'ri mahalliy sifatida qayta ishlatiladi.
- **Tarmoq tezligiga qarab siqishni avtomatik sozlash**: Matn va kod kabi siqiladigan kontent uchun siqishni yoqing; protsessor sarfini kamaytirish uchun videolar, rasmlar, siqilgan paketlar yoki yuqori tezlikdagi LAN ostida siqishni avtomatik ravishda o'tkazib yuboring.
- **Ko‘p tugunli yaqin joydan tortib olish**: Bir nechta mashinalar bir xil katalogni tortganda, ma’lumotlar bloklari bir vaqtning o‘zida bir shahardagi yuklangan tugunlardan yoki pastroq kechikish bilan tortib olinadi.

## O'rnatish

### 1-usul: Yuk orqali o'rnatish

```bash
cargo install sync0
```

### 2-usul: Oldindan kompilyatsiya qilingan ikkilik fayllarni yuklab oling

Tegishli tizim uchun (Linux x86_64/ARM64, macOS Apple Silicon/Intel, Windows x64/ARM64, Android ARM64/x86_64, FreeBSD, NetBSD-ni qo'llab-quvvatlaydi) `.tar.xz` siqilgan paketini yuklab olish uchun [GitHub Releases](https://github.com/webc-site/sync0/releases) sahifasiga o'ting. Zipni ochgandan so'ng, `sync0` katalogini `PATH` katalogiga joylashtiring:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Foydalanuvchi uchun qoʻllanma

### 1. Ikki mashina o'rtasida fayllar yoki kataloglarni to'g'ridan-to'g'ri uzatish

Qabul qiluvchi mashinada monitoringni boshlang va qabul qilish va saqlash yo'lini belgilang:

```bash
sync0 recv 0.0.0.0:9000 ./backup
```

Yuboruvchi mashinadan bitta fayl yoki butun katalogni yuboring:

```bash
sync0 send ./project 192.168.1.10:9000
```

Tez-tez ishlatiladigan parametrlar:

```bash
# Qabul qiluvchi tomonda mavjud bo'lgan, lekin jo'natuvchi tomonda bo'lmagan ortiqcha fayllarni o'chirish (oyna sinxronizatsiyasi)
sync0 send ./project 192.168.1.10:9000 --delete

# Yuborish tezligini cheklash (birlik: bayt/sekund, masalan, 50 MB/s)
sync0 send ./project 192.168.1.10:9000 --rate 50000000

# Ovozsiz rejim (hech qanday taraqqiyot paneli ko'rsatilmaydi)
sync0 send ./project 192.168.1.10:9000 -q
```

### 2. Birgalikda kataloglarni nashr qilish va bir nechta tugunlardan tortib olish

Bosh serverda umumiy katalogni (`dataset` nomli) nashr qiling:

```bash
sync0 share dataset ./data --region SHA
```

Umumiy katalogni boshqa mashinalarda torting (bir vaqtning o'zida bir nechta tugun manzillarini bir vaqtning o'zida olish uchun vergul bilan ajratilgan holda to'ldirishingiz mumkin):

```bash
sync0 pull 10.0.0.1:9000,10.0.0.2:9000 dataset ./local_data
```

Umumiy katalogdagi faqat ma'lum bir pastki katalogni torting:

```bash
sync0 pull 10.0.0.1:9000 dataset ./local_data --subpath 2026/logs
```

Mahalliy o'zgarishlarni masofaviy umumiy katalogga qaytaring:

```bash
sync0 push 10.0.0.1:9000 dataset ./local_data
```

### 3. Katalogning avtomatik yangilanishiga obuna bo'ling

Manba tugunining umumiy katalogiga obuna bo'ling. Fayl manba katalogiga qo'shilsa yoki o'zgartirilsa, u avtomatik ravishda mahalliy kompyuter bilan sinxronlashtiriladi. Shu bilan birga, mahalliy mashina avtomatik ravishda boshqa mashinalar uchun tugun bo'lib xizmat qiladi:

```bash
sync0 sub 10.0.0.1:9000 dataset ./local_mirror
```

### 4. Ko'p mashinali guruhlarning ikki tomonlama sinxronizatsiyasi

Bir nechta mashinalar bir xil guruh nomiga qo'shiladi va har qanday tugun katalogidagi o'zgarishlar avtomatik ravishda guruhning boshqa tugunlari bilan sinxronlashtiriladi:

```bash
sync0 group team-docs ./docs 10.0.0.1:9000,10.0.0.2:9000
```

### 5. Tugun identifikatori va shaxsiy tashrif qog'ozi

Har bir tugun foydalanuvchi nomi, shahar, elektron pochta, veb-sayt va shaxsiy imzo (maksimal 256 belgi) o'rnatishni qo'llab-quvvatlaydi va mahalliy shaxsiy kalit imzo autentifikatsiyasidan o'tadi:

```bash
# Mahalliy umumiy kalit identifikatorini ko'rish (kalitni saqlab qolish uchun konfiguratsiya katalogini ko'rsating)
sync0 id -c ~/.config/sync0

# Shaxsiylashtirilgan imzolar bilan tugun biznes kartalarini yarating va ko'rsating
sync0 profile -c ~/.config/sync0 \
  -u alice \
  --region SHA \
  --email alice@example.com \
  --url https://example.com \
  -b "Shanghai backup node"
```

`share`, `pull`, `sub`, `group` ham ushbu profil parametrlari bilan ishga tushirilishi mumkin:

```bash
sync0 share dataset ./data -c ~/.config/sync0 -u alice --region SHA -b "Shanghai backup node"
```

### 6. Ko'p tillarni almashtirish

`sync0` 75 ta tilni qo'llab-quvvatlaydi. Odatiy bo'lib, buyruq qatori yordami va tegishli tildagi so'rovlar operatsion tizimning til muhitiga muvofiq avtomatik ravishda ko'rsatiladi; agar tizim tili qo'llab-quvvatlanadigan ro'yxatda bo'lmasa, sukut bo'yicha ingliz tilidan foydalaniladi.

Tilni qo'lda belgilash uchun `SYNC0_LANG` muhit o'zgaruvchisini o'rnating:

```bash
SYNC0_LANG=en sync0 --help
SYNC0_LANG=ja sync0 --help
SYNC0_LANG=de sync0 --help
```