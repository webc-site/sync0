# sync0

`sync0` là công cụ chia sẻ tệp ngang hàng (P2P) và đồng bộ hóa nhiều máy cạnh tranh với Resilio Sync.

Mỗi máy vừa là máy chủ vừa là máy khách: bất kỳ nút nào cũng có thể xuất bản nhiều nhóm chia sẻ tệp. Sau khi các nút khác tham gia, chúng có thể tự động lấy các nút giống có sẵn từ máy chủ ngang hàng và lấy chúng đồng thời từ nhiều nguồn. Sau khi đồng bộ hóa hoàn tất, các hạt giống sẽ được tạo tự động cho nhiều nút hơn để tăng tốc độ tải xuống.

## Điểm nổi bật cốt lõi

- **Mọi người đều vừa là máy chủ vừa là máy khách**: Có thể so sánh với Resilio Sync, không phụ thuộc vào máy chủ trung tâm. Bất kỳ nút nào cũng có thể tạo nhiều nhóm chia sẻ tệp và cũng có thể tham gia nhóm chia sẻ của người khác; các nút tự động trao đổi và thu được các nút giống có sẵn để đạt được khả năng kéo đồng thời và đồng bộ hóa thời gian thực của nhiều máy.
- **Địa chỉ nút không có IP**: Theo mặc định, khóa nút và `base64url` địa chỉ `iroh` được mã hóa sẽ tự động được tạo trong `~/.config/sync0` mà không cần phải nhớ hoặc hiển thị IP công khai.
- **Đồng bộ hóa nhiều lựa chọn theo yêu cầu**: Hỗ trợ đồng bộ hóa toàn bộ nhóm chia sẻ và cũng hỗ trợ đồng bộ hóa theo yêu cầu bằng cách chọn nhiều tệp hoặc thư mục con cùng một lúc.
- **Quyền phân cấp của thành viên nhóm và kiểm soát thư mục tải lên**: Hỗ trợ các quyền chỉ đọc (`ro`), tải lên đọc-ghi (`rw`) và quản lý (`admin`), đồng thời có thể chỉ định riêng tiền tố đường dẫn mà mỗi thành viên được phép tải lên (theo mặc định, được phép tải lên thư mục gốc của phần chia sẻ).
- **Đồng bộ hóa tăng dần và tốc độ đầy đủ trên các mạng**: Truyền đồng thời các tệp lớn, tổng hợp các tệp nhỏ lớn trong bộ nhớ và chỉ các khối dữ liệu được đồng bộ hóa khi các tệp được sửa đổi một phần.

## So sánh tốc độ với rsync

### Đo thực tế nguồn đơn mạng xuyên đại dương (Trung Quốc → Hoa Kỳ, độ trễ 260 ms)

| Kịch bản truyền tải | Kích thước dữ liệu | người mẫu | `sync0` | `rsync -az` | Tăng số nhân |
| :--- | :---: | :---: | :---: | :---: | :---: |
| 500 tập tin nhỏ | 1.91 MB | Đồng bộ hóa hoàn toàn | 18.84 Mbps | 5.32 Mbps | 3.5x |
| 500 tập tin nhỏ | 1.91 MB | Đồng bộ hóa gia tăng | 27.15 Mbps | 3.83 Mbps | 7.1x |
| Gói mã và lưu trữ (`.tar`) | 10.0 MB | Đồng bộ hóa hoàn toàn | 139.70 Mbps | 24.97 Mbps | 5.6x |
| Gói mã và lưu trữ (`.tar`) | 10.0 MB | Đồng bộ hóa gia tăng | 83.38 Mbps | 26.02 Mbps | 3.2x |
| Tệp video (`.mp4`) | 15.0 MB | Đồng bộ hóa hoàn toàn | 54.06 Mbps | 27.04 Mbps | 2.0x |
| Tệp video (`.mp4`) | 15.0 MB | Đồng bộ hóa gia tăng | 216.83 Mbps | 21.51 Mbps | 10.1x |

### Thử nghiệm kéo đa nút P2P của mạng xuyên đại dương (Trung Quốc → Nút kép của Hoa Kỳ, độ trễ 260 ms)

Đo so sánh lực kéo đồng thời của nút hạt giống đa nguồn có sọc:

