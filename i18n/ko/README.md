# sync0

`sync0`은 Resilio Sync와 경쟁하는 P2P(Peer-to-Peer) 파일 공유 및 다중 시스템 동기화 도구입니다.

각 시스템은 서버이자 클라이언트입니다. 모든 노드는 여러 파일 공유 그룹을 게시할 수 있습니다. 다른 노드가 합류한 후에는 피어 서버에서 사용 가능한 시드 노드를 자동으로 얻고 여러 소스에서 동시에 가져올 수 있습니다. 동기화가 완료된 후 다운로드 속도를 높이기 위해 더 많은 노드에 대한 시드가 자동으로 생성됩니다.

## 핵심 하이라이트

- **모든 사람은 서버이자 클라이언트입니다**: Resilio Sync와 유사하며 중앙 서버 종속성이 없습니다. 모든 노드는 여러 파일 공유 그룹을 만들 수 있으며 다른 사람의 공유 그룹에 가입할 수도 있습니다. 노드는 사용 가능한 시드 노드를 자동으로 교환하고 획득하여 여러 시스템의 동시 풀링 및 실시간 동기화를 달성합니다.
- **IP 프리 노드 주소**: 기본적으로 노드 키와 `base64url` 인코딩된 `iroh` 주소는 공용 IP를 기억하거나 노출할 필요 없이 `~/.config/sync0`에서 자동으로 생성됩니다.
- **요청 시 다중 선택 동기화**: 전체 공유 그룹의 동기화를 지원하고 동시에 여러 파일 또는 하위 디렉터리를 선택하여 요청 시 동기화도 지원합니다.
- **그룹 구성원 계층적 권한 및 업로드 디렉터리 제어**: 읽기 전용(`ro`), 읽기-쓰기 업로드(`rw`) 및 관리(`admin`) 권한을 지원하고 각 구성원이 업로드할 수 있는 경로 접두사를 개별적으로 지정할 수 있습니다(기본적으로 공유의 루트 디렉터리에 업로드가 허용됨).
- **네트워크 전반에 걸쳐 최고 속도 및 증분 동기화**: 대용량 파일의 동시 전송, 대용량 소형 파일의 메모리 내 집계, 파일이 부분적으로 수정된 경우에만 데이터 블록 동기화.

## rsync와의 속도 비교

### 대양횡단망 단일 소스 실측(중국 → 미국, 지연 260ms)

| 전송 시나리오 | 데이터 크기 | 모델 | `sync0` | `rsync -az` | 승수 증가 |
| :--- | :---: | :---: | :---: | :---: | :---: |
| 작은 파일 500개 | 1.91 MB | 전체 동기화 | 18.84 Mbps | 5.32 Mbps | 3.5x |
| 작은 파일 500개 | 1.91 MB | 증분 동기화 | 27.15 Mbps | 3.83 Mbps | 7.1x |
| 코드 및 아카이브 패키지(`.tar`) | 10.0 MB | 전체 동기화 | 139.70 Mbps | 24.97 Mbps | 5.6x |
| 코드 및 아카이브 패키지(`.tar`) | 10.0 MB | 증분 동기화 | 83.38 Mbps | 26.02 Mbps | 3.2x |
| 비디오 파일(`.mp4`) | 15.0 MB | 전체 동기화 | 54.06 Mbps | 27.04 Mbps | 2.0x |
| 비디오 파일(`.mp4`) | 15.0 MB | 증분 동기화 | 216.83 Mbps | 21.51 Mbps | 10.1x |

### 대양 횡단 네트워크 P2P 다중 노드 풀 테스트(중국 → 미국 이중 노드, 지연 260ms)

다중 소스 시드 노드 스트라이프 동시 풀의 측정된 비교:

| 전송 시나리오 | 데이터 크기 | 단일 노드 시딩 | 2노드 P2P 동시 시딩 | 승수 증가 |
| :--- | :---: | :---: | :---: | :---: |
| 작은 파일 500개 | 1.97 MB | 5.28 Mbps | 7.87 Mbps | 1.5x |
| 코드 및 아카이브 패키지(`.tar`) | 30.0 MB | 56.35 Mbps | 83.17 Mbps | 1.5x |
| 비디오 파일(`.mp4`) | 30.0 MB | 27.00 Mbps | 39.46 Mbps | 1.5x |

