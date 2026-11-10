# sync0

`sync0` 是一款用於替代 `rsync` 的去中心化 P2P 檔案分享、即時同步與多來源分發工具－跨洲廣域網路實測速度最高達 `rsync` 的 **6.95×**（單點幾何平均快 **2.01×**，P2P 雙源幾何平均達速

每台機器既是服務端也是客戶端：無需中心伺服器與公網位址，基於分散式網路公鑰位址自動打洞，支援兩機直傳、多分享群組權限託管、按需多選子目錄、毫秒即時增量同步與 P2P 多來源並發加速。

## 安裝

### 方式一：Cargo 安裝

```bash
cargo install sync0
```

### 方式二：下載預編譯二進位

前往 [GitHub Releases](https://github.com/webc-site/sync0/releases) 下載對應系統的 `.tar.xz` 壓縮套件（支援 Linux、macOS、Windows、Android、iOS、FreeBSD），解壓縮放入 `PATH`：

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## 快速上手

### 1. 兩機零配置直傳（`send` / `recv`）

無需設定分享群組，直接在兩台機器間高速傳輸檔案或目錄：

```bash
# 接收端監聽埠並寫入 ./backup
sync0 recv 0.0.0.0:9000 ./backup

# 發送端直傳（可選 --delete 鏡像清理多餘文件，--rate 50MB 限速）
sync0 send ./project 192.168.1.10:9000
```

### 2. 發佈分享與按需拉取 / 上傳（`share` / `pull` / `push`）

```bash
# 服務端：一行指令發布分享群組（啟動後輸出免 IP 的 iroh 節點位址）
sync0 share dataset:./data

# 用戶端：全量拉取 dataset 分享群組到本機 ./local_data
sync0 pull dataset:// ./local_data -p <節點位址>

# 客戶端：按需多選拉取（僅同步指定的單一檔案與子目錄）
sync0 pull dataset://docs/readme.md,assets/icons,src/core ./local_data -p <節點位址>

# 客戶端：上傳檔案或目錄到授權分享群組
sync0 push dataset://releases/v1 ./build_output -p <節點位址>
```

### 3. 即時訂閱做種與多機雙向同步（`sub` / `group`）

建立長連線後，遠端發生任何檔案**新增、修改、刪除、權限或修改時間變更**，本機目錄都會毫秒即時同步更新（新增/修改大檔案僅拉取差異區塊原位更新，刪除檔案自動清理本機檔案與空目錄，僅改權限或時間戳零資料區塊直達）：戳零資料區塊直達）：

- **`sync0 sub`（單向即時鏡像 + 自動做種）**：本地即時跟隨分享端增刪改，同時自動作為 P2P 種子來源為其他拉取節點提供並發加速。
- **`sync0 group`（多機對等雙向即時同步）**：群組內任一台機器發生檔案增刪改，其餘所有線上節點即時同步更新並互為種子。

```bash
# 訂閱整個分享群組：遠端增刪改本地即時同步，並自動製作
sync0 sub dataset:// ./local_mirror -p <節點位址>

# 按需多選訂閱：僅即時同步並做種指定的單一檔案與子目錄
sync0 sub dataset://models/llama.onnx,configs ./local_mirror -p <節點位址>

# 多機對等群組雙向即時同步（同樣支援全量或逗號多選路徑）
sync0 group team-docs:// ./docs -p <節點位址>
```

### 4. 團隊多分享群組與成員權限配置（`.nt` 設定）

默認配置目錄為 `~/.config/sync0`（可通過 `-c <目錄>` 切換），採用極簡 [NestedText](https://nestedtext.org)（`.nt`）格式：
- **支援限制公網讀取（僅授權使用者可讀）**：只要分享群組配置了 `users:` 成員清單（或命令列指定 `--allow-r` / `--allow-rw` / `--allow-w` / `--auth`），、**自動開啟私有公証（`auth: true`）舉**訂閱做種節點也會同步繼承白名單，僅允許授權使用者讀取；未配置 `users` 的分享群組預設為公開只讀（如需公開只讀且僅對指定成員開放上傳，可明確設定 `auth: false`）。
- **細粒度讀寫與子目錄投遞權限**：成員預設唯讀（`r` 無需明確編寫），支援 `r`（唯讀）、`w`（僅寫投遞箱，不可讀取）、`rw`（讀寫）及 `write_prefix`（限制允許上傳的子目錄前綴）。

**成員配置範例（`~/.config/sync0/users/`）：**

```yaml
# ~/.config/sync0/users/alice.nt（預設唯讀，限速 80MB）
id: 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c
rate: 80MB
```

```yaml
# ~/.config/sync0/users/bob.nt（讀寫權限，僅允許上傳到指定子目錄）
id: 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f
perm: rw
write_prefix:
  - uploads/bob
  - shared/common
```

```yaml
# ~/.config/sync0/users/charlie.nt（僅寫投遞權限，禁止讀取）
id: 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
perm: w
```

**分享群組配置範例（`~/.config/sync0/config.nt`）：**

```yaml
users_dir: ./users

groups:
  # 未配置 users：預設公網唯讀
  dataset:
    path: ./data
    rate: 50MB

  # 配置了 users：自動限制公網讀取（僅 alice、bob 可讀，charlie 僅可投遞上傳）
  team-vault:
    path: ./vault
    users:
      - alice
      - bob
      - charlie
```

啟動服務（加載配置文件或直接用命令行參數限制公網讀取並授權成員）：

```bash
# 載入 ~/.config/sync0/config.nt 啟動
sync0 share --region SHA

# 或純命令行發布私有分享組（指定 --allow-* 自動限制公網讀取，僅授權用戶可訪問）
sync0 share team-vault:./vault \
  --allow-r 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c \
  --allow-rw 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f:uploads/bob,shared/common \
  --allow-w 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
```

### 5. 節點地址、名片與多語言

```bash
# 查看本機免 IP 節點位址（首次執行自動產生金鑰）
sync0 id

# 產生 Ed25519 自簽名節點名片（暱稱、城市代碼、郵箱、網址、簡介）
sync0 profile -u alice --region SHA --email alice@example.com -b "Shanghai node"

# 內建 75 種語言，預設跟隨系統語言，可透過環境變數切換
SYNC0_LANG=en sync0 --help
```

## 性能與核心特性

### 跨洲廣域網路實測（對照 `rsync`）

實測環境：美國 `c1` → 歐洲 `n1`（RTT 65 ms），資料集為 Linux 核心 `linux-7.3.3`：

#### 1) 單點模式 vs `rsync`

| 數據集 | 大小 | `sync0` 單點<br>(Mbps) | `rsync`<br>(Mbps) | 加速比 (`sync0` / `rsync`) |
| :--- | ---: | ---: | ---: | ---: |
| `linux-7.3.3.tar.xz`（不可壓縮） | 155 MB | **121.07** | 107.89 | **1.12×** |
| `linux-7.3.3.tar`（可壓縮） | 1.6 GB | **442.07** | 506.62 | **0.87×** |
| `linux-7.3.3.tar`（局部改動增量） | 1.6 GB | **8,123.87** | 1,169.49 | **6.95×** |
| `linux-7.3.3/`（95,979 個小文件） | 1.5 GB | **565.49** | 235.27 | **2.40×** |
| **幾何平均** | — | **704.17** | 350.19 | **2.01×** |

#### 2) P2P 雙源模式 vs `rsync`

| 數據集 | 大小 | `sync0` P2P 雙源<br>(Mbps) | `rsync`<br>(Mbps) | 加速比 (`sync0` / `rsync`) |
| :--- | ---: | ---: | ---: | ---: |
| `linux-7.3.3.tar.xz`（不可壓縮） | 155 MB | **466.85** | 107.89 | **4.33×** |
| `linux-7.3.3.tar`（可壓縮） | 1.6 GB | **992.17** | 506.62 | **1.96×** |
| `linux-7.3.3.tar`（局部改動增量） | 1.6 GB | **5,666.84** | 1,169.49 | **4.85×** |
| `linux-7.3.3/`（95,979 個小文件） | 1.5 GB | **703.30** | 235.27 | **2.99×** |
| **幾何平均** | — | **1,165.63** | 350.19 | **3.33×** |

> 註：P2P 雙源模式下由歐洲節點 `s1` 協同作種，跨洲主幹 RTT 65 ms。

### 核心產品特性

- **公鑰尋址自動打洞**：無需中心伺服器與公網位址，基於分散式網路公鑰位址自動 NAT 打洞穿透。
- **毫秒級即時同步與元資料感知**：基於原生檔案事件監聽（`FSEvents` / `inotify` / `ReadDirectoryChangesW`），即時推播檔案增刪改、POSIX 權限（`mode`）與毫秒修改時間（`mtime`）；僅修改時間零時零點傳輸時零點修改時間（`mtime`）；僅修改時間零時零點傳輸權限。
- **BT 式邊下載邊上傳與地域親和調度**：下載節點無需等全量落盤，凡已通過 128-bit 雜湊校驗的分塊區間（`Op::Have`）即刻在記憶體/暫存池無鎖供其他節點並行拉取，配合動態 PEX 節點交換與慢節點折半搶占（Work-Stealing）最大化集群吞吐。 
- **防惡意節點竄改與集群投毒免疫**：權威源清單 `root_hash` 自洽鎖定 + 每塊落盤與對外做種前強制執行硬體加速 `gxhash128` 摘要校驗；發現竄改塊毫秒級熔斷拉黑惡意節點並自動從誠實源重拉恢復。
- **FastCDC 原位增量與小檔案聚合**：大檔案局部修改僅傳輸差異區塊並原位落盤；海量小檔案記憶體打包與前綴差分壓縮，消除逐檔案往返開銷。
- **斷電自癒與重啟續推**：採用類 Redis RDB + AOF 混合格式（`.s0_subs.bin`）持久化做種狀態與文件清單，日常改動微秒級追加、定期自動重寫整理，服務端重啟後自動向做種節點補推離線改動。

## 技術架構與文檔

- **雙平面解耦協定**：
  - **控制與即時變更面（`iroh` QUIC / TCP，首字節 `PROTO_VERSION` 版本握手）**：負責 Ed25519 名片驗簽、權限校驗、Swarm 種子發現，以及支持 `u64::MAX` 級海量變更流式分批與 `mzstd` 壓縮的`FsOp::Meta` 三原語）。
  - **資料分發面（原生 UDP + `compio` 非同步 I/O）**：20B 緊湊包頭、18B TACK 週期回饋、BBRv3/Copa 混合擁塞控制、RACK-TLP 丟包恢復、首塊香農端自適應 Zstd 壓縮旁路，以及 `gxhash128` 分塊到端校驗。
- **深入了解**：
  - [效能實測報告與復現腳本](https://github.com/webc-site/sync0/blob/main/doc/zh/benchmark.md)
  - [系統架構設計](https://github.com/webc-site/sync0/blob/main/doc/zh/design_v4.md)
  - [二進制線協定與持久化規範](https://github.com/webc-site/sync0/blob/main/doc/zh/proto.md)
  - [身分認證、權限控制與完整性校驗](https://github.com/webc-site/sync0/blob/main/doc/zh/crypto.md)