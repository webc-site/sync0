# sync0

`sync0` is a peer-to-peer (P2P) file sharing and multi-machine synchronization tool that competes with Resilio Sync.

Each machine is both a server and a client: any node can publish multiple file sharing groups. After other nodes join, they can automatically obtain available seed nodes from the peer server and pull them concurrently from multiple sources. After synchronization is completed, seeds will be automatically created for more nodes to accelerate downloads.

## Core Highlights

- **Everyone is both a server and a client**: Comparable to Resilio Sync, no central server dependency. Any node can create multiple file sharing groups, and can also join other people's sharing groups; nodes automatically exchange and obtain available seed nodes to achieve concurrent pulling and real-time synchronization of multiple machines.
- **IP-free node address**: By default, the node key and `base64url` encoded `iroh` address are automatically generated in `~/.config/sync0` without needing to remember or expose the public IP.
- **Multi-select synchronization on demand**: Supports synchronization of the entire sharing group, and also supports synchronization on demand by selecting multiple files or subdirectories at the same time.
- **Group member hierarchical permissions and upload directory control**: Supports read-only (`ro`), read-write upload (`rw`), and management (`admin`) permissions, and can individually specify the path prefix that each member is allowed to upload (by default, uploading to the root directory of the share is allowed).
- **Full-speed and incremental synchronization across networks**: Concurrent transmission of large files, aggregation of massive small files in memory, and only synchronized data blocks when files are partially modified.

## Speed ​​comparison with rsync

### Transoceanic network single source actual measurement (China → United States, delay 260 ms)

| Transmission scenario | Data size | Model | `sync0` | `rsync -az` | Increase multiplier |
| :--- | :---: | :---: | :---: | :---: | :---: |
| 500 Small Files | 1.91 MB | Full synchronization | 18.84 Mbps | 5.32 Mbps | 3.5x |
| 500 Small Files | 1.91 MB | Incremental synchronization | 27.15 Mbps | 3.83 Mbps | 7.1x |
| Code and archive packages (`.tar`) | 10.0 MB | Full synchronization | 139.70 Mbps | 24.97 Mbps | 5.6x |
| Code and archive packages (`.tar`) | 10.0 MB | Incremental synchronization | 83.38 Mbps | 26.02 Mbps | 3.2x |
| Video file (`.mp4`) | 15.0 MB | Full synchronization | 54.06 Mbps | 27.04 Mbps | 2.0x |
| Video file (`.mp4`) | 15.0 MB | Incremental synchronization | 216.83 Mbps | 21.51 Mbps | 10.1x |

### Transoceanic network P2P multi-node pull test (China → United States dual nodes, delay 260 ms)

Measured comparison of multi-source seed node striped concurrent pull:

| Transmission scenario | Data size | Single node seeding | Two-node P2P concurrent seeding | Increase multiplier |
| :--- | :---: | :---: | :---: | :---: |
| 500 Small Files | 1.97 MB | 5.28 Mbps | 7.87 Mbps | 1.5x |
| Code and archive packages (`.tar`) | 30.0 MB | 56.35 Mbps | 83.17 Mbps | 1.5x |
| Video file (`.mp4`) | 30.0 MB | 27.00 Mbps | 39.46 Mbps | 1.5x |

### Actual measurement of transcontinental high-speed dedicated line (Europe → North America, delay 86 ms)

| Transmission scenario | Data size | `sync0` | `rsync` | Increase multiplier |
| :--- | :---: | :---: | :---: | :---: |
| 10,000 small files | 77.6 MB | 688 Mbps | 128 Mbps | 5.4x |
| Code and archive packages (`.tar`) | 512 MB | 2111 Mbps | 438 Mbps | 4.8x |
| Video file (`.mp4`) | 512 MB | 556 Mbps | 338 Mbps | 1.6x |
| Incremental synchronization (modify 1% content) | 512 MB | 34133 Mbps | 2214 Mbps | 15.4x |

## Install

### Method 1: Install via Cargo

```bash
cargo install sync0
```

### Method 2: Download precompiled binaries

