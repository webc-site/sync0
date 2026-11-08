#!/usr/bin/env bash
# sync0 广域网基准：c1 (x86_64) -> n1 (aarch64)
#
# 数据集（三档文件特征）：
#   linux-7.3.3.tar.xz  不可压缩：内核源码 xz 归档（已压缩，熵接近上限）
#   linux-7.3.3.tar     可压缩：xz 解压后的 tar（大量重复文本与零填充）
#   linux-7.3.3/        海量小文件：tar 解压后的内核源码树（约 8 万文件）
#
# 对比：sync0 单点直传 (send/recv) / sync0 多源 P2P (c1 + s1 双源做种, n1 pull) / rsync -az
# 仅 c1 -> n1，全部使用 /tmp，数据只在首次测试时下载并缓存。
#
# 传输路径：三个模式统一走 Tailscale 直连地址（100.x）。原因是机房安全组只放行
# SSH 等少数 TCP 端口，P2P 控制面需要任意 TCP 端口；Tailscale 是同一物理链路上的
# WireGuard 直连（tailscale ping 显示 via [2a0a:...]:41641，非 DERP 中继）。
# 同时打印公网 IPv6 ICMP RTT 与 Tailscale RTT 供对照。
#
# 用法：
#   ./bench.sh                 # 全量：检查/下载数据 -> 三种模式 -> 输出表格
#   SKIP_BUILD=1 ./bench.sh    # 复用已有交叉编译产物
#   REFRESH=1 ./bench.sh       # 重新下载数据集
#   MODES="sp p2p" ./bench.sh  # 只跑部分模式
set -uo pipefail

