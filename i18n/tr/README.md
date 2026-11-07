# sync0

`sync0`, dosyaları ağlar arasında aktarmak, dizinleri aşamalı olarak senkronize etmek ve verileri aynı anda birden fazla makineye dağıtmak için kullanılan bir komut satırı aracıdır.

## Orijinal tasarım amacı

Ağ gecikmeleri ve paket kaybının olduğu makineler arası odalarda, ülkelerde veya ortamlarda, `rsync` ve `scp` gibi araçlar, bant genişliği kullanımını sınırlayan tek bağlantı sırası onayını temel alır; birden fazla makine aynı anda aynı sunucudan veri çektiğinde, kaynak sunucunun ağ kartı kolaylıkla bir darboğaz haline gelebilir.

`sync0`'in amacı iki sorunu çözmektir:

1. İki makine arasındaki dosya ve dizin senkronizasyonunun fiziksel bant genişliğinin tamamını çalıştırmasına izin verin ve değiştirilen kısmı yalnızca dosya kısmen değiştirildiğinde aktarın.
2. Birden fazla makine, aynı dizini senkronize ederken veri bloklarını birbirine aktarabilir, bu da kaynak sunucudaki yükü azaltır ve genel senkronizasyon süresini kısaltır.

## Rsync ile hız karşılaştırması

Test ortamı: Avrupa düğümü $\leftrightarrow$ Kuzey Amerika düğümü, gidiş-dönüş gecikmesi 86 ms, dizin tarama, veri tekilleştirme karşılaştırması, ağ iletimi ve disk yerleştirme doğrulaması da dahil olmak üzere tüm zaman alıcı işlemleri ölçer:

| İletim senaryosu | Veri boyutu | `sync0` | `rsync` | çoklu |
| :--- | :---: | :---: | :---: | :---: |
| 10.000 küçük dosya | 77.6 MB | 0.90 s (688 Mbps) | 4.86 s (128 Mbps) | 5.4x |
| Kod ve arşiv paketleri (`.tar`) | 512 MB | 1.94 s (2111 Mbps) | 9.36 s (438 Mbps) | 4.8x |
| Video dosyası (`.mp4`) | 512 MB | 7.36 s (556 Mbps) | 12.11 s (338 Mbps) | 1.6x |
| Artımlı senkronizasyon (%1 içeriği değiştirin) | 512 MB | 0.12 s | 1.85 s | 15.4x |

## Performans iyileştirme yöntemleri

- **UDP eşzamanlı iletim**: Veri bloklarını UDP aracılığıyla gönderin ve sonraki dosya aktarımlarını engelleyen tek bir paket kaybını önlemek için yalnızca kayıp parçaları yeniden iletin.
- **İçerik segmentasyonu ve tekilleştirme**: Dosyayı içeriğe göre veri bloklarına bölün ve kontrol değerini hesaplayın. Halihazırda alıcı tarafta bulunan aynı içerik, ağ çoğaltılmadan doğrudan yerel olarak yeniden kullanılır.
- **Sıkıştırmayı ağ hızına göre otomatik olarak ayarlayın**: Metin ve kod gibi sıkıştırılabilir içerik için sıkıştırmayı açın; CPU tüketimini azaltmak için videolar, resimler, sıkıştırılmış paketler veya yüksek hızlı LAN altında sıkıştırmayı otomatik olarak atlayın.
- **Yakındaki çok düğümlü çekme**: Birden fazla makine aynı dizini çektiğinde, veri blokları aynı şehirdeki indirilen düğümlerden eş zamanlı olarak veya daha düşük gecikmeyle çekilecektir.

## Düzenlemek

### Yöntem 1: Kargo Yoluyla Kurulum

```bash
cargo install sync0
```

### Yöntem 2: Önceden derlenmiş ikili dosyaları indirin