Go to [GitHub Releases](https://github.com/webc-site/sync0/releases) to download the `.tar.xz` compressed package for the corresponding system (supports Linux, macOS, Windows, Android, FreeBSD, NetBSD), unzip and put it into the `PATH` directory:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Get started quickly

`sync0` uses `~/.config/sync0` as the configuration directory by default (if you need to switch to other directories, you can add `-c <directory>`).

### 1. View the local node address and setting business card

```bash
# View the local node address (the key is automatically generated in ~/.config/sync0 for the first time)
sync0 id

# Generate a personal node business card (supports user name, city, email, website and signature within 256 words)
sync0 profile -u alice --region SHA --email alice@example.com --url https://example.com -b "Shanghai node"
```

### 2. Publish multiple file sharing groups and set group member permissions (`.nt` Configuration)

`sync0` is configured in the [NestedText](https://nestedtext.org) (`.nt`) format. Each user has a separate `.nt` configuration file. You can specify the user profile directory through `users_dir` in `config.nt` (which automatically includes all `.nt` files in this directory). Both the user and the path prefix allowed to be uploaded use an unordered list (`-`). **When the upload path prefix is ​​not filled in, uploading to the root directory of the share is allowed by default**.

1. Create separate `.nt` configuration files for each member in the `~/.config/sync0/users/` directory:

`~/.config/sync0/users/alice.nt` (read-only member, rate limited `80MB`, `rate` supports `GB`, `MB`, `KB` units):

```yaml
id: 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c
perm: ro
rate: 80MB
```

`~/.config/sync0/users/bob.nt` (restricted upload members: only allowed to upload to `uploads/bob` and `shared/common` directories):

```yaml
id: 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f
perm: rw
write_prefix:
  - uploads/bob
  - shared/common
```

`~/.config/sync0/users/charlie.nt` (Default member upload: When `write_prefix` is omitted, uploading to the root directory of the share is allowed by default):

```yaml
id: 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
perm: rw
```

2. Configure the user directory and multiple file sharing groups in `~/.config/sync0/config.nt`:

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

Start sharing service:

```bash
# Direct startup (automatically loads all .nt user configurations in ~/.config/sync0/config.nt and users_dir directories)
sync0 share --region SHA

# Or quickly publish multiple sharing groups and authorize group members directly through the command line
sync0 share dataset:./data team-vault:./vault \
  --auth \
  --allow-ro 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c \
  --allow-rw 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f:uploads/bob,shared/common \
  --allow-rw 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
```

### 3. Pull all content or select multiple files and directories (`sync0 pull`)

Connect to any peer node to automatically obtain all available seed nodes in the sharing group, and pull from multiple seed nodes concurrently:

```bash
# Pull the entire dataset sharing group to the local ./local_data
sync0 pull dataset:// ./local_data -p <node address>

# Multi-select synchronization: simultaneously pull the single file docs/readme.md and subdirectories assets/icons, src/core
sync0 pull dataset://docs/readme.md,assets/icons,src/core ./local_data -p <node address>
```

### 4. Upload a file or directory to a sharing group (`sync0 push`)

Members with upload permissions (`rw` or `admin`) can upload local files or directories to remote sharing groups:

```bash
# Members with root directory upload permission: Upload directly to the shared root directory
sync0 push team-vault:// ./new_release -p <node address>

# Only members authorized to a specific prefix (such as uploads/bob): upload to the allowed path prefix
sync0 push team-vault://uploads/bob/report.pdf ./report.pdf -p <node address>
```

### 5. Subscribe to automatic sync and seeding (`sync0 sub`)

After subscribing to the sharing group of a certain node, the local server is both a receiving client and a new seeding server; when the source node file is updated, it is automatically synchronized and provides chunked downloads for other nodes:

```bash
# Subscribe to the entire sharing group and automatically seed
sync0 sub dataset:// ./local_mirror -p <node address>

# Only multi-select to subscribe to specified files and subdirectories and seed them
sync0 sub dataset://models/llama.onnx,configs ./local_mirror -p <node address>
```

### 6. Multi-machine group peer synchronization (`sync0 group`)

Multiple machines join the same sharing group, each other automatically discovers available seed nodes and maintains two-way synchronization (supports full synchronization or multiple selection of files and directories):

```bash
# Fully join the group and two-way synchronization
sync0 group team-docs:// ./docs -p <node address>

# Only the wal directory and meta.json file in the multi-select synchronization group
sync0 group team-docs://wal,meta.json ./docs -p <node address>
```

### 7. Point-to-point direct transmission between two machines (`send` / `recv`)

There is no need to pre-configure a group, and you can temporarily transfer files or directories directly between two machines:

```bash
# Receiver monitoring
sync0 recv 0.0.0.0:9000 ./backup

# The sending end directly transfers files or directories (optional --delete mirror to clean up excess files, --rate speed limit)
sync0 send ./project 192.168.1.10:9000
```

### 8. Multi-language switching

`sync0` supports 75 languages, and automatically switches according to the system language environment by default; it can also be specified manually through `SYNC0_LANG`:

```bash
SYNC0_LANG=en sync0 --help
SYNC0_LANG=ja sync0 --help
SYNC0_LANG=de sync0 --help
```