DIR=$(realpath "$0") && DIR=${DIR%/*}
cd "$DIR"

C1="${C1:-2605:a141:2288:4651::1}"
N1="${N1:-2a0a:4cc0:2000:34aa::1}"
S1="${S1:-s1}"
C1_TS="${C1_TS:-100.64.1.1}"
N1_TS="${N1_TS:-100.105.13.82}"
S1_TS="${S1_TS:-100.107.76.41}"
SSH_KEY="${SSH_KEY:-$HOME/.ssh/id_ed25519.ai}"
SSH_OPTS=(-i "$SSH_KEY" -o StrictHostKeyChecking=no -o ConnectTimeout=15
  -o ServerAliveInterval=15 -o ServerAliveCountMax=3
  -o ControlMaster=auto -o "ControlPath=/tmp/s0-ssh-%h" -o ControlPersist=120s)

BASE=/tmp/s0bench
DATA=$BASE/data
OUT=$BASE/out
KERNEL_URL="https://www.kernel.org/pub/linux/kernel/v7.x/linux-7.3.3.tar.xz"
CTRL_PORT="${CTRL_PORT:-60088}"
SP_PORT="${SP_PORT:-60090}"
MODES="${MODES:-sp p2p rsync}"
DATASETS="${DATASETS:-xz tar dir}"
SKIP_BUILD="${SKIP_BUILD:-0}"
REFRESH="${REFRESH:-0}"

X86_BIN="$DIR/target/x86_64-unknown-linux-gnu/release/sync0"
ARM_BIN="$DIR/target/aarch64-unknown-linux-gnu/release/sync0"
RESULT_TSV="$DIR/bench/kernel-results.tsv"
RESULT_JSON="$DIR/bench/kernel-results.json"

mkdir -p "$DIR/bench"

log() { printf '%s\n' ">>> $*"; }
warn() { printf '%s\n' "!!! $*" >&2; }

ssh_c1() { ssh "${SSH_OPTS[@]}" "root@$C1" "$@"; }
ssh_n1() { ssh "${SSH_OPTS[@]}" "root@$N1" "$@"; }
ssh_s1() { ssh "${SSH_OPTS[@]}" "root@$S1" "$@"; }

ssh_run() {
  local fn="$1"
  shift
  "$fn" 'bash -s' "$@"
}

# ---------------------------------------------------------------- 构建与部署
build_bins() {
  if [[ "$SKIP_BUILD" == "1" ]]; then
    log "跳过编译 (SKIP_BUILD=1)"
    return
  fi
  log "编译 x86_64-unknown-linux-gnu / aarch64-unknown-linux-gnu release 二进制"
  cargo zigbuild --release -p sync0 --target x86_64-unknown-linux-gnu -q || return 1
  cargo zigbuild --release -p sync0 --target aarch64-unknown-linux-gnu -q || return 1
}

deploy_one() {
  local host="$1" bin="$2"
  local sha remote
  sha=$(shasum -a 256 "$bin" | awk '{print $1}')
  remote=$(ssh "${SSH_OPTS[@]}" "root@$host" 'sha256sum /tmp/sync0 2>/dev/null | cut -d" " -f1' 2>/dev/null || true)
  if [[ "$sha" == "$remote" ]]; then
    log "跳过部署 ${host}（sha256 一致）"
    return
  fi
  log "部署 $(basename "$(dirname "$(dirname "$bin")")") -> $host:/tmp/sync0"
  zstd -19 -f "$bin" -o /tmp/s0_deploy.zst -q
  ssh "${SSH_OPTS[@]}" "root@$host" 'cat > /tmp/sync0.zst' < /tmp/s0_deploy.zst
  ssh "${SSH_OPTS[@]}" "root@$host" 'bash -s' <<'EOS'
pkill -9 -x sync0 2>/dev/null || true
zstd -df /tmp/sync0.zst -o /tmp/sync0
chmod +x /tmp/sync0
rm -f /tmp/sync0.zst
EOS
  rm -f /tmp/s0_deploy.zst
}

deploy() {
  deploy_one "$C1" "$X86_BIN"
  deploy_one "$N1" "$ARM_BIN"
  deploy_one "$S1" "$X86_BIN"
}

# ---------------------------------------------------------------- 数据集
prepare_datasets() {
  log "准备数据集 ${DATA}（首次下载，后续复用缓存）"
  ssh_c1 "REFRESH=$REFRESH bash -s" <<'EOS'
set -euo pipefail
B=/tmp/s0bench; D=$B/data
mkdir -p "$D" "$B/share_xz" "$B/share_tar"
cd "$D"
if [ "$REFRESH" = "1" ] && [ -f linux-7.3.3.tar.xz ]; then
  rm -f linux-7.3.3.tar linux-7.3.3.tar.xz
  rm -rf linux-7.3.3
fi
[ -f linux-7.3.3.tar.xz ] || curl -fL --retry 5 --retry-delay 2 \
  -o linux-7.3.3.tar.xz.part "https://www.kernel.org/pub/linux/kernel/v7.x/linux-7.3.3.tar.xz"
[ -f linux-7.3.3.tar.xz ] || mv linux-7.3.3.tar.xz.part linux-7.3.3.tar.xz
xz -t linux-7.3.3.tar.xz
[ -f linux-7.3.3.tar ] || xz -dkT0 linux-7.3.3.tar.xz
[ -d linux-7.3.3 ] || tar -xf linux-7.3.3.tar
ln -f linux-7.3.3.tar.xz "$B/share_xz/linux-7.3.3.tar.xz"
ln -f linux-7.3.3.tar "$B/share_tar/linux-7.3.3.tar"
mkdir -p "$B/cfg_c1" "$B/cfg_n1"
printf 'XZ=%s\nTAR=%s\nDIR=%s\nFILES=%s\n' \
  "$(stat -c %s linux-7.3.3.tar.xz)" \
  "$(stat -c %s linux-7.3.3.tar)" \
  "$(du -sb linux-7.3.3 | cut -f1)" \
  "$(find linux-7.3.3 -type f | wc -l)" > "$B/dataset.meta"
cat "$B/dataset.meta"
EOS
}

# ---------------------------------------------------------------- s1 做种副本
prepare_s1_data() {
  log "准备 s1 做种副本（本地下载 xz 后解压，避免再走一次广域网）"
  ssh_s1 "REFRESH=$REFRESH bash -s" <<'EOS'
set -euo pipefail
B=/tmp/s0bench; D=$B/data
mkdir -p "$D" "$B/s1/xz" "$B/s1/tar" "$B/s1/dir" "$B/cfg_s1"
cd "$D"
if [ "$REFRESH" = "1" ]; then
  rm -f linux-7.3.3.tar linux-7.3.3.tar.xz
  rm -rf linux-7.3.3
fi
[ -f linux-7.3.3.tar.xz ] || curl -fL --retry 5 --retry-delay 2 \
  -o linux-7.3.3.tar.xz.part "https://www.kernel.org/pub/linux/kernel/v7.x/linux-7.3.3.tar.xz"
[ -f linux-7.3.3.tar.xz ] || mv linux-7.3.3.tar.xz.part linux-7.3.3.tar.xz
xz -t linux-7.3.3.tar.xz
[ -f linux-7.3.3.tar ] || xz -dkT0 linux-7.3.3.tar.xz
[ -d linux-7.3.3 ] || tar -xf linux-7.3.3.tar
ln -f linux-7.3.3.tar.xz "$B/s1/xz/linux-7.3.3.tar.xz"
ln -f linux-7.3.3.tar "$B/s1/tar/linux-7.3.3.tar"
if [ -z "$(ls -A "$B/s1/dir" 2>/dev/null)" ]; then
  cp -al linux-7.3.3/. "$B/s1/dir/"
fi
echo "S1_READY xz=$(stat -c %s linux-7.3.3.tar.xz) tar=$(stat -c %s linux-7.3.3.tar) files=$(find linux-7.3.3 -type f | wc -l)"
EOS
}

# ---------------------------------------------------------------- 延时
measure_latency() {
  log "测量 c1 -> n1 延时"
  ssh_run ssh_c1 <<'EOS'
set -e
LC_ALL=C ping -c 20 -i 0.2 -W 3 -q 2a0a:4cc0:2000:34aa::1 | tail -1
echo "TS"
LC_ALL=C tailscale ping -c 5 100.105.13.82 2>/dev/null | tail -3
EOS
}

# ---------------------------------------------------------------- 计时工具
now_us() { python3 -c 'import time; print(int(time.time() * 1_000_000))'; }

remote_prepare_out() {
  local host_fn="$1" path="$2" kind="$3"
  "$host_fn" 'bash -s' <<EOS
pkill -9 -x sync0 2>/dev/null || true
rm -rf "$path"
mkdir -p "$(if [ "$kind" = dir ]; then echo "$path"; else dirname "$path"; fi)"
EOS
}

# ---------------------------------------------------------------- 单点直传
run_sp() {
  local src="$1" dst="$2" kind="$3"
  remote_prepare_out ssh_n1 "$dst" "$kind"
  ssh_run ssh_n1 <<EOS
nohup /tmp/sync0 bench-recv -q '0.0.0.0:$SP_PORT' "$dst" >/tmp/s0bench/recv.log 2>&1 </dev/null &
sleep 0.5
EOS
  local out
  out=$(ssh_run ssh_c1 <<EOS
/tmp/sync0 bench-send -q --delete "$src" "$N1_TS:$SP_PORT" | tail -n 1
EOS
)
  ssh_run ssh_n1 <<'EOS'
for i in $(seq 1 300); do pgrep -x sync0 >/dev/null || break; sleep 0.1; done
EOS
  printf '%s\n' "$out" | tail -n 1
}

# ---------------------------------------------------------------- P2P share/pull
run_p2p() {
  local share_root="$1" dst="$2" seed_local="$3"
  local share_out addr out us sub_out
  share_out=$(ssh_run ssh_c1 <<EOS
pkill -9 -x sync0 2>/dev/null || true
rm -f /tmp/s0bench/share.log
nohup /tmp/sync0 share "bench:$share_root" -l '$C1_TS:$CTRL_PORT' \
  -c /tmp/s0bench/cfg_c1 -u seeder >/tmp/s0bench/share.log 2>&1 </dev/null &
for i in \$(seq 1 200); do grep -q 'iroh=' /tmp/s0bench/share.log 2>/dev/null && break; sleep 0.05; done
sleep 0.2
cat /tmp/s0bench/share.log
EOS
)
  addr=$(printf '%s\n' "$share_out" | sed -n 's/.*iroh=\([^ ]*\).*/\1/p' | tail -n 1)
  if [[ -z "$addr" ]]; then
    warn "无法解析 share 地址：$share_out"
    return 1
  fi

  # s1 先订阅 c1 成为第二个做种源；预置好同名数据，initial pull 只做校验
  sub_out=$(ssh_s1 "S1_TS=$S1_TS ctrl_port=$CTRL_PORT addr=$addr seed_local=$seed_local bash -s" <<'EOS'
pkill -9 -x sync0 2>/dev/null || true
rm -f /tmp/s0bench/sub.log
mkdir -p "$seed_local" /tmp/s0bench/cfg_s1
nohup /tmp/sync0 sub 'bench://' "$seed_local" -p "$addr" \
  -l "$S1_TS:$ctrl_port" -c /tmp/s0bench/cfg_s1 -u seeder2 >/tmp/s0bench/sub.log 2>&1 </dev/null &
for i in $(seq 1 6000); do
  grep -q 'sub ready' /tmp/s0bench/sub.log && break
  grep -q '错误' /tmp/s0bench/sub.log && break
  sleep 0.1
done
cat /tmp/s0bench/sub.log
EOS
)
  if ! printf '%s\n' "$sub_out" | grep -q 'sub ready'; then
    warn "s1 做种失败：$sub_out"
    ssh_c1 'bash -c "pkill -9 -x sync0 2>/dev/null || true"' >/dev/null 2>&1 || true
    return 1
  fi
  log "s1 已加入 swarm 做种"

  remote_prepare_out ssh_n1 "$dst" "$kind"
  out=$(ssh_run ssh_n1 <<EOS
t0=\$(date +%s%N)
/tmp/sync0 pull 'bench://' "$dst" -p '$addr' \
  -c /tmp/s0bench/cfg_n1 -u leecher >/tmp/s0bench/pull.log 2>&1
rc=\$?
t1=\$(date +%s%N)
echo "US=\$(( (t1 - t0) / 1000 )) RC=\$rc"
cat /tmp/s0bench/pull.log
EOS
)
  ssh_c1 'bash -c "pkill -9 -x sync0 2>/dev/null || true"' >/dev/null 2>&1 || true
  ssh_s1 'bash -c "pkill -9 -x sync0 2>/dev/null || true"' >/dev/null 2>&1 || true
  us=$(printf '%s\n' "$out" | sed -n 's/^US=\([0-9]*\).*/\1/p' | tail -n 1)
  printf '%s\n' "$out" | grep -E '^pull ok|错误|error' || true
  printf 'US=%s\n' "${us:-0}"
}

