# sync0

`sync0` là một công cụ phân phối đa nguồn, đồng bộ hóa thời gian thực và chia sẻ tệp P2P phi tập trung được sử dụng để thay thế `rsync` - tốc độ đo được của mạng diện rộng xuyên lục địa lên tới **6,95×** của `rsync` (tốc độ trung bình hình học một điểm **2,01×**, tốc độ trung bình hình học nguồn kép P2P **3,33×**) và quá trình đồng bộ hóa các tệp nhỏ lớn trên khắp các đại dương nhanh như **7,19×**.

Mỗi máy vừa là máy chủ vừa là máy khách: không cần máy chủ trung tâm hoặc địa chỉ mạng công cộng, tính năng đục lỗ tự động dựa trên địa chỉ khóa công khai của mạng phân tán và hỗ trợ truyền trực tiếp giữa hai máy, lưu trữ quyền nhóm chia sẻ nhiều lần, nhiều lựa chọn thư mục con theo yêu cầu, đồng bộ hóa gia tăng theo thời gian thực ở mức mili giây và tăng tốc đồng thời đa nguồn P2P.

## Cài đặt

### Cách 1: Lắp đặt hàng hóa

```bash
cargo install sync0
```

### Phương pháp 2: Tải xuống các tệp nhị phân được biên dịch sẵn