| Kịch bản truyền tải | Kích thước dữ liệu | Hạt giống nút đơn | Gieo đồng thời hai nút P2P | Tăng số nhân |
| :--- | :---: | :---: | :---: | :---: |
| 500 tập tin nhỏ | 1.97 MB | 5.28 Mbps | 7.87 Mbps | 1.5x |
| Gói mã và lưu trữ (`.tar`) | 30.0 MB | 56.35 Mbps | 83.17 Mbps | 1.5x |
| Tệp video (`.mp4`) | 30.0 MB | 27.00 Mbps | 39.46 Mbps | 1.5x |

### Đo thực tế đường dây chuyên dụng tốc độ cao xuyên lục địa (Châu Âu → Bắc Mỹ, độ trễ 86 ms)

| Kịch bản truyền tải | Kích thước dữ liệu | `sync0` | `rsync` | Tăng số nhân |
| :--- | :---: | :---: | :---: | :---: |
| 10.000 tập tin nhỏ | 77.6 MB | 688 Mbps | 128 Mbps | 5.4x |
| Gói mã và lưu trữ (`.tar`) | 512 MB | 2111 Mbps | 438 Mbps | 4.8x |
| Tệp video (`.mp4`) | 512 MB | 556 Mbps | 338 Mbps | 1.6x |
| Đồng bộ hóa gia tăng (sửa đổi nội dung 1%) | 512 MB | 34133 Mbps | 2214 Mbps | 15.4x |

## Cài đặt

### Cách 1: Cài đặt qua Cargo

```bash
cargo install sync0
```

### Phương pháp 2: Tải xuống các tệp nhị phân được biên dịch sẵn