# ---------------------------------------------------------------- rsync
run_rsync() {
  local src="$1" dst="$2" kind="$3"
  local src_arg dst_arg del
  remote_prepare_out ssh_n1 "$dst" "$kind"
  if [[ "$kind" == "dir" ]]; then
    src_arg="$src/"
    dst_arg="root@$N1_TS:$dst/"
    del="--delete"
  else
    src_arg="$src"
    dst_arg="root@$N1_TS:$dst"
    del=""
  fi
  ssh_run ssh_c1 <<EOS
t0=\$(date +%s%N)
rsync -az $del -e "ssh -o StrictHostKeyChecking=no -o BatchMode=yes -o ConnectTimeout=10" \
  "$src_arg" "$dst_arg"
t1=\$(date +%s%N)
echo "US=\$(( (t1 - t0) / 1000 ))"
EOS
}

# ---------------------------------------------------------------- 校验
hash_tree_remote() {
  local fn="$1" path="$2" kind="$3"
  "$fn" 'bash -s' <<EOS
set -e
if [ "$kind" = file ]; then
  sha256sum "$path" | cut -d' ' -f1
else
  cd "$path"
  find . -type f ! -name '*.s0tmp' ! -name '*.s0part' ! -name '*.s0_stage*' -print0 \
    | LC_ALL=C sort -z | xargs -0 -r sha256sum | sha256sum | cut -d' ' -f1
fi
EOS
}

