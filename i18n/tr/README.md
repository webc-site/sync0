# sync0

`sync0`, `rsync`'nin yerine kullanılan, merkezi olmayan bir P2P dosya paylaşımı, gerçek zamanlı senkronizasyon ve çok kaynaklı dağıtım aracıdır - kıtalararası WAN'ın ölçülen hızı `rsync`'nin **5,89×** (tek nokta geometrik ortalama hızı **2,67×**, P2P çift kaynaklı geometrik ortalama hızı **3,16×**) ve okyanus ötesi yüksek gecikmeli bağlantıdır P2P hızlandırma **7,68×**'ye kadardır (Tam bağlantı 24 senaryolu kapsamlı geometrik ortalama hız **2,56×**).

Her makine hem bir sunucu hem de bir istemcidir: merkezi bir sunucuya veya genel ağ adresine gerek yoktur, otomatik delik açma, dağıtılmış ağ genel anahtar adresine dayanır ve iki makine arasında doğrudan iletimi, çoklu paylaşımlı grup izinli barındırmayı, talep üzerine çoklu alt dizin seçimini, milisaniye düzeyinde gerçek zamanlı artımlı senkronizasyonu ve P2P çok kaynaklı eşzamanlı hızlandırmayı destekler.

## Düzenlemek

### Yöntem 1: Kargo kurulumu

```bash
cargo install sync0
```

### Yöntem 2: Önceden derlenmiş ikili dosyaları indirin

