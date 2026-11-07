# sync0

`sync0` là công cụ dòng lệnh để truyền tệp qua mạng, đồng bộ hóa dần dần các thư mục và phân phối dữ liệu đồng thời tới nhiều máy.

## Ý định thiết kế ban đầu

Trong các phòng máy, quốc gia hoặc môi trường có độ trễ mạng và mất gói, các công cụ như `rsync` và `scp` dựa trên xác nhận trình tự kết nối duy nhất, điều này hạn chế việc sử dụng băng thông; Khi nhiều máy kéo dữ liệu từ cùng một máy chủ cùng lúc, card mạng của máy chủ nguồn rất dễ bị tắc nghẽn.

Mục tiêu của `sync0` là giải quyết hai vấn đề:

1. Để quá trình đồng bộ hóa tệp và thư mục giữa hai máy chạy hết băng thông vật lý và chỉ truyền phần đã thay đổi khi tệp được sửa đổi một phần.
2. Nhiều máy có thể truyền các khối dữ liệu cho nhau khi đồng bộ cùng một thư mục, giảm tải cho máy chủ nguồn và rút ngắn thời gian đồng bộ tổng thể.

## So sánh tốc độ với rsync

Môi trường thử nghiệm: Nút Châu Âu $\leftrightarrow$ Nút Bắc Mỹ, độ trễ khứ hồi 86 ms, đo toàn bộ thời gian bao gồm quét thư mục, so sánh chống trùng lặp, truyền mạng và xác minh vị trí ổ đĩa:

| Kịch bản truyền tải | Kích thước dữ liệu | `sync0` | `rsync` | nhiều |
| :--- | :---: | :---: | :---: | :---: |
| 10.000 tập tin nhỏ | 77.6 MB | 0.90 s (688 Mbps) | 4.86 s (128 Mbps) | 5.4x |
| Gói mã và lưu trữ (`.tar`) | 512 MB | 1.94 s (2111 Mbps) | 9.36 s (438 Mbps) | 4.8x |
| Tệp video (`.mp4`) | 512 MB | 7.36 s (556 Mbps) | 12.11 s (338 Mbps) | 1.6x |
| Đồng bộ hóa gia tăng (sửa đổi nội dung 1%) | 512 MB | 0.12 s | 1.85 s | 15.4x |

## Phương pháp cải tiến hiệu suất

- **Truyền đồng thời UDP**: Gửi khối dữ liệu qua UDP và chỉ truyền lại các đoạn bị mất để tránh mất gói duy nhất, chặn các lần truyền tệp tiếp theo.
- **Phân đoạn nội dung và chống trùng lặp**: Chia file thành các khối dữ liệu theo nội dung và tính giá trị kiểm tra. Nội dung tương tự đã có ở đầu nhận sẽ được sử dụng lại trực tiếp cục bộ mà không cần sao chép mạng.
- **Tự động điều chỉnh nén theo tốc độ mạng**: Bật tính năng nén đối với nội dung có thể nén như văn bản và mã; tự động bỏ qua việc nén video, hình ảnh, gói nén hoặc trong mạng LAN tốc độ cao để giảm mức tiêu thụ CPU.
- **Kéo nhiều nút lân cận**: Khi nhiều máy kéo cùng một thư mục, các khối dữ liệu sẽ được kéo đồng thời từ các nút đã tải xuống trong cùng một thành phố hoặc có độ trễ thấp hơn.

## Cài đặt

### Cách 1: Cài đặt qua Cargo

```bash
cargo install sync0
```

### Phương pháp 2: Tải xuống các tệp nhị phân được biên dịch sẵn

