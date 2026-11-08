# sync0

`sync0` - bu Resilio Sync bilan raqobatlashadigan tengdoshga (P2P) fayl almashish va ko'p mashinali sinxronizatsiya vositasi.

Har bir mashina ham server, ham mijozdir: har qanday tugun bir nechta fayl almashish guruhlarini nashr qilishi mumkin. Boshqa tugunlar qo'shilgandan so'ng, ular avtomatik ravishda teng serverdan mavjud urug'lik tugunlarini olishlari va ularni bir vaqtning o'zida bir nechta manbalardan tortib olishlari mumkin. Sinxronizatsiya tugagandan so'ng, yuklab olishni tezlashtirish uchun ko'proq tugunlar uchun urug'lar avtomatik ravishda yaratiladi.

## Asosiy diqqatga sazovor joylar

- **Hamma ham server, ham mijozdir**: Resilio Sync bilan solishtirish mumkin, markaziy serverga bog'liqlik yo'q. Har qanday tugun bir nechta fayl almashish guruhlarini yaratishi mumkin, shuningdek, boshqa odamlarning almashish guruhlariga qo'shilishi mumkin; tugunlar bir vaqtning o'zida bir vaqtning o'zida tortish va bir nechta mashinalarni real vaqtda sinxronlashtirishga erishish uchun mavjud urug'lik tugunlarini avtomatik ravishda almashadi va oladi.
- **IPsiz tugun manzili**: Odatiy bo'lib, tugun kaliti va `base64url` kodlangan `iroh` manzili umumiy IP-ni eslab yoki oshkor qilmasdan avtomatik ravishda `~/.config/sync0` da yaratiladi.
- **Talab bo'yicha ko'p tanlovli sinxronizatsiya**: butun almashish guruhining sinxronizatsiyasini qo'llab-quvvatlaydi, shuningdek, bir vaqtning o'zida bir nechta fayl yoki pastki kataloglarni tanlash orqali talab bo'yicha sinxronlashni qo'llab-quvvatlaydi.
- **Guruh aʼzolarining ierarxik ruxsatnomalari va yuklash katalogini boshqarish**: faqat oʻqish uchun (`ro`), oʻqish-yozishni yuklash (`rw`) va boshqaruv (`admin`) ruxsatlarini qoʻllab-quvvatlaydi va har bir aʼzo yuklashi mumkin boʻlgan yoʻl prefiksini alohida belgilashi mumkin (standart boʻyicha, asosiy rejissyorga yuklashga ruxsat beriladi).
- **Tarmoqlar bo‘ylab to‘liq tezlikda va bosqichma-bosqich sinxronizatsiya**: Katta fayllarni bir vaqtda uzatish, xotirada katta hajmdagi kichik fayllarni yig‘ish va fayllar qisman o‘zgartirilganda faqat sinxronlashtirilgan ma’lumotlar bloklari.

## Rsync bilan tezlikni taqqoslash

### Transokeanik tarmoqning yagona manbali haqiqiy o'lchovi (Xitoy → Qo'shma Shtatlar, kechikish 260 ms)

| Transmissiya stsenariysi | Ma'lumotlar hajmi | model | `sync0` | `rsync -az` | Multiplikatorni oshiring |
| :--- | :---: | :---: | :---: | :---: | :---: |
| 500 ta kichik fayl | 1.91 MB | To'liq sinxronizatsiya | 18.84 Mbps | 5.32 Mbps | 3.5x |
| 500 ta kichik fayl | 1.91 MB | Incremental sinxronizatsiya | 27.15 Mbps | 3.83 Mbps | 7.1x |
| Kod va arxiv paketlari (`.tar`) | 10.0 MB | To'liq sinxronizatsiya | 139.70 Mbps | 24.97 Mbps | 5.6x |
| Kod va arxiv paketlari (`.tar`) | 10.0 MB | Incremental sinxronizatsiya | 83.38 Mbps | 26.02 Mbps | 3.2x |
| Video fayl (`.mp4`) | 15.0 MB | To'liq sinxronizatsiya | 54.06 Mbps | 27.04 Mbps | 2.0x |
| Video fayl (`.mp4`) | 15.0 MB | Incremental sinxronizatsiya | 216.83 Mbps | 21.51 Mbps | 10.1x |

### Transoceanic tarmog'i P2P ko'p tugunli tortish testi (Xitoy → Qo'shma Shtatlar ikki tugun, kechikish 260 ms)

Ko'p manbali urug'lik tugunining chiziqli bir vaqtning o'zida tortishishini o'lchagan taqqoslash:

| Transmissiya stsenariysi | Ma'lumotlar hajmi | Yagona tugunli ekish | Ikki tugunli P2P bir vaqtda ekish | Multiplikatorni oshiring |
| :--- | :---: | :---: | :---: | :---: |
| 500 ta kichik fayl | 1.97 MB | 5.28 Mbps | 7.87 Mbps | 1.5x |
| Kod va arxiv paketlari (`.tar`) | 30.0 MB | 56.35 Mbps | 83.17 Mbps | 1.5x |
| Video fayl (`.mp4`) | 30.0 MB | 27.00 Mbps | 39.46 Mbps | 1.5x |