verify() {
  local label="$1" dst="$2" kind="$3" expect="$4" actual
  actual=$(hash_tree_remote ssh_n1 "$dst" "$kind") || actual=""
  if [[ "$actual" == "$expect" ]]; then
    log "SHA-256 校验通过：$label"
    return 0
  fi
  warn "SHA-256 校验失败：$label"
  warn "  expect=$expect"
  warn "  actual=$actual"
  return 1
}

# ---------------------------------------------------------------- 主流程
build_bins || exit 1
deploy || exit 1
prepare_datasets || exit 1
prepare_s1_data || exit 1

cleanup() {
  local h
  for h in c1 s1 n1; do
    "ssh_$h" 'bash -c "pkill -9 -x sync0 2>/dev/null || true"' >/dev/null 2>&1 || true
  done
}
trap cleanup EXIT INT TERM

META=$(ssh_c1 'cat /tmp/s0bench/dataset.meta')
xz_bytes=$(printf '%s\n' "$META" | sed -n 's/^XZ=//p')
tar_bytes=$(printf '%s\n' "$META" | sed -n 's/^TAR=//p')
dir_bytes=$(printf '%s\n' "$META" | sed -n 's/^DIR=//p')
dir_files=$(printf '%s\n' "$META" | sed -n 's/^FILES=//p')