İlgili sistem için `.tar.xz` sıkıştırılmış paketi indirmek için [GitHub Sürümleri](https://github.com/webc-site/sync0/releases) adresine gidin (Linux x86_64/ARM64, macOS Apple Silicon/Intel, Windows x64/ARM64, Android ARM64/x86_64, FreeBSD, NetBSD'yi destekler). Sıkıştırmayı açtıktan sonra `sync0` dosyasını `PATH` dizinine yerleştirin:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Kullanım Kılavuzu

### 1. Dosyaları veya dizinleri iki makine arasında doğrudan aktarın

Alıcı makinede izlemeye başlayın ve alma ve kaydetme yolunu belirtin:

```bash
sync0 recv 0.0.0.0:9000 ./backup
```

Gönderen makineden tek bir dosyayı veya dizinin tamamını gönderin:

```bash
sync0 send ./project 192.168.1.10:9000
```

Yaygın olarak kullanılan parametreler:

```bash
# Alıcı uçta bulunan ancak gönderen uçta bulunmayan gereksiz dosyaları silin (yansıtma senkronizasyonu)
sync0 send ./project 192.168.1.10:9000 --delete

# Gönderim hızını sınırlayın (birim: bayt/saniye, örneğin 50 MB/s)
sync0 send ./project 192.168.1.10:9000 --rate 50000000

# Sessiz mod (ilerleme çubuğu gösterilmiyor)
sync0 send ./project 192.168.1.10:9000 -q
```

### 2. Paylaşılan dizinleri yayınlama ve birden fazla düğümden çekme

Kaynak sunucuda paylaşılan bir dizini (`dataset` adlı) yayınlayın:

```bash
sync0 share dataset ./data --region SHA
```

Paylaşılan dizini diğer makinelere çekin (aynı anda birden fazla düğüm adresini, aynı anda çekmek için virgülle ayırarak doldurabilirsiniz):

```bash
sync0 pull 10.0.0.1:9000,10.0.0.2:9000 dataset ./local_data
```

Paylaşılan dizinde yalnızca belirli bir alt dizini çekin:

```bash
sync0 pull 10.0.0.1:9000 dataset ./local_data --subpath 2026/logs
```

Yerel değişiklikleri uzak paylaşılan dizine geri gönderin:

```bash
sync0 push 10.0.0.1:9000 dataset ./local_data
```

### 3. Katalog otomatik güncellemelerine abone olun

Kaynak düğümün paylaşılan dizinine abone olun. Kaynak dizine bir dosya eklendiğinde veya değiştirildiğinde, bu dosya otomatik olarak yerel bilgisayarla senkronize edilecektir. Aynı zamanda, yerel makine otomatik olarak diğer makinelerin çekebileceği bir düğüm görevi görecektir:

```bash
sync0 sub 10.0.0.1:9000 dataset ./local_mirror
```

### 4. Çoklu makine gruplarının iki yönlü senkronizasyonu

Birden fazla makine aynı grup adına katılır ve herhangi bir düğüm dizinindeki değişiklikler, gruptaki diğer düğümlerle otomatik olarak senkronize edilir:

```bash
sync0 group team-docs ./docs 10.0.0.1:9000,10.0.0.2:9000
```

### 5. Düğüm kimliği ve kişisel kartvizit

Her düğüm, kullanıcı adının, şehrin, e-postanın, web sitesinin ve kişiselleştirilmiş imzanın (maksimum 256 karakter) ayarlanmasını destekler ve yerel özel anahtar imza kimlik doğrulamasını geçer:

```bash
# Yerel ortak anahtar kimliğini görüntüleyin (anahtarın kalıcı olacağı yapılandırma dizinini belirtin)
sync0 id -c ~/.config/sync0

# Kişiselleştirilmiş imzalara sahip düğüm kartvizitleri oluşturun ve görüntüleyin
sync0 profile -c ~/.config/sync0 \
  -u alice \
  --region SHA \
  --email alice@example.com \
  --url https://example.com \
  -b "Shanghai backup node"
```

`share`, `pull`, `sub`, `group` ayrıca şu profil parametreleriyle de başlatılabilir:

```bash
sync0 share dataset ./data -c ~/.config/sync0 -u alice --region SHA -b "Shanghai backup node"
```

### 6. Çoklu dil değiştirme

`sync0` 75 dili destekler. Varsayılan olarak, ilgili dildeki komut satırı yardımı ve istemleri, işletim sisteminin dil ortamına göre otomatik olarak görüntülenir; sistem dili desteklenen listede yoksa varsayılan olarak İngilizce kullanılır.

Dili manuel olarak belirlemek için ortam değişkenini `SYNC0_LANG` ayarlayın:

```bash
SYNC0_LANG=en sync0 --help
SYNC0_LANG=ja sync0 --help
SYNC0_LANG=de sync0 --help
```