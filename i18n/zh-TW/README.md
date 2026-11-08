# sync0

`sync0` 是一款對標 Resilio Sync 的點對點（P2P）檔案分享與多機同步工具。

每台機器既是伺服器也是客戶端：任何節點都可以發布多個檔案分享群組，其他節點加入後可從對等伺服器自動取得可用的種子節點並多源並發拉取，同步完成後自動做種供更多節點加速下載。

## 核心亮點

- **每個人既是伺服器也是客戶端**：對標 Resilio Sync，無中心伺服器依賴。任意節點均可建立多個文件分享群組，也可加入他人的分享群組；節點之間自動交換並取得可用的種子節點，實現多機並發拉取與即時同步。
- **免 IP 節點位址**：預設在 `~/.config/sync0` 自動產生節點金鑰與 `base64url` 編碼的 `iroh` 位址，無需記憶或暴露公網 IP。
- **按需多選同步**：支援同步整個分享群組，也支援同時勾選多個檔案或子目錄按需同步。
- **組員分級權限與上傳目錄控制**：支援唯讀（`ro`）、讀寫上傳（`rw`）、管理（`admin`）權限，並可單獨指定每個成員允許上傳的路徑前綴（預設允許上傳到該分享的根目錄）。
- **跨網滿速與增量同步**：大檔案並發傳輸、海量小檔案記憶體聚合落盤，檔案局部修改時僅同步變化的資料區塊。

## 與 rsync 速度對比

### 跨洋網路單源實測（中國 → 美國，延遲 260 ms）

| 傳輸場景 | 數據規模 | 模式 | `sync0` | `rsync -az` | 提升倍數 |
| :--- | :---: | :---: | :---: | :---: | :---: |
| 500 個小文件 | 1.91 MB | 全量同步 | 18.84 Mbps | 5.32 Mbps | 3.5x |
| 500 個小文件 | 1.91 MB | 增量同步 | 27.15 Mbps | 3.83 Mbps | 7.1x |
| 代碼與歸檔包 (`.tar`) | 10.0 MB | 全量同步 | 139.70 Mbps | 24.97 Mbps | 5.6x |
| 代碼與歸檔包 (`.tar`) | 10.0 MB | 增量同步 | 83.38 Mbps | 26.02 Mbps | 3.2x |
| 影片檔 (`.mp4`) | 15.0 MB | 全量同步 | 54.06 Mbps | 27.04 Mbps | 2.0x |
| 影片檔 (`.mp4`) | 15.0 MB | 增量同步 | 216.83 Mbps | 21.51 Mbps | 10.1x |

### 跨洋網路 P2P 多節點拉取實測（中國 → 美國雙節點，延遲 260 ms）

多源種子節點條帶化並發拉取實測比較：

| 傳輸場景 | 數據規模 | 單節點做種 | 雙節點 P2P 並發做種 | 提升倍數 |
| :--- | :---: | :---: | :---: | :---: |
| 500 個小文件 | 1.97 MB | 5.28 Mbps | 7.87 Mbps | 1.5x |
| 代碼與歸檔包 (`.tar`) | 30.0 MB | 56.35 Mbps | 83.17 Mbps | 1.5x |
| 影片檔 (`.mp4`) | 30.0 MB | 27.00 Mbps | 39.46 Mbps | 1.5x |

### 跨洲高速專線實測（歐洲 → 北美，延時 86 ms）

| 傳輸場景 | 數據規模 | `sync0` | `rsync` | 提升倍數 |
| :--- | :---: | :---: | :---: | :---: |
| 10,000 個小文件 | 77.6 MB | 688 Mbps | 128 Mbps | 5.4x |
| 代碼與歸檔包 (`.tar`) | 512 MB | 2111 Mbps | 438 Mbps | 4.8x |
| 影片檔 (`.mp4`) | 512 MB | 556 Mbps | 338 Mbps | 1.6x |
| 增量同步（修改 1% 內容） | 512 MB | 34133 Mbps | 2214 Mbps | 15.4x |

## 安裝

### 方式一：透過 Cargo 安裝

```bash
cargo install sync0
```

### 方式二：下載預編譯二進位

