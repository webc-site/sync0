# sync0

`sync0` 是用於跨網路傳輸檔案、增量同步目錄以及多機器並發分發資料的命令列工具。

## 設計初衷

在跨機房、跨國或有網路延遲與丟包的環境中，`rsync` 和 `scp` 等工具基於單一連接順序確認，頻寬利用率受限；當多台機器同時從同一台伺服器拉取資料時，來源伺服器網路卡容易成為瓶頸。

`sync0` 的目標是解決兩個問題：

1. 讓兩台機器之間的檔案與目錄同步跑滿實體頻寬，並在檔案發生局部修改時只傳輸變化的部分。
2. 讓多台機器同步同一份目錄時可以互相傳資料塊，降低來源伺服器負載，縮短整體同步時間。

## 與 rsync 速度對比

測試環境：歐洲節點 $\leftrightarrow$ 北美節點，往返延遲 86 ms，測量包含目錄掃描、去重比對、網路傳輸與落盤校驗的完整耗時：

| 傳輸場景 | 數據規模 | `sync0` | `rsync` | 倍數 |
| :--- | :---: | :---: | :---: | :---: |
| 10,000 個小文件 | 77.6 MB | 0.90 s (688 Mbps) | 4.86 s (128 Mbps) | 5.4x |
| 代碼與歸檔包 (`.tar`) | 512 MB | 1.94 s (2111 Mbps) | 9.36 s (438 Mbps) | 4.8x |
| 影片檔 (`.mp4`) | 512 MB | 7.36 s (556 Mbps) | 12.11 s (338 Mbps) | 1.6x |
| 增量同步（修改 1% 內容） | 512 MB | 0.12 s | 1.85 s | 15.4x |

## 性能提升方式

- **UDP 並發傳輸**：透過 UDP 發送資料區塊，僅對遺失的分片進行重傳，避免單一丟包阻塞後續檔案傳輸。
- **內容分塊去重**：將檔案依內容切分為資料塊並計算校驗值。接收端已有的相同內容直接本地重複使用，不重複佔網。
- **按網路速度自動調整壓縮**：對文字、程式碼等可壓縮內容開啟壓縮；對影片、圖片、壓縮套件或在高速區域網路下自動跳過壓縮，減少 CPU 消耗。
- **多節點就近拉取**：多台機器拉取同一目錄時，自動優先從同城或延遲更低的已下載節點並發拉取資料塊。

## 安裝

### 方式一：透過 Cargo 安裝

```bash
cargo install sync0
```

### 方式二：下載預編譯二進位

前往 [GitHub Releases](https://github.com/webc-site/sync0/releases) 下載對應系統的 `.tar.xz` 壓縮包（支援 Linux x86_64/ARM64、macOS Apple Silicon/Intel、Windows x64/ARM64、Android ARM64/x86_64、FreeBSD、WindowsBSD），解壓縮後將 <bid="3 <bid="bid="bid="

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## 使用指南

### 1. 兩台機器直接傳文件或目錄

在接收端機器啟動監聽，指定接收保存路徑：

```bash
sync0 recv 0.0.0.0:9000 ./backup
```

在發送端機器發送單一檔案或整個目錄：

```bash
sync0 send ./project 192.168.1.10:9000
```

常用參數：

```bash
# 刪除接收端存在但發送端不存在的多餘檔案（鏡像同步）
sync0 send ./project 192.168.1.10:9000 --delete

# 限制發送速率（單位：位元組/秒，例如 50 MB/s）
sync0 send ./project 192.168.1.10:9000 --rate 50000000

# 靜默模式（不顯示進度條）
sync0 send ./project 192.168.1.10:9000 -q
```

### 2. 發布共享目錄與多節點拉取

在來源伺服器發布一個共享目錄（名稱為 `dataset`）：

```bash
sync0 share dataset ./data --region SHA
```

在其他機器上拉取該共享目錄（可同時填入多個節點位址，用逗號分隔以並發拉取）：

```bash
sync0 pull 10.0.0.1:9000,10.0.0.2:9000 dataset ./local_data
```

只拉取共享目錄中的某個子目錄：

```bash
sync0 pull 10.0.0.1:9000 dataset ./local_data --subpath 2026/logs
```

將本地修改推送回遠端共享目錄：

```bash
sync0 push 10.0.0.1:9000 dataset ./local_data
```

### 3. 訂閱目錄自動更新

訂閱源節點的共用目錄，當來源目錄有檔案新增或修改時會自動同步到本機，同時本機自動作為節點供其他機器拉取：

```bash
sync0 sub 10.0.0.1:9000 dataset ./local_mirror
```

### 4. 多機群組雙向同步

多台機器加入同一個群組名稱，任意節點目錄內的變更會自動同步給群組內其他節點：

```bash
sync0 group team-docs ./docs 10.0.0.1:9000,10.0.0.2:9000
```

### 5. 節點身分與個人名片

每個節點支援設定使用者名稱、所在城市、郵箱、網址以及一句個人簽名（上限 256 個字元），並透過本機私鑰簽署認證：

```bash
# 查看本機公鑰 ID（指定設定目錄以持久化保存金鑰）
sync0 id -c ~/.config/sync0

# 產生並展示帶有個性簽名的節點名片
sync0 profile -c ~/.config/sync0 \
  -u alice \
  --region SHA \
  --email alice@example.com \
  --url https://example.com \
  -b "Shanghai backup node"
```

在啟動 `share`、`pull`、`sub`、`group` 時同樣可以附帶這些個人資料參數：

```bash
sync0 share dataset ./data -c ~/.config/sync0 -u alice --region SHA -b "Shanghai backup node"
```

### 6. 多語言切換

`sync0` 支援 75 種語言，預設會根據作業系統的語言環境自動顯示對應語言的命令列幫助與提示；如果系統語言不在支援清單中，預設使用英文。

如需手動指定語言，可設定環境變數 `SYNC0_LANG`：

```bash
SYNC0_LANG=en sync0 --help
SYNC0_LANG=ja sync0 --help
SYNC0_LANG=de sync0 --help
```