### Transkontinental yuqori tezlikdagi ajratilgan liniyaning haqiqiy o'lchovi (Evropa → Shimoliy Amerika, kechikish 86 ms)

| Transmissiya stsenariysi | Ma'lumotlar hajmi | `sync0` | `rsync` | Multiplikatorni oshiring |
| :--- | :---: | :---: | :---: | :---: |
| 10 000 ta kichik fayllar | 77.6 MB | 688 Mbps | 128 Mbps | 5.4x |
| Kod va arxiv paketlari (`.tar`) | 512 MB | 2111 Mbps | 438 Mbps | 4.8x |
| Video fayl (`.mp4`) | 512 MB | 556 Mbps | 338 Mbps | 1.6x |
| Incremental sinxronizatsiya (1% tarkibni o'zgartirish) | 512 MB | 34133 Mbps | 2214 Mbps | 15.4x |

## O'rnatish

### 1-usul: Yuk orqali o'rnatish

```bash
cargo install sync0
```

### 2-usul: Oldindan kompilyatsiya qilingan ikkilik fayllarni yuklab oling

Tegishli tizim uchun (Linux, macOS, Windows, Android, FreeBSD, NetBSD-ni qo'llab-quvvatlaydi) `.tar.xz` siqilgan paketini yuklab olish uchun [GitHub Releases](https://github.com/webc-site/sync0/releases) sahifasiga o'ting va uni `PATH` katalogiga qo'ying:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Tezroq boshlang

`sync0` sukut bo'yicha konfiguratsiya katalogi sifatida `~/.config/sync0` dan foydalanadi (agar siz boshqa kataloglarga o'tishingiz kerak bo'lsa, siz `-c <katalog` qo'shishingiz mumkin).

### 1. Mahalliy tugun manzilini va tashrifnomani sozlashni ko'ring

```bash
# Mahalliy tugun manzilini ko'rish (kalit birinchi marta ~/.config/sync0 da avtomatik ravishda yaratiladi)
sync0 id

# Shaxsiy tugun biznes kartasini yarating (foydalanuvchi nomi, shahar, elektron pochta, veb-sayt va imzoni 256 so'z ichida qo'llab-quvvatlaydi)
sync0 profile -u alice --region SHA --email alice@example.com --url https://example.com -b "Shanghai node"
```

### 2. Bir nechta fayl almashish guruhlarini nashr qilish va guruh aʼzolariga ruxsatlarni oʻrnatish (`.nt` Konfiguratsiya)

`sync0` [NestedText](https://nestedtext.org) (`.nt`) formatida sozlangan. Har bir foydalanuvchi alohida `.nt` konfiguratsiya fayliga ega. Foydalanuvchi profili katalogini `config.nt` ichidagi `users_dir` orqali belgilashingiz mumkin (bu katalogdagi barcha `.nt` fayllarni avtomatik ravishda o'z ichiga oladi). Foydalanuvchi ham, yuklashga ruxsat berilgan yoʻl prefiksi ham tartibsiz roʻyxatdan foydalanadi (`-`). **Yuklash yoʻli prefiksi toʻldirilmagan boʻlsa, sukut boʻyicha ulashishning asosiy katalogiga yuklashga ruxsat beriladi**.

1. `~/.config/sync0/users/` katalogidagi har bir a'zo uchun alohida `.nt` konfiguratsiya fayllarini yarating:

`~/.config/sync0/users/alice.nt` (faqat o'qish uchun mo'ljallangan a'zo, tarifi cheklangan `80MB`, `rate` `GB`, `MB`, `KB` birliklarni qo'llab-quvvatlaydi):

```yaml
id: 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c
perm: ro
rate: 80MB
```

`~/.config/sync0/users/bob.nt` (cheklangan yuklash a'zolari: faqat `uploads/bob` va `shared/common` kataloglarga yuklash mumkin):

```yaml
id: 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f
perm: rw
write_prefix:
  - uploads/bob
  - shared/common
```

`~/.config/sync0/users/charlie.nt` (Birlamchi a'zo yuklash: `write_prefix` o'tkazib yuborilsa, ulashishning asosiy katalogiga sukut bo'yicha yuklashga ruxsat beriladi):

```yaml
id: 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
perm: rw
```

2. `~/.config/sync0/config.nt`da foydalanuvchi katalogini va bir nechta fayl almashish guruhlarini sozlang:

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

Xizmatni almashishni boshlang:

```bash
# To'g'ridan-to'g'ri ishga tushirish (~/.config/sync0/config.nt va users_dir kataloglarida barcha .nt foydalanuvchi konfiguratsiyasini avtomatik ravishda yuklaydi)
sync0 share --region SHA

# Yoki bir nechta almashish guruhlarini tezda nashr eting va guruh a'zolariga buyruq satri orqali ruxsat bering
sync0 share dataset:./data team-vault:./vault \
  --auth \
  --allow-ro 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c \
  --allow-rw 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f:uploads/bob,shared/common \
  --allow-rw 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
```

### 3. Barcha tarkibni torting yoki bir nechta fayl va kataloglarni tanlang (`sync0 pull`)

Almashish guruhidagi barcha mavjud tugunlarni avtomatik ravishda olish uchun har qanday tengdosh tugunga ulaning va bir vaqtning o'zida bir nechta urug'lik tugunlaridan tortib oling:

```bash
# Barcha ma'lumotlar to'plamini almashish guruhini mahalliy ./local_data ga torting
sync0 pull dataset:// ./local_data -p <tugun manzili>

# Ko'p tanlovli sinxronizatsiya: bir vaqtning o'zida bitta fayl docs/readme.md va subkataloglar aktivlari/piktogrammalarini, src/core-ni torting
sync0 pull ma'lumotlar to'plami://docs/readme.md,assets/icons,src/core ./local_data -p <tugun manzili>
```

### 4. Fayl yoki katalogni almashish guruhiga yuklash (`sync0 push`)

Yuklash ruxsatiga ega (`rw` yoki `admin`) a'zolar mahalliy fayllar yoki kataloglarni masofaviy almashish guruhlariga yuklashlari mumkin:

```bash
# Ildiz katalogni yuklash ruxsatiga ega aʼzolar: Toʻgʻridan-toʻgʻri umumiy asosiy katalogga yuklang
sync0 push team-vault:// ./new_release -p <tugun manzili>

# Faqat ma'lum bir prefiksga ruxsat berilgan a'zolar (masalan, yuklamalar/bob): ruxsat etilgan yo'l prefiksiga yuklash
sync0 push team-vault://uploads/bob/report.pdf ./report.pdf -p <tugun manzili>
```

### 5. Avtomatik sinxronlash va ekishga obuna bo'ling (`sync0 sub`)

Muayyan tugunning almashish guruhiga obuna bo'lgandan so'ng, mahalliy server ham qabul qiluvchi mijoz, ham yangi ekish serveri hisoblanadi; manba tugun fayli yangilanganda, u avtomatik ravishda sinxronlashtiriladi va boshqa tugunlar uchun qismlarga bo'lingan yuklab olishni ta'minlaydi:

```bash
# Barcha almashish guruhiga obuna bo'ling va avtomatik ravishda urug'lantiring
sync0 sub ma'lumotlar to'plami:// ./local_mirror -p <tugun manzili>

# Ko'rsatilgan fayllar va pastki kataloglarga obuna bo'lish va ularni ekish uchun faqat ko'p tanlang
sync0 sub ma'lumotlar to'plami://models/llama.onnx,configs ./local_mirror -p <tugun manzili>
```

### 6. Ko'p mashinali guruhli tengdosh sinxronizatsiyasi (`sync0 group`)

Bir nechta mashinalar bir xil almashish guruhiga qo'shiladi, bir-biri avtomatik ravishda mavjud tugunlarni topadi va ikki tomonlama sinxronizatsiyani saqlaydi (to'liq sinxronizatsiya yoki bir nechta fayl va kataloglarni tanlashni qo'llab-quvvatlaydi):

```bash
# Guruhga to'liq qo'shiling va ikki tomonlama sinxronizatsiya
sync0 group team-docs:// ./docs -p <tugun manzili>

# Ko'p tanlovli sinxronizatsiya guruhidagi faqat wal katalogi va meta.json fayli
sync0 group team-docs://wal,meta.json ./docs -p <tugun manzili>
```

### 7. Ikki mashina o'rtasida nuqtadan nuqtaga to'g'ridan-to'g'ri uzatish (`send` / `recv`)

Guruhni oldindan sozlashning hojati yo'q va siz to'g'ridan-to'g'ri ikkita mashina o'rtasida fayllar yoki kataloglarni vaqtincha o'tkazishingiz mumkin:

```bash
# Qabul qiluvchilarni monitoring qilish
sync0 recv 0.0.0.0:9000 ./backup

# Yuboruvchi to'g'ridan-to'g'ri fayllar yoki kataloglarni uzatadi (ixtiyoriy --ortiqcha fayllarni tozalash uchun oynani o'chirish, --tezlik tezligi chegarasi)
sync0 send ./project 192.168.1.10:9000
```

### 8. Ko'p tillarni almashtirish

`sync0` 75 ta tilni qo'llab-quvvatlaydi va sukut bo'yicha tizim til muhitiga mos ravishda avtomatik ravishda o'tadi; uni `SYNC0_LANG` orqali qo'lda ham belgilash mumkin:

```bash
SYNC0_LANG=en sync0 --help
SYNC0_LANG=ja sync0 --help
SYNC0_LANG=de sync0 --help
```