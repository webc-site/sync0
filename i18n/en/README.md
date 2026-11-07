# sync0

`sync0` is a command line tool for transferring files across networks, incrementally synchronizing directories, and concurrently distributing data to multiple machines.

## Original design intention

In cross-machine rooms, countries, or environments with network delays and packet loss, tools such as `rsync` and `scp` are based on single connection sequence confirmation, which limits bandwidth utilization; when multiple machines pull data from the same server at the same time, the source server network card can easily become a bottleneck.

The goal of `sync0` is to solve two problems:

1. Let the file and directory synchronization between the two machines run full physical bandwidth, and only transfer the changed part when the file is partially modified.
2. Multiple machines can transfer data blocks to each other when synchronizing the same directory, reducing the load on the source server and shortening the overall synchronization time.

## Speed ​​comparison with rsync

Test environment: European node $\leftrightarrow$ North American node, round-trip delay 86 ms, measuring the complete time-consuming including directory scanning, deduplication comparison, network transmission and disk placement verification:

| Transmission scenario | Data size | `sync0` | `rsync` | Multiple |
| :--- | :---: | :---: | :---: | :---: |
| 10,000 small files | 77.6 MB | 0.90 s (688 Mbps) | 4.86 s (128 Mbps) | 5.4x |
| Code and archive packages (`.tar`) | 512 MB | 1.94 s (2111 Mbps) | 9.36 s (438 Mbps) | 4.8x |
| Video file (`.mp4`) | 512 MB | 7.36 s (556 Mbps) | 12.11 s (338 Mbps) | 1.6x |
| Incremental synchronization (modify 1% content) | 512 MB | 0.12 s | 1.85 s | 15.4x |

## Performance improvement methods

- **UDP concurrent transmission**: Send data blocks through UDP, and only retransmit lost fragments to avoid a single packet loss blocking subsequent file transfers.
- **Content segmentation and deduplication**: Split the file into data blocks according to content and calculate the check value. The same content already on the receiving end is reused directly locally without duplicating the network.
- **Automatically adjust compression according to network speed**: Turn on compression for compressible content such as text and code; automatically skip compression for videos, pictures, compressed packages or under high-speed LAN to reduce CPU consumption.
- **Multi-node nearby pulling**: When multiple machines pull the same directory, data blocks will be pulled concurrently from downloaded nodes in the same city or with lower latency.

## Install

### Method 1: Install via Cargo

```bash
cargo install sync0
```

### Method 2: Download precompiled binaries

Go to [GitHub Releases](https://github.com/webc-site/sync0/releases) to download the `.tar.xz` compressed package for the corresponding system (supports Linux x86_64/ARM64, macOS Apple Silicon/Intel, Windows x64/ARM64, Android ARM64/x86_64, FreeBSD, NetBSD). After unzipping, place `sync0` into the `PATH` directory:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## User Guide

### 1. Directly transfer files or directories between two machines

Start monitoring on the receiving machine and specify the receiving and saving path:

```bash
sync0 recv 0.0.0.0:9000 ./backup
```

Send a single file or an entire directory from the sending machine:

```bash
sync0 send ./project 192.168.1.10:9000
```

Commonly used parameters:

```bash
# Delete redundant files that exist on the receiving end but not on the sending end (mirror synchronization)
sync0 send ./project 192.168.1.10:9000 --delete

# Limit sending rate (unit: bytes/second, for example 50 MB/s)
sync0 send ./project 192.168.1.10:9000 --rate 50000000

# Silent mode (no progress bar shown)
sync0 send ./project 192.168.1.10:9000 -q
```

### 2. Publishing shared directories and pulling from multiple nodes

Publish a shared directory (named `dataset`) on the origin server:

```bash
sync0 share dataset ./data --region SHA
```

Pull the shared directory on other machines (you can fill in multiple node addresses at the same time, separated by commas to pull concurrently):

```bash
sync0 pull 10.0.0.1:9000,10.0.0.2:9000 dataset ./local_data
```

Pull only a certain subdirectory in the shared directory:

```bash
sync0 pull 10.0.0.1:9000 dataset ./local_data --subpath 2026/logs
```

Push local changes back to the remote shared directory:

```bash
sync0 push 10.0.0.1:9000 dataset ./local_data
```

### 3. Subscribe to catalog automatic updates

Subscribe to the shared directory of the source node. When a file is added or modified in the source directory, it will automatically be synchronized to the local computer. At the same time, the local machine will automatically serve as a node for other machines to pull:

```bash
sync0 sub 10.0.0.1:9000 dataset ./local_mirror
```

### 4. Two-way synchronization of multi-machine groups

Multiple machines join the same group name, and changes in any node directory will be automatically synchronized to other nodes in the group:

```bash
sync0 group team-docs ./docs 10.0.0.1:9000,10.0.0.2:9000
```

### 5. Node identity and personal business card

Each node supports setting the user name, city, email, website and a personalized signature (maximum 256 characters), and passes the local private key signature authentication:

```bash
# View the local public key ID (specify the configuration directory to persist the key)
sync0 id -c ~/.config/sync0

# Generate and display node business cards with personalized signatures
sync0 profile -c ~/.config/sync0 \
  -u alice \
  --region SHA \
  --email alice@example.com \
  --url https://example.com \
  -b "Shanghai backup node"
```

`share`, `pull`, `sub`, `group` can also be launched with these profile parameters:

```bash
sync0 share dataset ./data -c ~/.config/sync0 -u alice --region SHA -b "Shanghai backup node"
```

### 6. Multi-language switching

`sync0` Supports 75 languages. By default, command line help and prompts in the corresponding language are automatically displayed according to the language environment of the operating system; if the system language is not in the supported list, English is used by default.

To specify the language manually, set the environment variable `SYNC0_LANG`:

```bash
SYNC0_LANG=en sync0 --help
SYNC0_LANG=ja sync0 --help
SYNC0_LANG=de sync0 --help
```