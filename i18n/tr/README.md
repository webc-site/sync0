# sync0

`sync0`, Resilio Sync ile rekabet eden, eşler arası (P2P) bir dosya paylaşımı ve çoklu makine senkronizasyon aracıdır.

Her makine hem sunucu hem de istemcidir: herhangi bir düğüm birden fazla dosya paylaşım grubu yayınlayabilir. Diğer düğümler katıldıktan sonra, eş sunucudan otomatik olarak kullanılabilir çekirdek düğümleri alabilir ve bunları aynı anda birden fazla kaynaktan çekebilirler. Senkronizasyon tamamlandıktan sonra, indirmeleri hızlandırmak amacıyla daha fazla düğüm için çekirdekler otomatik olarak oluşturulacaktır.

## Önemli Noktalar

- **Herkes hem sunucu hem de istemcidir**: Resilio Sync ile karşılaştırılabilir, merkezi sunucu bağımlılığı yoktur. Herhangi bir düğüm birden fazla dosya paylaşım grubu oluşturabilir ve ayrıca diğer kişilerin paylaşım gruplarına da katılabilir; birden fazla makinenin eş zamanlı çekilmesini ve gerçek zamanlı senkronizasyonunu sağlamak için düğümler otomatik olarak değiş tokuş yapar ve kullanılabilir çekirdek düğümleri elde eder.
- **IP'siz düğüm adresi**: Varsayılan olarak, düğüm anahtarı ve `base64url` kodlu `iroh` adresi, genel IP'yi hatırlamaya veya göstermeye gerek kalmadan `~/.config/sync0` içinde otomatik olarak oluşturulur.
- **Talep üzerine çoklu seçim senkronizasyonu**: Tüm paylaşım grubunun senkronizasyonunu destekler ve ayrıca aynı anda birden fazla dosya veya alt dizin seçilerek talep üzerine senkronizasyonu destekler.
- **Grup üyesi hiyerarşik izinleri ve yükleme dizini kontrolü**: Salt okunur (`ro`), okuma-yazma yükleme (`rw`) ve yönetim (`admin`) izinlerini destekler ve her üyenin yüklemesine izin verilen yol önekini ayrı ayrı belirtebilir (varsayılan olarak, paylaşımın kök dizinine yüklemeye izin verilir).
- **Ağlar arasında tam hızlı ve artımlı senkronizasyon**: Büyük dosyaların eşzamanlı iletimi, çok büyük küçük dosyaların bellekte toplanması ve yalnızca dosyalar kısmen değiştirildiğinde senkronize veri blokları.

## Rsync ile hız karşılaştırması

### Okyanus ötesi ağ tek kaynaklı gerçek ölçüm (Çin → Amerika Birleşik Devletleri, gecikme 260 ms)

| İletim senaryosu | Veri boyutu | modeli | `sync0` | `rsync -az` | Çarpanı artır |
| :--- | :---: | :---: | :---: | :---: | :---: |
| 500 küçük dosya | 1.91 MB | Tam senkronizasyon | 18.84 Mbps | 5.32 Mbps | 3.5x |
| 500 küçük dosya | 1.91 MB | Artımlı senkronizasyon | 27.15 Mbps | 3.83 Mbps | 7.1x |
| Kod ve arşiv paketleri (`.tar`) | 10.0 MB | Tam senkronizasyon | 139.70 Mbps | 24.97 Mbps | 5.6x |
| Kod ve arşiv paketleri (`.tar`) | 10.0 MB | Artımlı senkronizasyon | 83.38 Mbps | 26.02 Mbps | 3.2x |
| Video dosyası (`.mp4`) | 15.0 MB | Tam senkronizasyon | 54.06 Mbps | 27.04 Mbps | 2.0x |
| Video dosyası (`.mp4`) | 15.0 MB | Artımlı senkronizasyon | 216.83 Mbps | 21.51 Mbps | 10.1x |

### Okyanus ötesi ağ P2P çok düğümlü çekme testi (Çin → Amerika Birleşik Devletleri çift düğümleri, gecikme 260 ms)

Çok kaynaklı tohum düğümü şeritli eşzamanlı çekmenin ölçülen karşılaştırması:

| İletim senaryosu | Veri boyutu | Tek düğümlü tohumlama | İki düğümlü P2P eşzamanlı tohumlama | Çarpanı artır |
| :--- | :---: | :---: | :---: | :---: |
| 500 küçük dosya | 1.97 MB | 5.28 Mbps | 7.87 Mbps | 1.5x |
| Kod ve arşiv paketleri (`.tar`) | 30.0 MB | 56.35 Mbps | 83.17 Mbps | 1.5x |
| Video dosyası (`.mp4`) | 30.0 MB | 27.00 Mbps | 39.46 Mbps | 1.5x |