前往 [GitHub Releases](https://github.com/webc-site/sync0/releases) 下載對應系統的 `.tar.xz` 壓縮套件（支援 Linux、macOS、Windows、Android、FreeBSD、NetBSD），解壓縮後放入 `PATH` 目錄：

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## 快速上手

`sync0` 預設使用 `~/.config/sync0` 作為配置目錄（如需切換其他目錄，可新增 `-c <目錄>`）。

### 1. 查看本機節點位址與設定名片

```bash
# 查看本機節點位址（首次執行自動在 ~/.config/sync0 產生金鑰）
sync0 id

# 產生個人節點名片（支援使用者名稱、所在城市、信箱、網址與 256 字以內簽名）
sync0 profile -u alice --region SHA --email alice@example.com --url https://example.com -b "Shanghai node"
```

### 2. 發布多個檔案分享群組與設定群組員權限（`.nt` 設定）

`sync0` 採用 [NestedText](https://nestedtext.org)（`.nt`）格式配置，每個使用者單獨一個 `.nt` 配置文件，可在 `config.nt` 中透過 `users_dir` 指定使用者設定檔目錄（自動包含該目錄下的全部 `.nt`b="87> 使用者使用字彙的序列（87<b="87<b="8)。 **未填入上傳路徑前綴時，預設允許上傳到該分享的根目錄**。

1. 在 `~/.config/sync0/users/` 目錄下為每個成員建立獨立的 `.nt` 設定檔：

`~/.config/sync0/users/alice.nt`（唯讀成員，限速 `80MB`，`rate` 支援 `GB`、`MB`、`KB` 單位）：

```yaml
id: 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c
perm: ro
rate: 80MB
```

`~/.config/sync0/users/bob.nt`（受限上傳成員：僅允許上傳到 `uploads/bob` 和 `shared/common` 目錄下）：

```yaml
id: 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f
perm: rw
write_prefix:
  - uploads/bob
  - shared/common
```

`~/.config/sync0/users/charlie.nt`（預設上傳成員：省略 `write_prefix` 時，預設允許上傳到該分享的根目錄）：

```yaml
id: 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
perm: rw
```

2. 在 `~/.config/sync0/config.nt` 中設定使用者目錄與多個檔案分享群組：

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

啟動分享服務：

```bash
# 直接啟動（自動載入 ~/.config/sync0/config.nt 及 users_dir 目錄下的全部 .nt 使用者設定）
sync0 share --region SHA

# 或直接透過命令列快速發布多個分享群組並授權組員
sync0 share dataset:./data team-vault:./vault \
  --auth \
  --allow-ro 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c \
  --allow-rw 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f:uploads/bob,shared/common \
  --allow-rw 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
```

### 3. 拉取全部內容或多選檔案與目錄 (`sync0 pull`)

連接任一對等節點即可自動取得該分享群組內所有可用的種子節點，並從多個種子節點並發拉取：

```bash
# 拉取整個 dataset 分享群組到本地 ./local_data
sync0 pull dataset:// ./local_data -p <節點位址>

# 多重選擇同步：同時拉取單一檔案 docs/readme.md 與子目錄 assets/icons、src/core
sync0 pull dataset://docs/readme.md,assets/icons,src/core ./local_data -p <節點位址>
```

### 4. 上傳檔案或目錄 (`sync0 push`)

擁有上傳權限（`rw` 或 `admin`）的成員可將本機檔案或目錄上傳至遠端分享群組：

```bash
# 擁有根目錄上傳權限的成員：直接上傳到分享根目錄
sync0 push team-vault:// ./new_release -p <節點位址>

# 僅被授權特定前綴（如 uploads/bob）的成員：上傳到允許的路徑前綴下
sync0 push team-vault://uploads/bob/report.pdf ./report.pdf -p <節點位址>
```

### 5. 訂閱自動同步與做種 (`sync0 sub`)

訂閱某台節點的分享群組後，本地既是接收客戶端，也是新的做種伺服器；當來源節點檔案更新時自動同步，同時為其他節點提供分塊下載：

```bash
# 訂閱整個分享群組並自動做種
sync0 sub dataset:// ./local_mirror -p <節點位址>

# 僅多選訂閱指定檔案與子目錄並做種
sync0 sub dataset://models/llama.onnx,configs ./local_mirror -p <節點位址>
```

### 6. 多機群組對等同步 (`sync0 group`)

多台機器加入同一分享群組，彼此自動發現可用種子節點並保持雙向同步（支援全量同步或多選檔案與目錄）：

```bash
# 全量加入群組雙向同步
sync0 group team-docs:// ./docs -p <節點位址>

# 僅多選同步群組中的 wal 目錄與 meta.json 文件
sync0 group team-docs://wal,meta.json ./docs -p <節點位址>
```

### 7. 兩台機器點對點直傳 (`send` / `recv`)

無需預先配置群組，暫時在兩台機器間直傳檔案或目錄：

```bash
# 接收端監聽
sync0 recv 0.0.0.0:9000 ./backup

# 傳送端直傳文件或目錄（可選 --delete 鏡像清理多餘文件，--rate 限速）
sync0 send ./project 192.168.1.10:9000
```

### 8. 多語言切換

`sync0` 支援 75 種語言，預設會根據系統語言環境自動切換；也可透過 `SYNC0_LANG` 手動指定：

```bash
SYNC0_LANG=en sync0 --help
SYNC0_LANG=ja sync0 --help
SYNC0_LANG=de sync0 --help
```