Đi tới [GitHub Releases](https://github.com/webc-site/sync0/releases) để tải xuống gói nén `.tar.xz` cho hệ thống tương ứng (hỗ trợ Linux, macOS, Windows, Android, FreeBSD, NetBSD), giải nén và cho vào thư mục `PATH`:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Bắt đầu nhanh chóng

`sync0` sử dụng `~/.config/sync0` làm thư mục cấu hình theo mặc định (nếu bạn cần chuyển sang các thư mục khác, bạn có thể thêm `-c <directory>`).

### 1. Xem địa chỉ nút cục bộ và cài đặt danh thiếp

```bash
# Xem địa chỉ nút cục bộ (khóa được tạo tự động trong ~/.config/sync0 lần đầu tiên)
sync0 id

# Tạo danh thiếp nút cá nhân (hỗ trợ tên người dùng, thành phố, email, trang web và chữ ký trong vòng 256 từ)
sync0 profile -u alice --region SHA --email alice@example.com --url https://example.com -b "Shanghai node"
```

### 2. Xuất bản nhiều nhóm chia sẻ tệp và đặt quyền cho thành viên nhóm (`.nt` Cấu hình)

`sync0` được định cấu hình ở định dạng [NestedText](https://nestedtext.org) (`.nt`). Mỗi người dùng có một tệp cấu hình `.nt` riêng. Bạn có thể chỉ định thư mục hồ sơ người dùng thông qua `users_dir` trong `config.nt` (tự động bao gồm tất cả các tệp `.nt` trong thư mục này). Cả người dùng và tiền tố đường dẫn được phép tải lên đều sử dụng danh sách không có thứ tự (`-`). **Khi tiền tố đường dẫn tải lên không được điền, việc tải lên thư mục gốc của nội dung chia sẻ được cho phép theo mặc định**.

1. Tạo các tệp cấu hình `.nt` riêng cho từng thành viên trong thư mục `~/.config/sync0/users/`:

`~/.config/sync0/users/alice.nt` (thành viên chỉ đọc, tỷ lệ giới hạn `80MB`, `rate` hỗ trợ các đơn vị `GB`, `MB`, `KB`):

```yaml
id: 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c
perm: ro
rate: 80MB
```

`~/.config/sync0/users/bob.nt` (hạn chế thành viên tải lên: chỉ được phép tải lên thư mục `uploads/bob` và `shared/common`):

```yaml
id: 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f
perm: rw
write_prefix:
  - uploads/bob
  - shared/common
```

`~/.config/sync0/users/charlie.nt` (Tải lên thành viên mặc định: Khi `write_prefix` bị bỏ qua, việc tải lên thư mục gốc của phần chia sẻ được cho phép theo mặc định):

```yaml
id: 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
perm: rw
```

2. Định cấu hình thư mục người dùng và nhiều nhóm chia sẻ tệp trong `~/.config/sync0/config.nt`:

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

Bắt đầu dịch vụ chia sẻ:

```bash
# Khởi động trực tiếp (tự động tải tất cả cấu hình người dùng .nt trong thư mục ~/.config/sync0/config.nt và user_dir)
sync0 share --region SHA

# Hoặc nhanh chóng xuất bản nhiều nhóm chia sẻ và ủy quyền thành viên nhóm trực tiếp thông qua dòng lệnh
sync0 share dataset:./data team-vault:./vault \
  --auth \
  --allow-ro 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c \
  --allow-rw 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f:uploads/bob,shared/common \
  --allow-rw 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
```

### 3. Kéo tất cả nội dung hoặc chọn nhiều tệp và thư mục (`sync0 pull`)

Kết nối với bất kỳ nút ngang hàng nào để tự động lấy tất cả các nút giống có sẵn trong nhóm chia sẻ và kéo đồng thời từ nhiều nút giống:

```bash
# Kéo toàn bộ nhóm chia sẻ tập dữ liệu về ./local_data cục bộ
tập dữ liệu kéo sync0:// ./local_data -p <địa chỉ nút>

# Đồng bộ hóa nhiều lựa chọn: kéo đồng thời các tệp docs/readme.md và thư mục con assets/icon, src/core
tập dữ liệu kéo sync0://docs/readme.md,assets/icons,src/core ./local_data -p <địa chỉ nút>
```

### 4. Tải tệp hoặc thư mục lên nhóm chia sẻ (`sync0 push`)

Các thành viên có quyền tải lên (`rw` hoặc `admin`) có thể tải các tệp hoặc thư mục cục bộ lên các nhóm chia sẻ từ xa:

```bash
# Thành viên có quyền tải lên thư mục gốc: Tải trực tiếp lên thư mục gốc được chia sẻ
sync0 push team-vault:// ./new_release -p <địa chỉ nút>

# Chỉ các thành viên được ủy quyền cho một tiền tố cụ thể (chẳng hạn như tải lên/bob): tải lên tiền tố đường dẫn được phép
sync0 push team-vault://uploads/bob/report.pdf ./report.pdf -p <địa chỉ nút>
```

### 5. Đăng ký đồng bộ hóa và gieo hạt tự động (`sync0 sub`)

Sau khi đăng ký vào nhóm chia sẻ của một nút nhất định, máy chủ cục bộ vừa là máy khách nhận vừa là máy chủ gieo hạt mới; khi tệp nút nguồn được cập nhật, nó sẽ tự động được đồng bộ hóa và cung cấp các bản tải xuống theo khối cho các nút khác:

```bash
# Đăng ký theo dõi toàn bộ nhóm chia sẻ và tự động gieo hạt
tập dữ liệu phụ sync0:// ./local_mirror -p <địa chỉ nút>

# Chỉ chọn nhiều lựa chọn để đăng ký các tệp và thư mục con được chỉ định và chọn chúng
tập dữ liệu phụ sync0://models/llama.onnx,configs ./local_mirror -p <địa chỉ nút>
```

### 6. Đồng bộ hóa ngang hàng nhiều nhóm máy (`sync0 group`)

Nhiều máy tham gia cùng một nhóm chia sẻ, mỗi máy tự động phát hiện các nút giống có sẵn và duy trì đồng bộ hai chiều (hỗ trợ đồng bộ đầy đủ hoặc chọn nhiều file, thư mục):

```bash
# Tham gia nhóm đầy đủ và đồng bộ hai chiều
nhóm sync0 team-docs:// ./docs -p <địa chỉ nút>

# Chỉ có thư mục wal và tệp meta.json trong nhóm đồng bộ hóa nhiều lựa chọn
nhóm sync0 team-docs://wal,meta.json ./docs -p <địa chỉ nút>
```

### 7. Truyền trực tiếp điểm-điểm giữa hai máy (`send` / `recv`)

Không cần phải định cấu hình trước một nhóm và bạn có thể tạm thời truyền tệp hoặc thư mục trực tiếp giữa hai máy:

```bash
# Giám sát máy thu
sync0 recv 0.0.0.0:9000 ./backup

# Đầu gửi trực tiếp chuyển các tập tin hoặc thư mục (tùy chọn --delete mirror để dọn sạch các tập tin dư thừa, --rate giới hạn tốc độ)
sync0 send ./project 192.168.1.10:9000
```

### 8. Chuyển đổi đa ngôn ngữ

`sync0` hỗ trợ 75 ngôn ngữ và tự động chuyển đổi theo môi trường ngôn ngữ hệ thống theo mặc định; nó cũng có thể được chỉ định thủ công thông qua `SYNC0_LANG`:

```bash
SYNC0_LANG=en sync0 --help
SYNC0_LANG=ja sync0 --help
SYNC0_LANG=de sync0 --help
```