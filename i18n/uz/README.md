# sync0

`sync0` markazlashtirilmagan P2P fayl almashish, real vaqt rejimida sinxronlash va ko'p manbali tarqatish vositasi `rsync` o'rnini bosadi - transkontinental WAN o'lchangan tezligi `rsync` ning **5,89×** (bir nuqta geometrik o'rtacha tezligi **2,67 × 6 **), manba geometrik o'rtacha tezligi **2,67 × 6 **, geometrik o'rtacha tezligi P2. transokeanik yuqori kechikishli havola P2P tezlashuvi **7,68×** gacha (To‘liq havola 24-stsenariy keng qamrovli o‘rtacha geometrik tezlik **2,56×**).

Har bir mashina ham server, ham mijozdir: markaziy server yoki umumiy tarmoq manziliga ehtiyoj yo'q, avtomatik teshik ochish taqsimlangan tarmoq umumiy kalit manziliga asoslanadi va u ikki mashina o'rtasida to'g'ridan-to'g'ri uzatishni, ko'p almashuvli guruh ruxsatini hostingni, talab bo'yicha ko'plab pastki kataloglarni tanlashni, millisekundlik darajadagi real vaqtda ko'p manbali sinxronlashtirishni va P2P ulanishini qo'llab-quvvatlaydi.

## O'rnatish

### 1-usul: yukni o'rnatish

```bash
cargo install sync0
```

### 2-usul: Oldindan kompilyatsiya qilingan ikkilik fayllarni yuklab oling