log "数据集：xz=${xz_bytes}B tar=${tar_bytes}B dir=${dir_bytes}B/${dir_files}文件"
measure_latency

# 预计算 c1 侧基准哈希
log "计算 c1 侧基准 SHA-256"
XZ_HASH=$(hash_tree_remote ssh_c1 "$DATA/linux-7.3.3.tar.xz" file)
TAR_HASH=$(hash_tree_remote ssh_c1 "$DATA/linux-7.3.3.tar" file)
DIR_HASH=$(hash_tree_remote ssh_c1 "$DATA/linux-7.3.3" dir)

: > "$RESULT_TSV"
printf 'dataset\tbytes\tfiles\tmode\tus\tnote\n' >> "$RESULT_TSV"

run_case() {
  local id="$1" label="$2" bytes="$3" files="$4" src="$5" share_root="$6"
  local kind="$7" expect="$8"
  local t0 t1 us mode dst p2p_dst

  for mode in $MODES; do
    case "$mode" in
      sp|p2p|rsync) ;;
      *) continue ;;
    esac
    # 文件模式 dst 是完整文件路径；目录模式 dst 是树根
    dst="$OUT/$mode/$(basename "$src")"
    if [[ "$kind" == "file" ]]; then
      # share 根目录包含单个文件，pull 目标是目录，文件落在目录内
      p2p_dst="$OUT/p2p"
    else
      p2p_dst="$dst"
    fi
    seed_local="$BASE/s1/$id"

    log "[$label] $mode 开始"
    t0=$(now_us)
    case "$mode" in
      sp) out=$(run_sp "$src" "$dst" "$kind"); us=$(printf '%s\n' "$out" | sed -n 's/.*"us":\([0-9]*\).*/\1/p' | tail -n 1); note="blocks=$(printf '%s\n' "$out" | sed -n 's/.*"blocks":\([0-9]*\).*/\1/p' | tail -n 1) retx=$(printf '%s\n' "$out" | sed -n 's/.*"retx":\([0-9]*\).*/\1/p' | tail -n 1)" ;;
      p2p) out=$(run_p2p "$share_root" "$p2p_dst" "$seed_local"); us=$(printf '%s\n' "$out" | sed -n 's/^US=//p' | tail -n 1); note=$(printf '%s\n' "$out" | grep -o 'peers=[0-9]*' | tail -n 1) ;;
      rsync) out=$(run_rsync "$src" "$dst" "$kind"); us=$(printf '%s\n' "$out" | sed -n 's/^US=//p' | tail -n 1); note="rsync" ;;
    esac
    t1=$(now_us)
    [[ -z "${us:-}" || "$us" == "0" ]] && us=$((t1 - t0))

    verify "$label/$mode" "$dst" "$kind" "$expect" || exit 1
    printf '%s\t%s\t%s\t%s\t%s\t%s\n' "$id" "$bytes" "$files" "$mode" "$us" "$note" >> "$RESULT_TSV"
    log "[$label] $mode 完成：$(awk -v us="$us" -v b="$bytes" 'BEGIN{printf "%.1f ms, %.2f Mbps", us/1000, (b*8)/us}')"
    ssh_n1 'bash -c "rm -rf /tmp/s0bench/out"' >/dev/null 2>&1 || true
  done
}