### Kıtalararası yüksek hızlı özel hattın gerçek ölçümü (Avrupa → Kuzey Amerika, gecikme 86 ms)

| İletim senaryosu | Veri boyutu | `sync0` | `rsync` | Çarpanı artır |
| :--- | :---: | :---: | :---: | :---: |
| 10.000 küçük dosya | 77.6 MB | 688 Mbps | 128 Mbps | 5.4x |
| Kod ve arşiv paketleri (`.tar`) | 512 MB | 2111 Mbps | 438 Mbps | 4.8x |
| Video dosyası (`.mp4`) | 512 MB | 556 Mbps | 338 Mbps | 1.6x |
| Artımlı senkronizasyon (%1 içeriği değiştirin) | 512 MB | 34133 Mbps | 2214 Mbps | 15.4x |

## Düzenlemek

### Yöntem 1: Kargo Yoluyla Kurulum

```bash
cargo install sync0
```

### Yöntem 2: Önceden derlenmiş ikili dosyaları indirin

İlgili sistem için (Linux, macOS, Windows, Android, FreeBSD, NetBSD'yi destekler) `.tar.xz` sıkıştırılmış paketini indirmek için [GitHub Sürümleri](https://github.com/webc-site/sync0/releases) adresine gidin, sıkıştırın ve `PATH` dizinine koyun:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Hızlı bir şekilde başlayın

`sync0`, varsayılan olarak yapılandırma dizini olarak `~/.config/sync0`'yi kullanır (başka dizinlere geçmeniz gerekiyorsa `-c <directory>` ekleyebilirsiniz).

### 1. Yerel düğüm adresini görüntüleyin ve kartviziti ayarlayın

```bash
# Yerel düğüm adresini görüntüleyin (anahtar ilk kez ~/.config/sync0 konumunda otomatik olarak oluşturulur)
sync0 id

# Kişisel bir düğüm kartviziti oluşturun (256 kelimelik kullanıcı adı, şehir, e-posta, web sitesi ve imzayı destekler)
sync0 profile -u alice --region SHA --email alice@example.com --url https://example.com -b "Shanghai node"
```

### 2. Birden fazla dosya paylaşım grubu yayınlayın ve grup üyesi izinlerini ayarlayın (`.nt` Yapılandırma)

`sync0`, [NestedText](https://nestedtext.org) (`.nt`) biçiminde yapılandırılmıştır. Her kullanıcının ayrı bir `.nt` yapılandırma dosyası vardır. Kullanıcı profili dizinini `config.nt` içindeki `users_dir` aracılığıyla belirtebilirsiniz (bu, bu dizindeki tüm `.nt` dosyalarını otomatik olarak içerir). Hem kullanıcı hem de yüklenmesine izin verilen yol öneki, sırasız bir liste (`-`) kullanır. **Yükleme yolu öneki doldurulmadığında, varsayılan olarak paylaşımın kök dizinine yükleme yapılmasına izin verilir**.

1. `~/.config/sync0/users/` dizinindeki her üye için ayrı `.nt` yapılandırma dosyaları oluşturun:

`~/.config/sync0/users/alice.nt` (salt okunur üye, hız sınırlı `80MB`, `rate` `GB`, `MB`, `KB` birimlerini destekler):

```yaml
id: 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c
perm: ro
rate: 80MB
```

`~/.config/sync0/users/bob.nt` (kısıtlı yükleme üyeleri: yalnızca `uploads/bob` ve `shared/common` dizinlerine yükleme yapılmasına izin verilir):

```yaml
id: 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f
perm: rw
write_prefix:
  - uploads/bob
  - shared/common
```

`~/.config/sync0/users/charlie.nt` (Varsayılan üye yükleme: `write_prefix` atlandığında, paylaşımın kök dizinine yüklemeye varsayılan olarak izin verilir):

```yaml
id: 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
perm: rw
```

2. `~/.config/sync0/config.nt`'de kullanıcı dizinini ve birden fazla dosya paylaşım grubunu yapılandırın:

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

Hizmeti paylaşmaya başlayın:

```bash
# Doğrudan başlatma (~/.config/sync0/config.nt ve user_dir dizinlerindeki tüm .nt kullanıcı yapılandırmalarını otomatik olarak yükler)
sync0 share --region SHA

# Veya birden fazla paylaşım grubunu hızlı bir şekilde yayınlayın ve grup üyelerini doğrudan komut satırı aracılığıyla yetkilendirin
sync0 share dataset:./data team-vault:./vault \
  --auth \
  --allow-ro 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c \
  --allow-rw 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f:uploads/bob,shared/common \
  --allow-rw 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
```

### 3. Tüm içeriği çekin veya birden fazla dosya ve dizin seçin (`sync0 pull`)

Paylaşım grubundaki tüm mevcut çekirdek düğümleri otomatik olarak elde etmek için herhangi bir eş düğüme bağlanın ve aynı anda birden fazla çekirdek düğümden çekim yapın:

```bash
# Veri kümesi paylaşım grubunun tamamını yerel ./local_data'ya çekin
sink0 pull dataset:// ./local_data -p <düğüm adresi>

# Çoklu seçim senkronizasyonu: docs/readme.md tek dosyasını ve asset/icons, src/core alt dizinlerini aynı anda çekin
sink0 pull dataset://docs/readme.md,assets/icons,src/core ./local_data -p <düğüm adresi>
```

### 4. Bir dosya veya dizini paylaşım grubuna yükleyin (`sync0 push`)

Yükleme izinlerine sahip üyeler (`rw` veya `admin`), yerel dosyaları veya dizinleri uzak paylaşım gruplarına yükleyebilir:

```bash
# Kök dizin yükleme iznine sahip üyeler: Doğrudan paylaşılan kök dizine yükleyin
sink0 push team-vault:// ./new_release -p <düğüm adresi>

# Yalnızca belirli bir önek için yetkilendirilen üyeler (örneğin, yüklemeler/bob): izin verilen yol önekine yükleme
sink0 push team-vault://uploads/bob/report.pdf ./report.pdf -p <düğüm adresi>
```

### 5. Otomatik senkronizasyona ve tohumlamaya abone olun (`sync0 sub`)

Belirli bir düğümün paylaşım grubuna abone olduktan sonra, yerel sunucu hem alıcı istemci hem de yeni tohumlama sunucusu olur; kaynak düğüm dosyası güncellendiğinde otomatik olarak senkronize edilir ve diğer düğümler için parçalı indirmeler sağlar:

```bash
# Tüm paylaşım grubuna abone olun ve otomatik olarak tohumlayın
sink0 alt veri kümesi:// ./local_mirror -p <düğüm adresi>

# Belirtilen dosyalara ve alt dizinlere abone olmak ve bunları tohumlamak için yalnızca çoklu seçim
sink0 sub dataset://models/llama.onnx,configs ./local_mirror -p <düğüm adresi>
```

### 6. Çoklu makine grubu eş senkronizasyonu (`sync0 group`)

Birden fazla makine aynı paylaşım grubuna katılır, her biri otomatik olarak kullanılabilir tohum düğümlerini keşfeder ve iki yönlü senkronizasyonu sürdürür (tam senkronizasyonu veya çoklu dosya ve dizin seçimini destekler):

```bash
# Gruba tam olarak katılın ve iki yönlü senkronizasyon
sink0 grubu takım-docs:// ./docs -p <düğüm adresi>

# Çoklu seçim senkronizasyon grubunda yalnızca wal dizini ve meta.json dosyası
sink0 grubu team-docs://wal,meta.json ./docs -p <düğüm adresi>
```

### 7. İki makine arasında noktadan noktaya doğrudan iletim (`send` / `recv`)

Bir grubu önceden yapılandırmanıza gerek yoktur ve dosyaları veya dizinleri doğrudan iki makine arasında geçici olarak aktarabilirsiniz:

```bash
# Alıcı izleme
sync0 recv 0.0.0.0:9000 ./backup

# Gönderen taraf, dosyaları veya dizinleri doğrudan aktarır (isteğe bağlı --delete ayna, fazla dosyaları temizlemek için --rate hız sınırı)
sync0 send ./project 192.168.1.10:9000
```

### 8. Çoklu dil değiştirme

`sync0` 75 dili destekler ve varsayılan olarak sistem dili ortamına göre otomatik olarak geçiş yapar; ayrıca `SYNC0_LANG` aracılığıyla manuel olarak da belirtilebilir:

```bash
SYNC0_LANG=en sync0 --help
SYNC0_LANG=ja sync0 --help
SYNC0_LANG=de sync0 --help
```