Tizimga mos keladigan `.tar.xz` siqilgan paketni yuklab olish uchun [GitHub Releases](https://github.com/webc-site/sync0/releases) ga oʻting (Linux, macOS, Windows, Android, iOS, FreeBSD-ni qoʻllab-quvvatlaydi), arxivdan oching va `PATH`-ga joylashtiring:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Tezroq boshlang

### 1. Ikki mashina o'rtasida nol konfiguratsiyali to'g'ridan-to'g'ri uzatish (`send` / `recv`)

Almashish guruhini sozlashning hojati yo'q, fayllar yoki kataloglarni ikki mashina o'rtasida yuqori tezlikda to'g'ridan-to'g'ri uzatish:

```bash
# Qabul qiluvchi tomon portni tinglaydi va ./backup ga yozadi
sync0 recv 0.0.0.0:9000 ./backup

# Yuboruvchidan to'g'ridan-to'g'ri uzatish (ixtiyoriy - ortiqcha fayllarni tozalash uchun oynani o'chirish, - 50 MB tezlik chegarasi)
sync0 send ./project 192.168.1.10:9000
```

### 2. Post almashish va talab boʻyicha tortib/yuklash (`share` / `pull` / `push`)

```bash
# Server: Bitta buyruq qatori bilan almashish guruhini nashr eting (ishlashdan keyin IP-bo'sh iroh tugun manzilini chiqaring)
sync0 share dataset:./data

# Mijoz: butun ma'lumotlar to'plamini almashish guruhini mahalliy ./local_data ga torting
sync0 pull dataset:// ./local_data -p <tugun manzili>

# Mijoz: so'rov bo'yicha bir nechta tanlovlarni torting (faqat belgilangan bitta fayl va pastki kataloglarni sinxronlashtiring)
sync0 pull ma'lumotlar to'plami://docs/readme.md,assets/icons,src/core ./local_data -p <tugun manzili>

# Mijoz: fayllar yoki kataloglarni ruxsat berilgan almashish guruhlariga yuklang
sync0 push ma'lumotlar to'plami://releases/v1 ./build_output -p <tugun manzili>
```

### 3. Haqiqiy vaqtda obunani ekish va ko'p mashinali ikki tomonlama sinxronizatsiya (`sub` / `group`)

Uzoq ulanishni o'rnatgandan so'ng, masofaviy uchida biron bir fayl **qo'shish, o'zgartirish, o'chirish, ruxsatlarni o'zgartirish yoki vaqtni o'zgartirish** sodir bo'lsa, mahalliy katalog real vaqt rejimida millisekundlarda sinxron ravishda yangilanadi (yangi/o'zgartirilgan katta fayllar faqat farq bloklarini tortib oladi va ularni joyida yangilaydi, fayllarni o'chirish mahalliy fayllarni avtomatik ravishda tozalaydi va faqat to'g'ridan-to'g'ri bo'sh kataloglarni o'zgartiradi):

- **`sync0 sub`(Bir tomonlama real vaqt rejimida aks ettirish + avtomatik ekish)**: Mahalliy real vaqt rejimi ulashish oxiridagi qoʻshimchalar, oʻchirishlar va modifikatsiyalarni kuzatib boradi va boshqa tortish tugunlari uchun bir vaqtda tezlashtirishni taʼminlash uchun avtomatik ravishda P2P urugʻ manbai boʻlib xizmat qiladi.
- **`sync0 group`(Multi-machine peer-to-peer real time real time synchronization)**: Agar fayllar guruhdagi istalgan mashinada qo'shilsa, o'chirilsa yoki o'zgartirilsa, boshqa barcha onlayn tugunlar real vaqtda sinxron ravishda yangilanadi va bir-biriga urug' bo'lib xizmat qiladi.

```bash
# Butun almashish guruhiga obuna bo'ling: masofaviy qo'shimchalar, o'chirishlar va o'zgartirishlar real vaqtda mahalliy sinxronlashtiriladi va avtomatik ravishda ekiladi.
sync0 sub ma'lumotlar to'plami:// ./local_mirror -p <tugun manzili>

# Talab bo'yicha ko'p tanlovli obuna: real vaqt rejimida faqat ko'rsatilgan bitta fayl va pastki kataloglarni sinxronlashtiring va urug'lang
sync0 sub ma'lumotlar to'plami://models/llama.onnx,configs ./local_mirror -p <tugun manzili>

# Ko'p mashinali tengdoshlar guruhi ikki tomonlama real vaqtda sinxronizatsiya (shuningdek, to'liq yoki vergul ko'p tanlovli yo'llarni qo'llab-quvvatlaydi)
sync0 group team-docs:// ./docs -p <tugun manzili>
```

### 4. Jamoa koʻp ulashish guruhi va aʼzolar ruxsati konfiguratsiyasi (`.nt` konfiguratsiyasi)

Standart konfiguratsiya katalogi `~/.config/sync0` (`-c <katalog` orqali almashtirilishi mumkin), minimalist [NestedText](https://nestedtext.org) (`.nt`) formatidan foydalangan holda:
- **Ommaviy tarmoqni o‘qishni cheklashni qo‘llab-quvvatlaydi (faqat avtorizatsiya qilingan foydalanuvchilar o‘qiy oladi)**: Agar ulashish guruhi `users:` a'zolar ro‘yxati bilan sozlangan bo‘lsa (yoki buyruq qatori `--allow-r` / `--allow-rw` / `--allow-w` / `--auth`ni ko‘rsatsa), tizim avtomatik ravishda yoqiladi (maxfiy **"6">). va umumiy tarmoq anonim tugunlarini sanab o'tish yoki o'qishni taqiqlaydi va P2P ekish tugunlariga obuna bo'lish ham oq ro'yxatni bir vaqtning o'zida meros qilib oladi va faqat ruxsat berilgan foydalanuvchilarga o'qish imkonini beradi; `users` bilan sozlanmagan baham ko'rish guruhlari sukut bo'yicha umumiy va faqat o'qish uchun mo'ljallangan (agar siz hammaga ochiq bo'lishni va faqat o'qish uchun va faqat belgilangan a'zolar uchun yuklash uchun ochiq bo'lishni istasangiz, `auth: false` ni aniq belgilashingiz mumkin).
- **Yuqoridagi o‘qish-yozish va pastki katalogni yetkazib berish ruxsatlari**: a’zolar sukut bo‘yicha faqat o‘qish uchun mo‘ljallangan (`r` aniq yozilishi shart emas) va `r` (faqat o‘qish uchun), `w` (faqat tushirish qutisiga yozish, o‘qib bo‘lmaydi), <b) id="te" va <b) id=""4" ni qo‘llab-quvvatlaydi. (yuklash uchun ruxsat berilgan pastki katalog prefiksini cheklang).

**A'zo konfiguratsiyasi misoli (`~/.config/sync0/users/`):**

```yaml
# ~/.config/sync0/users/alice.nt (standart faqat o'qish uchun, tezlik chegarasi 80 MB)
id: 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c
rate: 80MB
```

```yaml
# ~/.config/sync0/users/bob.nt (o'qish va yozish uchun ruxsatlar, faqat belgilangan quyi katalogga yuklash mumkin)
id: 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f
perm: rw
write_prefix:
  - uploads/bob
  - shared/common
```

```yaml
# ~/.config/sync0/users/charlie.nt (faqat yozish ruxsatnomasi, o'qish taqiqlangan)
id: 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
perm: w
```

**Guruh konfiguratsiyasi misolini almashish (`~/.config/sync0/config.nt`):**