Sisteme karşılık gelen (Linux, macOS, Windows, Android, iOS, FreeBSD'yi destekler) `.tar.xz` sıkıştırılmış paketini indirmek için [GitHub Sürümleri](https://github.com/webc-site/sync0/releases) adresine gidin, sıkıştırın ve `PATH` içine yerleştirin:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Hızlı bir şekilde başlayın

### 1. İki makine arasında sıfır konfigürasyonlu doğrudan iletim (`send` / `recv`)

Bir paylaşım grubu yapılandırmanıza gerek yok, dosyaları veya dizinleri iki makine arasında yüksek hızda doğrudan aktarın:

```bash
# Alıcı uç bağlantı noktasını dinler ve ./backup dosyasına yazar.
sync0 recv 0.0.0.0:9000 ./backup

# Gönderenden doğrudan iletim (isteğe bağlı --fazla dosyaları temizlemek için aynayı sil, --rate 50MB hız sınırı)
sync0 send ./project 192.168.1.10:9000
```

### 2. Gönderi paylaşımı ve isteğe bağlı çekme/yükleme (`share` / `pull` / `push`)

```bash
# Sunucu: Paylaşım grubunu bir komut satırıyla yayınlayın (başlangıçtan sonra IP'siz iroh düğüm adresinin çıktısını alın)
sync0 share dataset:./data

# İstemci: Veri kümesi paylaşım grubunun tamamını yerel ./local_data'ya çekin
sink0 pull dataset:// ./local_data -p <düğüm adresi>

# İstemci: İsteğe bağlı olarak birden fazla seçimi çekin (yalnızca belirtilen tek dosyaları ve alt dizinleri senkronize edin)
sink0 pull dataset://docs/readme.md,assets/icons,src/core ./local_data -p <düğüm adresi>

# İstemci: Dosyaları veya dizinleri yetkili paylaşım gruplarına yükleyin
sink0 push dataset://releases/v1 ./build_output -p <düğüm adresi>
```

### 3. Gerçek zamanlı abonelik tohumlama ve çok makineli iki yönlü senkronizasyon (`sub` / `group`)

Uzun bir bağlantı kurulduktan sonra, uzak uçta herhangi bir dosya **ekleme, değiştirme, silme, izinleri değiştirme veya değiştirme zamanı** meydana gelirse, yerel dizin gerçek zamanlı olarak milisaniye cinsinden eşzamanlı olarak güncellenecektir (yeni/değiştirilmiş büyük dosyalar yalnızca fark bloklarını çeker ve bunları yerinde günceller, dosyaları silmek, yerel dosyaları ve boş dizinleri otomatik olarak temizler ve yalnızca izinleri değiştirir veya sıfır veri bloklarını doğrudan zaman damgasına koyar):

- **`sync0 sub`(Tek yönlü gerçek zamanlı yansıtma + otomatik tohumlama)**: Yerel gerçek zamanlı, paylaşım ucundaki eklemeleri, silmeleri ve değişiklikleri takip eder ve diğer çeken düğümler için eşzamanlı hızlandırma sağlamak üzere otomatik olarak bir P2P tohum kaynağı görevi görür.
- **`sync0 group`(Çok makineli eşler arası iki yönlü gerçek zamanlı senkronizasyon)**: Gruptaki herhangi bir makinede dosyalar eklenirse, silinirse veya değiştirilirse, diğer tüm çevrimiçi düğümler gerçek zamanlı olarak eşzamanlı olarak güncellenir ve birbirleri için tohum görevi görür.

```bash
# Tüm paylaşım grubuna abone olun: uzaktan eklemeler, silmeler ve değişiklikler gerçek zamanlı olarak yerel olarak senkronize edilir ve otomatik olarak yayınlanır
sink0 alt veri kümesi:// ./local_mirror -p <düğüm adresi>

# İsteğe bağlı çoklu seçim aboneliği: yalnızca belirtilen tek dosyaları ve alt dizinleri gerçek zamanlı olarak senkronize edin ve tohumlayın
sink0 sub dataset://models/llama.onnx,configs ./local_mirror -p <düğüm adresi>

# Çok makineli eş grup iki yönlü gerçek zamanlı senkronizasyon (aynı zamanda tam veya virgül çoklu seçim yollarını da destekler)
sink0 grubu takım-docs:// ./docs -p <düğüm adresi>
```

### 4. Ekip çoklu paylaşım grubu ve üye izni yapılandırması (`.nt` yapılandırması)

Varsayılan yapılandırma dizini `~/.config/sync0` şeklindedir (`-c <directory>` aracılığıyla değiştirilebilir), minimalist [NestedText](https://nestedtext.org) (`.nt`) biçimi kullanılarak değiştirilebilir:
- **Genel ağ okumasının kısıtlanmasını destekler (yalnızca yetkili kullanıcılar okuyabilir)**: Paylaşım grubu bir `users:` üye listesiyle yapılandırıldığı sürece (veya komut satırı `--allow-r` / `--allow-rw` / `--allow-w` / `--auth` belirttiği sürece), sistem **otomatik olarak özel kimlik doğrulamayı açar (`auth: true`)** ve genel ağ anonim düğüm numaralandırmasını veya okunmasını yasaklar, ve P2P Tohumlama düğümlerine abone olmak aynı zamanda beyaz listeyi de devralacak ve yalnızca yetkili kullanıcıların okumasına izin verecektir; `users` ile yapılandırılmamış paylaşım grupları varsayılan olarak geneldir ve salt okunurdur (genel ve salt okunur olmak ve yalnızca belirli üyelere yapılan yüklemelere açık olmak istiyorsanız, açıkça `auth: false` ayarını yapabilirsiniz).
- **Ayrıntılı okuma-yazma ve alt dizin teslim izinleri**: Üyeler varsayılan olarak salt okunurdur (`r` açıkça yazılmasına gerek yoktur) ve `r` (salt okunur), `w` (yalnızca açılan kutuya yazma, okunamaz), `rw` (okuma-yazma) ve `write_prefix` (yükleme için izin verilen alt dizin önekini sınırlayın) destekler.

**Üye yapılandırma örneği (`~/.config/sync0/users/`):**

```yaml
# ~/.config/sync0/users/alice.nt (varsayılan salt okunur, hız sınırı 80 MB)
id: 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c
rate: 80MB
```

```yaml
# ~/.config/sync0/users/bob.nt (okuma ve yazma izinleri, yalnızca belirtilen alt dizine yüklemeye izin verilir)
id: 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f
perm: rw
write_prefix:
  - uploads/bob
  - shared/common
```

```yaml
# ~/.config/sync0/users/charlie.nt (yalnızca yazma teslim izni, okuma yasaktır)
id: 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
perm: w
```

**Paylaşım grubu yapılandırma örneği (`~/.config/sync0/config.nt`):**

```yaml
users_dir: ./users

groups:
  # Yapılandırılmamış kullanıcılar: varsayılan genel ağ salt okunurdur
  dataset:
    path: ./data
    rate: 50MB

  # Yapılandırılmış kullanıcılar: genel ağ okumasını otomatik olarak kısıtlayın (yalnızca alice ve bob okuyabilir, charlie yalnızca teslim edebilir ve yükleyebilir)
  team-vault:
    path: ./vault
    users:
      - alice
      - bob
      - charlie
```

Hizmeti başlatın (yapılandırma dosyasını yükleyin veya genel ağ okumasını kısıtlamak ve üyeleri yetkilendirmek için doğrudan komut satırı parametrelerini kullanın):

```bash
# Başlamak için ~/.config/sync0/config.nt dosyasını yükleyin
sync0 share --region SHA

# Veya yalnızca komut satırı aracılığıyla özel bir paylaşım grubu yayınlayın (genel ağ okumasını otomatik olarak kısıtlamak ve yalnızca yetkili kullanıcıların buna erişebilmesi için --allow-* belirtin)
sync0 share team-vault:./vault \
  --allow-r 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c \
  --allow-rw 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f:uploads/bob,shared/common \
  --allow-w 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
```

### 5. Düğüm adresi, kartvizit ve çoklu dil

```bash
# Yerel IP'siz düğüm adresini görüntüleyin (anahtar ilk çalıştırmada otomatik olarak oluşturulur)
sync0 id

# Ed25519 kendinden imzalı düğüm kartvizitini oluşturun (takma ad, şehir kodu, e-posta, web sitesi, tanıtım)
sync0 profile -u alice --region SHA --email alice@example.com -b "Shanghai node"

# Varsayılan olarak sistem dilini takip eden ve ortam değişkenleri aracılığıyla değiştirilebilen 75 yerleşik dil vardır.
SYNC0_LANG=en sync0 --help
```

## Performans ve temel özellikler

### Kıtalararası WAN gerçek ölçümü (Karşılaştırma `rsync`)

Gerçek ölçüm ortamı: Amerika Birleşik Devletleri `c1` → Avrupa `n1` (RTT 65 ms), veri seti Linux çekirdeğidir `linux-7.3.3`:

#### 1) Alakart mod vs `rsync`

| Veri seti | boyut | `sync0` Tek Nokta<br>(Mbps) | `rsync`<br>(Mbps) | Hızlandırma (`sync0` / `rsync`) |
| :--- | ---: | ---: | ---: | ---: |
| `linux-7.3.3.tar.xz`(sıkıştırılamaz) | 155 MB | **236.48** | 108.74 | **2,17×** |
| `linux-7.3.3.tar`(sıkıştırılabilir) | 1.6 GB | **843.83** | 385.06 | **2,19×** |
| `linux-7.3.3.tar`(yerel değişiklik artışı) | 1.6 GB | **8,014.15** | 1,361.72 | **5,89×** |
| `linux-7.3.3/` (95.979 küçük dosya) | 1.5 GB | **627.58** | 347.80 | **1,80×** |
| **geometrik ortalama** | — | **1,000.91** | 375.26 | **2,67×** |

#### 2) P2P çift kaynak modu vs `rsync`

| Veri seti | boyut | `sync0` P2P Çift Kaynak<br>(Mbps) | `rsync`<br>(Mbps) | Hızlandırma (`sync0` / `rsync`) |
| :--- | ---: | ---: | ---: | ---: |
| `linux-7.3.3.tar.xz`(sıkıştırılamaz) | 155 MB | **330.09** | 108.74 | **3,04×** |
| `linux-7.3.3.tar`(sıkıştırılabilir) | 1.6 GB | **1,551.86** | 385.06 | **4,03×** |
| `linux-7.3.3.tar`(yerel değişiklik artışı) | 1.6 GB | **5,474.17** | 1,361.72 | **4,02×** |
| `linux-7.3.3/` (95.979 küçük dosya) | 1.5 GB | **706.56** | 347.80 | **2,03×** |
| **geometrik ortalama** | — | **1,186.42** | 375.26 | **3,16×** |

> Not: P2P çift kaynak modunda, Avrupa düğümleri `s1` tohumlama için işbirliği yapar ve kıtalar arası omurga RTT'si 65 ms'dir.

### Temel ürün özellikleri

- **Genel anahtar adreslemede otomatik delik açma**: Merkezi bir sunucuya ve genel ağ adresine gerek yoktur, dağıtılmış ağ genel anahtar adresine dayalı olarak otomatik NAT delik açma.
- **Milisaniye düzeyinde gerçek zamanlı senkronizasyon ve meta veri farkındalığı**: Yerel dosya olayı izlemeyi (`FSEvents` / `inotify` / `ReadDirectoryChangesW`), gerçek zamanlı dosya ekleme, silme işlemlerini, POSIX izinlerini (`mode`) ve milisaniye düzeyinde değişiklik süresini (`mtime`) temel alır; Yalnızca izinler veya zaman değiştirildiğinde sıfır veri blok iletimi.
- **BT tarzı eşzamanlı indirme ve yükleme ile bölgesel benzeşim planlaması**: İndirme düğümünün, indirilecek tutarın tamamını beklemesine gerek yoktur. 128 bit karma kontrolünden geçen tüm blok aralıkları (`Op::Have`), diğer düğümlerin aynı anda çekebilmesi için hemen bellek/geçici havuzda kilitlenir. Dinamik PEX düğüm değişimi ve yavaş düğüm yarı ön alımı (İş Çalma) ile birlikte küme verimi en üst düzeye çıkarılır.
- **Kötü amaçlı düğümlere müdahale ve küme zehirlenmesine karşı bağışıklık**: yetkili kaynak listesi `root_hash` Kendi kendine tutarlı kilitleme + zorunlu donanım hızlandırma `gxhash128` her blok diske yerleştirilmeden ve harici olarak tohumlanmadan önce özet doğrulama; Kurcalanmış bir blok bulunursa, milisaniye düzeyindeki devre kesici, kötü niyetli düğümü karartacak ve otomatik olarak dürüst kaynaktan kurtaracaktır.
- **FastCDC yerinde artış ve küçük dosya toplama**: Büyük dosyaların kısmi değiştirilmesi için yalnızca diferansiyel bloklar aktarılır ve diske yerleştirilir; devasa küçük dosya belleği paketleme ve önek diferansiyel sıkıştırması, dosya bazında gidiş-dönüş yükünü ortadan kaldırır.
- **Elektrik kesintisinden sonra kendi kendini iyileştirme ve yeniden başlatmanın ardından yeniden başlama**: Redis benzeri RDB + AOF karma formatı (`.s0_subs.bin`), tohumlama durumunu ve dosya listesini sürdürmek için kullanılır, günlük değişiklikler mikrosaniyeler içinde eklenir ve düzenli olarak otomatik olarak yeniden yazılır ve düzenlenir. Sunucu yeniden başlatıldıktan sonra çevrimdışı değişiklikler otomatik olarak tohumlama düğümüne iletilir.

## Teknik mimari ve dokümantasyon

- **Çift düzlemli dekuplaj protokolü**:
  - **Kontrol ve gerçek zamanlı değişiklik yüzeyi (`iroh` QUIC / TCP, ilk bayt `PROTO_VERSION` sürüm anlaşması) **: Ed25519 kartvizit doğrulamasından, izin doğrulamasından, Swarm tohum keşfinden ve `u64::MAX` düzey toplu değişiklik akışı toplu işlemi ve `mzstd` sıkıştırılmış `SubPush` gerçek zamanlı gönderim (`FsOp::Upsert` / `FsOp::Remove` / `FsOp::Meta` Üç temel öğe).
  - **Veri dağıtım yüzeyi (yerel UDP + `compio` eşzamansız G/Ç) **: 20B kompakt başlık, 18B TACK periyodik geri bildirim, BBRv3/Copa hibrit tıkanıklık kontrolü, RACK-TLP paket kaybı kurtarma, ilk blok Shannon entropi uyarlamalı Zstd sıkıştırma bypass'ı ve `gxhash128` blok uçtan uca doğrulama.
- **Daha fazla bilgi edin**:
  - [Performans ölçüm raporu ve çoğaltma komut dosyası](https://github.com/webc-site/sync0/blob/main/doc/zh/benchmark.md)
  - [Sistem mimarisi tasarımı](https://github.com/webc-site/sync0/blob/main/doc/zh/design_v4.md)
  - [İkili Tel Protokolü ve Kalıcılık Belirtimi](https://github.com/webc-site/sync0/blob/main/doc/zh/proto.md)
  - [Kimlik doğrulama, yetki kontrolü ve bütünlük doğrulama](https://github.com/webc-site/sync0/blob/main/doc/zh/crypto.md)