Đi tới [Bản phát hành GitHub](https://github.com/webc-site/sync0/releases) để tải xuống gói nén `.tar.xz` cho hệ thống tương ứng (hỗ trợ Linux x86_64/ARM64, macOS Apple Silicon/Intel, Windows x64/ARM64, Android ARM64/x86_64, FreeBSD, NetBSD). Sau khi giải nén, đặt `sync0` vào thư mục `PATH`:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Hướng dẫn sử dụng

### 1. Truyền trực tiếp tập tin hoặc thư mục giữa hai máy

Bắt đầu giám sát trên máy nhận và chỉ định đường dẫn nhận và lưu:

```bash
sync0 recv 0.0.0.0:9000 ./backup
```

Gửi một tập tin hoặc toàn bộ thư mục từ máy gửi:

```bash
sync0 send ./project 192.168.1.10:9000
```

Các thông số thường dùng:

```bash
# Xóa các tập tin dư thừa tồn tại ở đầu nhận nhưng không tồn tại ở đầu gửi (đồng bộ hóa gương)
sync0 send ./project 192.168.1.10:9000 --delete

# Giới hạn tốc độ gửi (đơn vị: byte/giây, ví dụ 50 MB/s)
sync0 send ./project 192.168.1.10:9000 --rate 50000000

# Chế độ im lặng (không hiển thị thanh tiến trình)
sync0 send ./project 192.168.1.10:9000 -q
```

### 2. Xuất bản các thư mục được chia sẻ và kéo từ nhiều nút

Xuất bản thư mục dùng chung (có tên `dataset`) trên máy chủ gốc:

```bash
sync0 share dataset ./data --region SHA
```

Kéo thư mục dùng chung trên máy khác (có thể điền nhiều địa chỉ node cùng lúc, cách nhau bằng dấu phẩy để kéo đồng thời):

```bash
sync0 pull 10.0.0.1:9000,10.0.0.2:9000 dataset ./local_data
```

Chỉ kéo một thư mục con nhất định trong thư mục dùng chung:

```bash
sync0 pull 10.0.0.1:9000 dataset ./local_data --subpath 2026/logs
```

Đẩy các thay đổi cục bộ trở lại thư mục chia sẻ từ xa:

```bash
sync0 push 10.0.0.1:9000 dataset ./local_data
```

### 3. Đăng ký cập nhật danh mục tự động

Đăng ký thư mục chia sẻ của nút nguồn. Khi một tập tin được thêm vào hoặc sửa đổi trong thư mục nguồn, nó sẽ tự động được đồng bộ hóa với máy tính cục bộ. Đồng thời, máy cục bộ sẽ tự động đóng vai trò là nút để các máy khác kéo:

```bash
sync0 sub 10.0.0.1:9000 dataset ./local_mirror
```

### 4. Đồng bộ hai chiều của nhiều nhóm máy

Nhiều máy tham gia cùng một tên nhóm và các thay đổi trong bất kỳ thư mục nút nào sẽ được tự động đồng bộ hóa với các nút khác trong nhóm:

```bash
sync0 group team-docs ./docs 10.0.0.1:9000,10.0.0.2:9000
```

### 5. Danh tính nút và danh thiếp cá nhân

Mỗi nút hỗ trợ đặt tên người dùng, thành phố, email, trang web và chữ ký được cá nhân hóa (tối đa 256 ký tự) và vượt qua xác thực chữ ký khóa riêng cục bộ:

```bash
# Xem ID khóa công khai cục bộ (chỉ định thư mục cấu hình để duy trì khóa)
sync0 id -c ~/.config/sync0

# Tạo và hiển thị danh thiếp nút có chữ ký được cá nhân hóa
sync0 profile -c ~/.config/sync0 \
  -u alice \
  --region SHA \
  --email alice@example.com \
  --url https://example.com \
  -b "Shanghai backup node"
```

`share`, `pull`, `sub`, `group` cũng có thể được khởi chạy với các tham số cấu hình sau:

```bash
sync0 share dataset ./data -c ~/.config/sync0 -u alice --region SHA -b "Shanghai backup node"
```

### 6. Chuyển đổi đa ngôn ngữ

`sync0` Hỗ trợ 75 ngôn ngữ. Theo mặc định, trợ giúp dòng lệnh và lời nhắc bằng ngôn ngữ tương ứng sẽ tự động được hiển thị theo môi trường ngôn ngữ của hệ điều hành; nếu ngôn ngữ hệ thống không có trong danh sách được hỗ trợ, tiếng Anh sẽ được sử dụng theo mặc định.

Để chỉ định ngôn ngữ theo cách thủ công, hãy đặt biến môi trường `SYNC0_LANG`:

```bash
SYNC0_LANG=en sync0 --help
SYNC0_LANG=ja sync0 --help
SYNC0_LANG=de sync0 --help
```