```yaml
users_dir: ./users

groups:
  # Sozlanmagan foydalanuvchilar: standart umumiy tarmoq faqat o‘qish uchun
  dataset:
    path: ./data
    rate: 50MB

  # Sozlangan foydalanuvchilar: umumiy tarmoq oʻqishini avtomatik ravishda cheklash (faqat Elis va Bob oʻqiy oladi, Charli faqat yetkazib berishi va yuklashi mumkin)
  team-vault:
    path: ./vault
    users:
      - alice
      - bob
      - charlie
```

Xizmatni ishga tushiring (konfiguratsiya faylini yuklang yoki umumiy tarmoqni o'qishni cheklash va a'zolarni avtorizatsiya qilish uchun to'g'ridan-to'g'ri buyruq qatori parametrlaridan foydalaning):

```bash
# Boshlash uchun ~/.config/sync0/config.nt-ni yuklang
sync0 share --region SHA

# Yoki sof buyruq qatori orqali shaxsiy almashish guruhini nashr eting (ommaviy tarmoq o'qishini avtomatik ravishda cheklash uchun --allow-* belgilang va unga faqat vakolatli foydalanuvchilar kirishi mumkin)
sync0 share team-vault:./vault \
  --allow-r 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c \
  --allow-rw 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f:uploads/bob,shared/common \
  --allow-w 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
```

### 5. Tugun manzili, tashrif qog'ozi va ko'p tilli

```bash
# Mahalliy IPsiz tugun manzilini ko'ring (kalit birinchi ishga tushirish uchun avtomatik ravishda yaratiladi)
sync0 id

# Ed25519 o'z-o'zidan imzolangan tugun biznes kartasini yarating (taxallus, shahar kodi, elektron pochta, veb-sayt, kirish)
sync0 profile -u alice --region SHA --email alice@example.com -b "Shanghai node"

# 75 ta o'rnatilgan til mavjud bo'lib, ular sukut bo'yicha tizim tiliga amal qiladi va atrof-muhit o'zgaruvchilari orqali almashtirilishi mumkin.
SYNC0_LANG=en sync0 --help
```

## Ishlash va asosiy xususiyatlar

### Qit'alararo WAN haqiqiy o'lchovi (taqqoslash `rsync`)

Haqiqiy o'lchash muhiti: Amerika Qo'shma Shtatlari `c1` → Evropa `n1` (RTT 65 ms), ma'lumotlar to'plami Linux yadrosi `linux-7.3.3`:

#### 1) Alakart rejim va `rsync`