for ds in $DATASETS; do
  case "$ds" in
    xz)  run_case xz  "linux-7.3.3.tar.xz" "$xz_bytes" 1          "$DATA/linux-7.3.3.tar.xz" "$BASE/share_xz"   file "$XZ_HASH" ;;
    tar) run_case tar "linux-7.3.3.tar"    "$tar_bytes" 1         "$DATA/linux-7.3.3.tar"    "$BASE/share_tar"  file "$TAR_HASH" ;;
    dir) run_case dir "linux-7.3.3/"       "$dir_bytes" "$dir_files" "$DATA/linux-7.3.3"    "$DATA/linux-7.3.3" dir "$DIR_HASH" ;;
  esac
done

python3 - "$RESULT_TSV" "$RESULT_JSON" <<'PY'
import json, sys

tsv, js = sys.argv[1], sys.argv[2]
rows = []
with open(tsv) as f:
    next(f)
    for line in f:
        ds, b, files, mode, us, note = line.rstrip("\n").split("\t")
        rows.append({
            "dataset": ds, "bytes": int(b), "files": int(files),
            "mode": mode, "us": int(us), "mbps": round(int(b) * 8 / max(int(us), 1), 2),
            "note": note,
        })
with open(js, "w") as f:
    json.dump(rows, f, ensure_ascii=False, indent=2)

names = {"xz": "linux-7.3.3.tar.xz（不可压缩）", "tar": "linux-7.3.3.tar（可压缩）", "dir": "linux-7.3.3/（海量小文件）"}
print()
print("| 数据集 | 大小 | 文件数 | sync0 单点 | sync0 P2P | rsync -az | 单点/rsync | P2P/rsync |")
print("| :--- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |")
for ds in ("xz", "tar", "dir"):
    sub = [r for r in rows if r["dataset"] == ds]
    if not sub:
        continue
    d = {r["mode"]: r for r in sub}
    def cell(m):
        r = d.get(m)
        if not r:
            return "—"
        return f"**{r['mbps']:.2f} Mbps**<br>{r['us']/1000:.0f} ms"
    b = sub[0]["bytes"]; n = sub[0]["files"]
    size = f"{b/1024/1024:.1f} MiB" if b < 1024**3 else f"{b/1024**3:.2f} GiB"
    sp = d.get("sp"); p2p = d.get("p2p"); rs = d.get("rsync")
    r1 = f"{sp['mbps']/rs['mbps']:.2f}×" if sp and rs else "—"
    r2 = f"{p2p['mbps']/rs['mbps']:.2f}×" if p2p and rs else "—"
    nfiles = f"{n:,}" if n > 1 else "1"
    print(f"| {names[ds]} | {size} | {nfiles} | {cell('sp')} | {cell('p2p')} | {cell('rsync')} | {r1} | {r2} |")
print()
print(f"原始数据：{tsv}")
print(f"JSON：{js}")
PY