### 대륙횡단 고속전용선 실측(유럽→북미, 지연시간 86ms)

| 전송 시나리오 | 데이터 크기 | `sync0` | `rsync` | 승수 증가 |
| :--- | :---: | :---: | :---: | :---: |
| 작은 파일 10,000개 | 77.6 MB | 688 Mbps | 128 Mbps | 5.4x |
| 코드 및 아카이브 패키지(`.tar`) | 512 MB | 2111 Mbps | 438 Mbps | 4.8x |
| 비디오 파일(`.mp4`) | 512 MB | 556 Mbps | 338 Mbps | 1.6x |
| 증분 동기화(1% 콘텐츠 수정) | 512 MB | 34133 Mbps | 2214 Mbps | 15.4x |

## 설치하다

### 방법 1: Cargo를 통해 설치

```bash
cargo install sync0
```

### 방법 2: 미리 컴파일된 바이너리 다운로드

[GitHub 릴리스](https://github.com/webc-site/sync0/releases)로 이동하여 해당 시스템(Linux, macOS, Windows, Android, FreeBSD, NetBSD 지원)에 대한 `.tar.xz` 압축 패키지를 다운로드하고 압축을 풀어 `PATH` 디렉터리에 넣습니다.

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## 빨리 시작하세요

`sync0`은 기본적으로 `~/.config/sync0`를 구성 디렉터리로 사용합니다(다른 디렉터리로 전환해야 하는 경우 `-c <디렉터리>`를 추가할 수 있음).

### 1. 로컬 노드 주소 보기 및 명함 설정

```bash
# 로컬 노드 주소 보기(처음으로 ~/.config/sync0에 키가 자동으로 생성됨)
sync0 id

# 개인 노드 명함 생성(256단어 이내의 사용자 이름, 도시, 이메일, 웹 사이트 및 서명 지원)
sync0 profile -u alice --region SHA --email alice@example.com --url https://example.com -b "Shanghai node"
```

### 2. 여러 파일 공유 그룹 게시 및 그룹 구성원 권한 설정(`.nt` 구성)

`sync0`은 [NestedText](https://nestedtext.org) (`.nt`) 형식으로 구성됩니다. 각 사용자에게는 별도의 `.nt` 구성 파일이 있습니다. `config.nt`의 `users_dir`을 통해 사용자 프로필 디렉터리를 지정할 수 있습니다(이 디렉터리의 모든 `.nt` 파일이 자동으로 포함됨). 업로드가 허용된 사용자와 경로 접두사는 모두 순서가 지정되지 않은 목록(`-`)을 사용합니다. **업로드 경로 접두사가 채워지지 않은 경우 기본적으로 공유의 루트 디렉터리에 업로드가 허용됩니다**.

1. `~/.config/sync0/users/` 디렉터리의 각 구성원에 대해 별도의 `.nt` 구성 파일을 만듭니다.

`~/.config/sync0/users/alice.nt`(읽기 전용 회원, 요금 제한 `80MB`, `rate`은 `GB`, `MB`, `KB` 단위 지원):

```yaml
id: 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c
perm: ro
rate: 80MB
```

`~/.config/sync0/users/bob.nt` (제한된 업로드 회원: `uploads/bob` 및 `shared/common` 디렉토리에만 업로드가 허용됨):

```yaml
id: 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f
perm: rw
write_prefix:
  - uploads/bob
  - shared/common
```

`~/.config/sync0/users/charlie.nt` (기본 구성원 업로드: `write_prefix`를 생략하면 기본적으로 공유의 루트 디렉터리에 업로드가 허용됩니다.)

```yaml
id: 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
perm: rw
```

2. `~/.config/sync0/config.nt`에서 사용자 디렉터리와 여러 파일 공유 그룹을 구성합니다.

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

공유 서비스 시작:

```bash
# 직접 시작(~/.config/sync0/config.nt 및 users_dir 디렉터리의 모든 .nt 사용자 구성을 자동으로 로드)
sync0 share --region SHA

# 또는 여러 공유 그룹을 빠르게 게시하고 명령줄을 통해 직접 그룹 구성원에게 권한을 부여하세요.
sync0 share dataset:./data team-vault:./vault \
  --auth \
  --allow-ro 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c \
  --allow-rw 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f:uploads/bob,shared/common \
  --allow-rw 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
```

### 3. 모든 콘텐츠를 가져오거나 여러 파일 및 디렉터리를 선택하세요(`sync0 pull`)

피어 노드에 연결하여 공유 그룹에서 사용 가능한 모든 시드 노드를 자동으로 얻고 여러 시드 노드에서 동시에 가져옵니다.

```bash
# 전체 데이터세트 공유 그룹을 로컬 ./local_data로 가져옵니다.
sync0 풀 데이터세트:// ./local_data -p <노드 주소>

# 다중 선택 동기화: 단일 파일 docs/readme.md와 하위 디렉터리 자산/아이콘, src/core를 동시에 가져옵니다.
sync0 풀 데이터 세트://docs/readme.md,assets/icons,src/core ./local_data -p <노드 주소>
```

### 4. 공유 그룹에 파일 또는 디렉터리 업로드(`sync0 push`)

업로드 권한(`rw` 또는 `admin`)이 있는 구성원은 로컬 파일이나 디렉터리를 원격 공유 그룹에 업로드할 수 있습니다.

```bash
# 루트 디렉터리 업로드 권한이 있는 구성원: 공유된 루트 디렉터리에 직접 업로드
sync0 push team-vault:// ./new_release -p <노드 주소>

# 특정 접두사(예: uploads/bob)에 권한이 부여된 구성원만: 허용된 경로 접두사에 업로드
sync0 push team-vault://uploads/bob/report.pdf ./report.pdf -p <노드 주소>
```

### 5. 자동 동기화 및 시드 구독(`sync0 sub`)

특정 노드의 공유 그룹에 가입한 후 로컬 서버는 수신 클라이언트이자 새로운 시딩 서버입니다. 소스 노드 파일이 업데이트되면 자동으로 동기화되어 다른 노드에 대한 청크 다운로드를 제공합니다.

```bash
# 전체 공유 그룹을 구독하고 자동으로 시드됩니다.
sync0 하위 데이터 세트:// ./local_mirror -p <노드 주소>

# 지정된 파일 및 하위 디렉터리를 구독하고 시드하려면 다중 선택만 가능
sync0 하위 데이터 세트://models/llama.onnx,configs ./local_mirror -p <노드 주소>
```

### 6. 다중 시스템 그룹 피어 동기화(`sync0 group`)

여러 시스템이 동일한 공유 그룹에 참여하고 서로 자동으로 사용 가능한 시드 노드를 검색하고 양방향 동기화를 유지합니다(전체 동기화 또는 파일 및 디렉터리의 다중 선택 지원).

```bash
# 그룹에 완전히 참여하고 양방향 동기화
sync0 그룹 team-docs:// ./docs -p <노드 주소>

# 다중 선택 동기화 그룹의 wal 디렉터리 및 Meta.json 파일만
sync0 그룹 team-docs://wal,meta.json ./docs -p <노드 주소>
```

### 7. 두 시스템 간의 지점간 직접 전송(`send` / `recv`)

그룹을 미리 구성할 필요가 없으며 임시로 두 컴퓨터 간에 직접 파일이나 디렉터리를 전송할 수 있습니다.

```bash
# 수신기 모니터링
sync0 recv 0.0.0.0:9000 ./backup

# 송신 측에서는 파일이나 디렉터리를 직접 전송합니다(과잉 파일을 정리하기 위한 --delete 미러 옵션, --rate 속도 제한).
sync0 send ./project 192.168.1.10:9000
```

### 8. 다중 언어 전환

`sync0` 75개 언어를 지원하며 기본적으로 시스템 언어 환경에 따라 자동으로 전환됩니다. `SYNC0_LANG`를 통해 수동으로 지정할 수도 있습니다.

```bash
SYNC0_LANG=en sync0 --help
SYNC0_LANG=ja sync0 --help
SYNC0_LANG=de sync0 --help
```