Vào [GitHub Releases](https://github.com/webc-site/sync0/releases) để tải xuống gói nén `.tar.xz` tương ứng với hệ thống (hỗ trợ Linux, macOS, Windows, Android, iOS, FreeBSD), giải nén và cho vào `PATH`:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Bắt đầu nhanh chóng

### 1. Truyền trực tiếp không cấu hình giữa hai máy (`send` / `recv`)

Không cần cấu hình nhóm chia sẻ, truyền trực tiếp file, thư mục giữa 2 máy với tốc độ cao:

```bash
# Đầu nhận lắng nghe trên cổng và ghi vào ./backup
sync0 recv 0.0.0.0:9000 ./backup

# Truyền trực tiếp từ người gửi (tùy chọn --delete mirror để dọn sạch các tập tin thừa, --rate giới hạn tốc độ 50MB)
sync0 send ./project 192.168.1.10:9000
```

### 2. Chia sẻ bài đăng và kéo/tải lên theo yêu cầu (`share` / `pull` / `push`)

```bash
# Máy chủ: Xuất bản nhóm chia sẻ bằng một dòng lệnh (xuất địa chỉ nút iroh không có IP sau khi khởi động)
sync0 share dataset:./data

# Máy khách: Kéo toàn bộ nhóm chia sẻ tập dữ liệu về ./local_data cục bộ
tập dữ liệu kéo sync0:// ./local_data -p <địa chỉ nút>

# Máy khách: Kéo nhiều lựa chọn theo yêu cầu (chỉ đồng bộ hóa các tệp và thư mục con được chỉ định)
tập dữ liệu kéo sync0://docs/readme.md,assets/icons,src/core ./local_data -p <địa chỉ nút>

# Máy khách: Tải tệp hoặc thư mục lên các nhóm chia sẻ được ủy quyền
tập dữ liệu đẩy sync0://releases/v1 ./build_output -p <địa chỉ nút>
```

### 3. Tạo mầm đăng ký theo thời gian thực và đồng bộ hóa hai chiều nhiều máy (`sub` / `group`)

Sau khi thiết lập kết nối dài, nếu bất kỳ tệp nào **thêm, sửa đổi, xóa, thay đổi quyền hoặc thời gian sửa đổi** xảy ra ở đầu từ xa, thư mục cục bộ sẽ được cập nhật đồng bộ tính bằng mili giây trong thời gian thực (các tệp lớn mới/được sửa đổi sẽ chỉ kéo các khối khác biệt và cập nhật chúng tại chỗ, các tệp bị xóa sẽ tự động dọn sạch các tệp cục bộ và thư mục trống và chỉ trực tiếp thay đổi quyền hoặc khối dữ liệu bằng 0 dấu thời gian):

- **`sync0 sub`(Phản chiếu thời gian thực một chiều + gieo hạt tự động)**: Thời gian thực cục bộ tuân theo các thao tác bổ sung, xóa và sửa đổi của đầu chia sẻ và tự động đóng vai trò là nguồn hạt giống P2P để cung cấp khả năng tăng tốc đồng thời cho các nút kéo khác.
- **`sync0 group`(Đồng bộ hóa thời gian thực hai chiều ngang hàng nhiều máy)**: Nếu tệp được thêm, xóa hoặc sửa đổi trên bất kỳ máy nào trong nhóm, tất cả các nút trực tuyến khác sẽ được cập nhật đồng bộ trong thời gian thực và đóng vai trò là hạt giống cho nhau.

```bash
# Đăng ký toàn bộ nhóm chia sẻ: các bổ sung, xóa và sửa đổi từ xa được đồng bộ hóa cục bộ theo thời gian thực và tự động được gieo hạt
tập dữ liệu phụ sync0:// ./local_mirror -p <địa chỉ nút>

# Đăng ký nhiều lựa chọn theo yêu cầu: chỉ đồng bộ hóa và tạo giống các tệp và thư mục con được chỉ định trong thời gian thực
tập dữ liệu phụ sync0://models/llama.onnx,configs ./local_mirror -p <địa chỉ nút>

# Đồng bộ hóa thời gian thực hai chiều của nhóm ngang hàng nhiều máy (cũng hỗ trợ các đường dẫn đa lựa chọn đầy đủ hoặc dấu phẩy)
nhóm sync0 team-docs:// ./docs -p <địa chỉ nút>
```

### 4. Cấu hình quyền thành viên và nhóm chia sẻ nhiều nhóm (cấu hình `.nt`)

Thư mục cấu hình mặc định là `~/.config/sync0` (có thể được chuyển qua `-c <directory>`), sử dụng định dạng [NestedText](https://nestedtext.org) (`.nt`) tối giản:
- **Hỗ trợ hạn chế đọc mạng công cộng (chỉ người dùng được ủy quyền mới có thể đọc)**: Miễn là nhóm chia sẻ được định cấu hình với danh sách thành viên `users:` (hoặc dòng lệnh chỉ định `--allow-r` / `--allow-rw` / `--allow-w` / `--auth`), hệ thống **tự động bật xác thực riêng tư (`auth: true`)** và cấm liệt kê hoặc đọc nút ẩn danh của mạng công cộng và việc đăng ký P2P với các nút gieo hạt cũng sẽ đồng thời kế thừa danh sách trắng, chỉ cho phép người dùng được ủy quyền đọc; các nhóm chia sẻ không được định cấu hình bằng `users` theo mặc định là công khai và chỉ đọc (nếu bạn muốn công khai và chỉ đọc cũng như chỉ mở để tải lên đối với các thành viên được chỉ định, bạn có thể đặt `auth: false` một cách rõ ràng).
- **Quyền đọc-ghi và phân phối thư mục con chi tiết**: Theo mặc định, các thành viên ở chế độ chỉ đọc (`r` không cần phải viết rõ ràng) và hỗ trợ `r` (chỉ đọc), `w` (chỉ ghi vào hộp thả, không thể đọc được), `rw` (đọc-ghi) và `write_prefix` (giới hạn tiền tố thư mục con được phép tải lên).

**Ví dụ về cấu hình thành viên (`~/.config/sync0/users/`):**

```yaml
# ~/.config/sync0/users/alice.nt (chỉ đọc mặc định, giới hạn tốc độ 80 MB)
id: 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c
rate: 80MB
```

```yaml
# ~/.config/sync0/users/bob.nt (quyền đọc và ghi, chỉ được phép tải lên thư mục con được chỉ định)
id: 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f
perm: rw
write_prefix:
  - uploads/bob
  - shared/common
```

```yaml
# ~/.config/sync0/users/charlie.nt (chỉ quyền gửi, cấm đọc)
id: 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
perm: w
```

**Ví dụ về cấu hình nhóm chia sẻ (`~/.config/sync0/config.nt`):**

```yaml
users_dir: ./users

groups:
  # Chưa cấu hình users: mặc định công khai chỉ đọc
  dataset:
    path: ./data
    rate: 50MB

  # Đã cấu hình users: tự động hạn chế đọc công khai (chỉ alice, bob có thể đọc, charlie chỉ có thể gửi và tải lên)
  team-vault:
    path: ./vault
    users:
      - alice
      - bob
      - charlie
```

Khởi động dịch vụ (tải tệp cấu hình hoặc sử dụng trực tiếp các tham số dòng lệnh để hạn chế việc đọc mạng công cộng và ủy quyền cho thành viên):

```bash
# Tải ~/.config/sync0/config.nt để bắt đầu
sync0 share --region SHA

# Hoặc xuất bản một nhóm chia sẻ riêng tư thông qua dòng lệnh thuần túy (chỉ định --allow-* để tự động hạn chế việc đọc mạng công cộng và chỉ những người dùng được ủy quyền mới có thể truy cập nó)
sync0 share team-vault:./vault \
  --allow-r 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c \
  --allow-rw 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f:uploads/bob,shared/common \
  --allow-w 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
```

### 5. Địa chỉ nút, danh thiếp và đa ngôn ngữ

```bash
# Xem địa chỉ nút không có IP cục bộ (khóa được tạo tự động cho lần chạy đầu tiên)
sync0 id

# Tạo danh thiếp nút tự ký Ed25519 (biệt danh, mã thành phố, email, trang web, phần giới thiệu)
sync0 profile -u alice --region SHA --email alice@example.com -b "Shanghai node"

# Có 75 ngôn ngữ tích hợp, tuân theo ngôn ngữ hệ thống theo mặc định và có thể được chuyển đổi thông qua các biến môi trường.
SYNC0_LANG=en sync0 --help
```

## Hiệu suất và tính năng cốt lõi

### Đo lường thực tế mạng WAN xuyên lục địa (So sánh `rsync`)

Môi trường đo thực tế: Hoa Kỳ `c1` → Châu Âu `n1` (RTT 65 ms), bộ dữ liệu là nhân Linux `linux-7.3.3`:

#### 1) Chế độ gọi món so với `rsync`

| Tập dữ liệu | kích cỡ | `sync0` Điểm đơn<br>(Mbps) | `rsync`<br>(Mbps) | Tăng tốc (`sync0` / `rsync`) |
| :--- | ---: | ---: | ---: | ---: |
| `linux-7.3.3.tar.xz`(không nén được) | 155 MB | **121.07** | 107.89 | **1,12**** |
| `linux-7.3.3.tar`(có thể nén được) | 1.6 GB | **442.07** | 506.62 | **0,87***** |
| `linux-7.3.3.tar`(gia tăng thay đổi cục bộ) | 1.6 GB | **8,123.87** | 1,169.49 | **6,95**** |
| `linux-7.3.3/` (95.979 tệp nhỏ) | 1.5 GB | **565.49** | 235.27 | **2,40**** |
| **trung bình hình học** | — | **704.17** | 350.19 | **2,01**** |

#### 2) Chế độ nguồn kép P2P so với `rsync`

| Tập dữ liệu | kích cỡ | `sync0` Nguồn kép P2P<br>(Mbps) | `rsync`<br>(Mbps) | Tăng tốc (`sync0` / `rsync`) |
| :--- | ---: | ---: | ---: | ---: |
| `linux-7.3.3.tar.xz`(không nén được) | 155 MB | **466.85** | 107.89 | **4,33×** |
| `linux-7.3.3.tar`(có thể nén được) | 1.6 GB | **992.17** | 506.62 | **1,96**** |
| `linux-7.3.3.tar`(gia tăng thay đổi cục bộ) | 1.6 GB | **5,666.84** | 1,169.49 | **4,85**** |
| `linux-7.3.3/` (95.979 tệp nhỏ) | 1.5 GB | **703.30** | 235.27 | **2,99***** |
| **trung bình hình học** | — | **1,165.63** | 350.19 | **3,33**** |

> Lưu ý: Ở chế độ nguồn kép P2P, các nút Châu Âu `s1` cộng tác để gieo hạt và RTT đường trục xuyên lục địa là 65 mili giây.

### Tính năng sản phẩm cốt lõi

- **Đục lỗ khóa công khai tự động**: Không cần máy chủ trung tâm và địa chỉ mạng công cộng, việc đục lỗ NAT tự động dựa trên địa chỉ khóa chung của mạng phân tán.
- **Nhận thức về siêu dữ liệu và đồng bộ hóa thời gian thực ở cấp độ mili giây**: Dựa trên giám sát sự kiện tệp gốc (`FSEvents` / `inotify` / `ReadDirectoryChangesW`), việc thêm, xóa tệp theo thời gian thực, quyền POSIX (`mode`) và thời gian sửa đổi ở mức mili giây (`mtime`); truyền khối dữ liệu bằng 0 khi chỉ sửa đổi quyền hoặc thời gian.
- **Tải xuống và tải lên đồng thời kiểu BT và lập lịch mối quan hệ theo khu vực**: Nút tải xuống không cần phải đợi tải xuống toàn bộ số lượng. Tất cả các khoảng thời gian khối (`Op::Have`) đã vượt qua kiểm tra băm 128 bit sẽ ngay lập tức bị khóa trong bộ nhớ/nhóm tạm thời để các nút khác kéo đồng thời. Cùng với việc trao đổi nút PEX động và quyền ưu tiên một nửa nút chậm (Đánh cắp công việc), thông lượng của cụm được tối đa hóa.
- **Miễn nhiễm chống giả mạo nút độc hại và nhiễm độc cụm**: danh sách nguồn có thẩm quyền `root_hash` Khóa tự nhất quán + tăng tốc phần cứng bắt buộc `gxhash128` xác minh thông báo trước khi mỗi khối được đặt trên đĩa và được gieo hạt bên ngoài; nếu tìm thấy một khối giả mạo, bộ ngắt mạch cấp mili giây sẽ tắt nút độc hại và tự động khôi phục từ nguồn trung thực.
- **Tập hợp tệp nhỏ và tăng tại chỗ FastCDC**: Chỉ các khối vi sai được chuyển để sửa đổi một phần các tệp lớn và được đặt trên đĩa; Việc đóng gói bộ nhớ tệp nhỏ và nén vi sai tiền tố sẽ loại bỏ chi phí cho từng tệp.
- **Tự phục hồi sau khi mất điện và tiếp tục hoạt động sau khi khởi động lại**: Định dạng kết hợp RDB + AOF giống Redis (`.s0_subs.bin`) được sử dụng để duy trì trạng thái gieo mầm và danh sách tệp, các thay đổi hàng ngày được thêm vào sau micro giây và được tự động viết lại và sắp xếp thường xuyên. Sau khi máy chủ được khởi động lại, các thay đổi ngoại tuyến sẽ tự động được đẩy tới nút gieo hạt.

## Kiến trúc kỹ thuật và tài liệu

- **Giao thức tách mặt phẳng kép**:
  - **Bề mặt điều khiển và thay đổi theo thời gian thực (`iroh` QUIC / TCP, byte đầu tiên `PROTO_VERSION` phiên bản bắt tay) **: Chịu trách nhiệm xác minh danh thiếp Ed25519, xác minh quyền, phát hiện Swarm Seed và hỗ trợ cho việc phân nhóm phát trực tuyến thay đổi hàng loạt cấp độ `u64::MAX` và `mzstd` nén `SubPush` đẩy thời gian thực (`FsOp::Upsert` / `FsOp::Remove` / `FsOp::Meta` Ba nguyên thủy).
  - **Bề mặt phân phối dữ liệu (UDP gốc + `compio` I/O không đồng bộ) **: Tiêu đề nhỏ gọn 20B, phản hồi định kỳ 18B TACK, kiểm soát tắc nghẽn kết hợp BBRv3/Copa, phục hồi mất gói RACK-TLP, bỏ qua nén Zstd thích ứng entropy Shannon khối đầu tiên và chặn `gxhash128` chặn xác minh từ đầu đến cuối.
- **Tìm hiểu thêm**:
  - [Báo cáo đo lường hiệu suất và tập lệnh sao chép](https://github.com/webc-site/sync0/blob/main/doc/zh/benchmark.md)
  - [Thiết kế kiến ​​trúc hệ thống](https://github.com/webc-site/sync0/blob/main/doc/zh/design_v4.md)
  - [Giao thức dây nhị phân và đặc điểm kỹ thuật liên tục](https://github.com/webc-site/sync0/blob/main/doc/zh/proto.md)
  - [Xác thực danh tính, kiểm soát quyền hạn và xác minh tính toàn vẹn](https://github.com/webc-site/sync0/blob/main/doc/zh/crypto.md)