| Ma'lumotlar to'plami | hajmi | `sync0` Yagona nuqta<br>(Mbps) | `rsync`<br>(Mbps) | Tezlashtirish (`sync0` / `rsync`) |
| :--- | ---: | ---: | ---: | ---: |
| `linux-7.3.3.tar.xz`(siqilib bo'lmaydi) | 155 MB | **236.48** | 108.74 | **2,17×** |
| `linux-7.3.3.tar`(siqilishi mumkin) | 1.6 GB | **843.83** | 385.06 | **2,19×** |
| `linux-7.3.3.tar`(mahalliy o'zgarishlarning o'sishi) | 1.6 GB | **8,014.15** | 1,361.72 | **5,89×** |
| `linux-7.3.3/` (95 979 ta kichik fayl) | 1.5 GB | **627.58** | 347.80 | **1,80×** |
| **geometrik o'rtacha** | — | **1,000.91** | 375.26 | **2,67×** |

#### 2) P2P ikki manba rejimi va `rsync`

| Ma'lumotlar to'plami | hajmi | `sync0` P2P ikkilamchi manba<br>(Mbps) | `rsync`<br>(Mbps) | Tezlashtirish (`sync0` / `rsync`) |
| :--- | ---: | ---: | ---: | ---: |
| `linux-7.3.3.tar.xz`(siqilib bo'lmaydi) | 155 MB | **330.09** | 108.74 | **3,04×** |
| `linux-7.3.3.tar`(siqilishi mumkin) | 1.6 GB | **1,551.86** | 385.06 | **4,03×** |
| `linux-7.3.3.tar`(mahalliy o'zgarishlarning o'sishi) | 1.6 GB | **5,474.17** | 1,361.72 | **4,02×** |
| `linux-7.3.3/` (95 979 ta kichik fayl) | 1.5 GB | **706.56** | 347.80 | **2,03×** |
| **geometrik o'rtacha** | — | **1,186.42** | 375.26 | **3,16×** |

> Eslatma: P2P ikki manba rejimida Yevropa tugunlari `s1` ekish uchun hamkorlik qiladi va qit'alararo magistral RTT 65 ms ni tashkil qiladi.

### Mahsulotning asosiy xususiyatlari

- **Ochiq kalit manzilli avtomatik teshik ochish**: markaziy server va umumiy tarmoq manziliga ehtiyoj yo‘q, taqsimlangan tarmoq ochiq kalit manzili asosida avtomatik NAT teshik ochish.
- **Millisoniya darajasidagi real vaqtda sinxronizatsiya va metamaʼlumotlardan xabardorlik**: Mahalliy fayl hodisasi monitoringi (`FSEvents` / `inotify` / `ReadDirectoryChangesW`), real vaqtda fayllarni qo'shish, o'chirish, POSIX ruxsatnomalari (`mode`) va millisekundlik o'zgartirish vaqti (=""> <b) asosida; faqat ruxsatlar yoki vaqt o'zgartirilganda ma'lumotlar blokini uzatish nolga teng.
- **BT uslubidagi bir vaqtning o'zida yuklab olish va yuklash va mintaqaviy yaqinlikni rejalashtirish**: Yuklash tugunining to'liq yuklanishini kutish shart emas. 128-bitli xesh tekshiruvidan o'tgan barcha blok oraliqlari (`Op::Have`) boshqa tugunlar bir vaqtning o'zida tortib olishi uchun darhol xotira/vaqtinchalik hovuzda qulflanadi. Dinamik PEX tugunlari almashinuvi va sekin tugunning yarmini oldini olish (Work-O'g'irlash) bilan birgalikda klaster o'tkazuvchanligi maksimal darajaga ko'tariladi.
- **Zararli tugunlarni buzish va klaster bilan zaharlanishga qarshi immunitet**: nufuzli manbalar ro'yxati `root_hash` O'z-o'zidan barqaror qulflash + majburiy apparat tezlashuvi `gxhash128` har bir blok diskga joylashtirilishidan va tashqaridan urug'lantirilishidan oldin dayjest tekshiruvi; agar buzilgan blok topilsa, millisekundlik o'chirgich zararli tugunni o'chiradi va halol manbadan avtomatik ravishda tiklanadi.
- **FastCDC in-situ o'sishi va kichik fayllarni yig'ish**: Katta fayllarni qisman o'zgartirish uchun faqat differentsial bloklar uzatiladi va diskka joylashtiriladi; katta hajmdagi kichik fayl xotirasi paketi va prefiksli differentsial siqish fayldan faylga aylanma yukni yo'q qiladi.
- **Elektr ta'minoti uzilganidan keyin o'z-o'zini tiklash va qayta ishga tushirilgandan so'ng qayta ishga tushirish**: Redisga o'xshash RDB + AOF gibrid formati (`.s0_subs.bin`) ekish holati va fayllar ro'yxatini saqlab qolish uchun ishlatiladi, kundalik o'zgarishlar mikrosekundlarda qo'shiladi va avtomatik ravishda qayta yoziladi va muntazam ravishda tartibga solinadi. Server qayta ishga tushirilgandan so'ng, oflayn o'zgarishlar avtomatik ravishda ekish tuguniga suriladi.

## Texnik arxitektura va hujjatlar

- **Ikki tekislikni ajratish protokoli**:
  - **Boshqaruv va real vaqtda o‘zgartirish yuzasi (`iroh` QUIC / TCP, birinchi bayt `PROTO_VERSION` versiya qo‘l siqish) **: Ed25519 tashrif qog‘ozini tekshirish, ruxsatni tekshirish, Swarm urug‘ini aniqlash va `u64::MAX` darajadagi ommaviy o‘zgarishlar oqimini qo‘llab-quvvatlash va real vaqtda siqilgan id="4"> <b id="> <b"> surish (`FsOp::Upsert` / `FsOp::Remove` / `FsOp::Meta` Uchta ibtidoiy).
  - **Ma'lumotlarni tarqatish yuzasi (mahalliy UDP + `compio` asinxron kiritish/chiqarish) **: 20B ixcham sarlavha, 18B TACK davriy fikr-mulohaza, BBRv3/Copa gibrid tirbandligi nazorati, RACK-TLP paket yo'qotilishini tiklash, birinchi blok Shannon entropiyasiga moslashuvchi Zstd="-" va siqish bloki <b-end bloki. tekshirish.
- **Batafsil ma'lumot**:
  - [Ishlash o'lchovi hisoboti va reproduktsiya skripti](https://github.com/webc-site/sync0/blob/main/doc/zh/benchmark.md)
  - [Tizim arxitekturasini loyihalash](https://github.com/webc-site/sync0/blob/main/doc/zh/design_v4.md)
  - [Ikkilik sim protokoli va doimiylik spetsifikatsiyasi](https://github.com/webc-site/sync0/blob/main/doc/zh/proto.md)
  - [Shaxsni autentifikatsiya qilish, vakolatlarni nazorat qilish va yaxlitlikni tekshirish](https://github.com/webc-site/sync0/blob/main/doc/zh/crypto.md)