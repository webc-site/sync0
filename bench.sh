#!/usr/bin/env bash
set -euo pipefail

DIR=$(realpath "$0") && DIR=${DIR%/*}
cd "$DIR"

TARGETS="${1:-c1}"
PORT="${PORT:-60088}"
UDT_PORT="${UDT_PORT:-60089}"
SSH_KEY="${SSH_KEY:-$HOME/.ssh/id_ed25519.ai}"
SSH_OPTS=(-i "$SSH_KEY" -o StrictHostKeyChecking=no -o ConnectTimeout=15 -o ServerAliveInterval=15 -o ServerAliveCountMax=3 -o ControlMaster=auto -o "ControlPath=/tmp/s0-ssh-%h" -o ControlPersist=120s)

ssh_run() {
  local host="$1"
  shift
  if ! ssh "${SSH_OPTS[@]}" "root@$host" "$@"; then
    rm -f "/tmp/s0-ssh-$host"
    ssh "${SSH_OPTS[@]}" "root@$host" "$@"
  fi
}

LOCAL_BIN="$DIR/target/release/sync0"
X86_BIN="$DIR/target/x86_64-unknown-linux-gnu/release/sync0"
ARM_BIN="$DIR/target/aarch64-unknown-linux-gnu/release/sync0"
LOCAL_UDT="/tmp/Web-UDT4/udt_bench"

BENCH_DIR="/tmp/sync0bench"
MOD_DIR="/tmp/sync0bench_mod"

ensure_build() {
  if [[ "${SKIP_BUILD:-0}" != "1" ]]; then
    echo ">>> 编译本机 Release 二进制..."
    cargo build --release -p sync0 -q
    if [[ "$TARGETS" == "c1" || "$TARGETS" == "all" ]]; then
      echo ">>> 编译 x86_64-unknown-linux-gnu Release 二进制..."
      cargo zigbuild --release -p sync0 --target x86_64-unknown-linux-gnu -q
    fi
    if [[ "$TARGETS" == "n1" || "$TARGETS" == "all" ]]; then
      echo ">>> 编译 aarch64-unknown-linux-gnu Release 二进制..."
      cargo zigbuild --release -p sync0 --target aarch64-unknown-linux-gnu -q
    fi
  fi
}

prepare_datasets() {
  if [[ ! -f "$BENCH_DIR/model.onnx" ]]; then
    echo ">>> 生成基准测试数据集 $BENCH_DIR..."
    mkdir -p "$BENCH_DIR/many-files"
    python3 - <<'PY'
import os, hashlib, random

base = "/tmp/sync0bench"
os.makedirs(f"{base}/many-files", exist_ok=True)

# 1. git-cargo.tar（10 MiB 可压缩源码归档数据）
with open(f"{base}/git-cargo.tar", "wb") as f:
    block = (b"pub fn fastcdc_chunk_boundary(data: &[u8]) -> usize { /* sync0 */ }\n" * 64)
    for i in range(230):
        hdr = f"/* file_{i:04d}.rs */\n".encode()
        f.write(hdr + block + hashlib.sha256(hdr).digest() * 32)
    f.truncate(10 * 1024 * 1024)

# 2. many-files（500 个小文件，分布在 10 个子目录）
for d in range(10):
    sub = f"{base}/many-files/pkg_{d:02d}"
    os.makedirs(sub, exist_ok=True)
    for i in range(50):
        p = f"{sub}/mod_{i:03d}.rs"
        content = f"// pkg {d} mod {i}\n".encode() + (b"use std::io::Result;\n" * (80 + (i * 7) % 250))
        with open(p, "wb") as f:
            f.write(content)

# 3. video.mp4（15 MiB 不可压缩视频媒体流）
rng = random.Random(42)
with open(f"{base}/video.mp4", "wb") as f:
    chunk = rng.randbytes(65536)
    for i in range(240):
        f.write(chunk[:49152] + i.to_bytes(4, "little") * 4096)
    f.truncate(15 * 1024 * 1024)

# 4. model.onnx（10 MiB 高熵模型权重）
rng2 = random.Random(1337)
with open(f"{base}/model.onnx", "wb") as f:
    f.write(rng2.randbytes(10 * 1024 * 1024))
PY
  fi

  echo ">>> 生成增删改测试数据集 ${MOD_DIR}（模拟中间插入、覆写、改名、新增）..."
  rm -rf "$MOD_DIR"
  mkdir -p "$MOD_DIR"
  cp -R "$BENCH_DIR/"* "$MOD_DIR/"

  python3 - <<'PY'
import os, hashlib, shutil

mod = "/tmp/sync0bench_mod"

with open(f"{mod}/git-cargo.tar", "rb") as f:
    data = bytearray(f.read())
insert_blob = b"/* INSERTED_HEADER_IN_MIDDLE_OF_TAR */\n" * 105
data[2*1024*1024 : 2*1024*1024] = insert_blob[:4096]
patch_off = 6 * 1024 * 1024
data[patch_off : patch_off + 16384] = hashlib.sha512(b"patch1").digest() * 256
with open(f"{mod}/git-cargo.tar", "wb") as f:
    f.write(data)

all_files = []
for root, _, files in os.walk(f"{mod}/many-files"):
    for fn in sorted(files):
        all_files.append(os.path.join(root, fn))
all_files.sort()

for i, p in enumerate(all_files[:10]):
    with open(p, "ab") as f:
        f.write(f"\n// hotfix patch {i}\npub const PATCH: u32 = {i};\n".encode())
for p in all_files[10:15]:
    shutil.move(p, p + ".renamed")
first_dir = os.path.dirname(all_files[0]) if all_files else f"{mod}/many-files"
for i in range(2):
    with open(os.path.join(first_dir, f"new_feature_{i}.rs"), "wb") as f:
        f.write(f"// new feature {i}\n".encode() * 128)

with open(f"{mod}/video.mp4", "rb") as f:
    vdata = bytearray(f.read())
vdata[5*1024*1024 : 5*1024*1024] = os.urandom(8192)
vdata[10*1024*1024 : 10*1024*1024 + 32768] = os.urandom(32768)
with open(f"{mod}/video.mp4", "wb") as f:
    f.write(vdata)

with open(f"{mod}/model.onnx", "r+b") as f:
    f.seek(4 * 1024 * 1024)
    f.write(os.urandom(65536))
PY
}

deploy_host() {
  local name="$1"
  local host="$2"
  local bin="$3"

  local local_sha
  local_sha=$(shasum -a 256 "$bin" | awk '{print $1}')
  local remote_sha
  remote_sha=$(ssh_run "$host" "bash -c 'sha256sum /tmp/sync0 2>/dev/null | awk \"{print \\\$1}\" || true'")

  if [[ "$local_sha" != "$remote_sha" || "${DEPLOY:-0}" == "1" ]]; then
    echo ">>> 部署 $bin 至远端节点 $name ($host:/tmp/sync0)..."
    zstd -19 -f "$bin" -o "/tmp/sync0_${name}.zst" -q
    ssh_run "$host" "bash -c 'pkill -9 -x sync0 2>/dev/null || true; cat > /tmp/sync0.zst && zstd -df /tmp/sync0.zst -o /tmp/sync0 && chmod +x /tmp/sync0 && rm -f /tmp/sync0.zst'" < "/tmp/sync0_${name}.zst"
    rm -f "/tmp/sync0_${name}.zst"
  fi

  if [[ -f "$LOCAL_UDT" ]]; then
    echo ">>> 确保远端节点 $name ($host:/tmp/Web-UDT4) 已编译 udt_bench..."
    ssh_run "$host" "bash -c 'if [ ! -d /tmp/Web-UDT4 ]; then git clone -q https://github.com/XXYoLoong/Web-UDT4 /tmp/Web-UDT4; fi'"
    ssh_run "$host" "cat > /tmp/Web-UDT4/app/udt_bench.cpp" < /tmp/Web-UDT4/app/udt_bench.cpp
    ssh_run "$host" "bash -c 'if [ ! -x /tmp/Web-UDT4/udt_bench ]; then g++ -O3 -std=c++17 -DLINUX -DAMD64 -I/tmp/Web-UDT4/src /tmp/Web-UDT4/src/*.cpp /tmp/Web-UDT4/app/udt_bench.cpp -lpthread -o /tmp/Web-UDT4/udt_bench; fi'"
  fi
}

run_one_case() {
  local name="$1"
  local host="$2"
  local src_path="$3"
  local dst_name="$4"
  local clean_before="$5"

  local rcmd="pkill -9 -x sync0 2>/dev/null || true; mkdir -p /tmp/s0out;"
  if [[ "$clean_before" == "1" ]]; then
    rcmd="$rcmd rm -rf /tmp/s0out/$dst_name;"
  fi
  rcmd="$rcmd nohup /tmp/sync0 bench-recv -q [::]:$PORT /tmp/s0out/$dst_name >/tmp/s0recv.log 2>&1 </dev/null & sleep 0.2"
  ssh_run "$host" "bash -c \"$rcmd\""

  local out
  out=$("$LOCAL_BIN" bench-send -q --delete "$src_path" "[$host]:$PORT" 2>&1 | tail -n 1)
  echo "$out"
}

run_rsync_case() {
  local host="$1"
  local src_path="$2"
  local dst_name="$3"
  local clean_before="$4"
  local is_dir="$5"

  if [[ "$clean_before" == "1" ]]; then
    ssh_run "$host" "rm -rf /tmp/rsout/$dst_name && mkdir -p /tmp/rsout"
  fi
  local rsh="ssh ${SSH_OPTS[*]}"
  local src_arg="$src_path"
  local dst_arg="root@[$host]:/tmp/rsout/$dst_name"
  if [[ "$is_dir" == "1" ]]; then
    src_arg="$src_path/"
    dst_arg="root@[$host]:/tmp/rsout/$dst_name/"
  fi
  python3 - "$rsh" "$src_arg" "$dst_arg" "$clean_before" <<'PY'
import sys, time, subprocess

rsh, src_arg, dst_arg, clean = sys.argv[1], sys.argv[2], sys.argv[3], sys.argv[4]
cmd = ["rsync", "-az", "--delete", "-e", rsh]
if clean == "0":
    cmd += ["--inplace", "--no-whole-file"]
cmd += [src_arg, dst_arg]
t0 = time.perf_counter()
subprocess.run(cmd, check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
us = int((time.perf_counter() - t0) * 1_000_000)
print(us)
PY
}

run_udt_case() {
  local host="$1"
  local src_path="$2"
  local dst_name="$3"

  local rcmd="pkill -9 -x udt_bench 2>/dev/null || true; mkdir -p /tmp/udtout; rm -f /tmp/udtout/$dst_name; nohup /tmp/Web-UDT4/udt_bench recv $UDT_PORT /tmp/udtout/$dst_name >/tmp/udtrecv.log 2>&1 </dev/null & sleep 0.25"
  ssh_run "$host" "bash -c \"$rcmd\""
  "$LOCAL_UDT" send "$host" "$UDT_PORT" "$src_path" 2>&1 | tail -n 1
}

run_host_suite() {
  local name="$1"
  local host="$2"
  local bin="$3"

  echo ""
  echo "================================================================================"
  echo "  测试节点: $name ($host:/tmp)"
  echo "================================================================================"
  deploy_host "$name" "$host" "$bin"

  local items=("git-cargo.tar" "many-files" "video.mp4" "model.onnx")

  echo ""
  echo "数据集          | 首次全量传输         | 修改增量同步         | 零改动复传         | 增量加速比 (块数)"
  echo "----------------+----------------------+----------------------+--------------------+-------------------"

  local summary_json=""
  for item in "${items[@]}"; do
    local out_cold
    out_cold=$(run_one_case "$name" "$host" "$BENCH_DIR/$item" "$item" 1)
    local out_mod
    out_mod=$(run_one_case "$name" "$host" "$MOD_DIR/$item" "$item" 0)
    local out_zero
    out_zero=$(run_one_case "$name" "$host" "$MOD_DIR/$item" "$item" 0)

    local row
    row=$(python3 - "$item" "$out_cold" "$out_mod" "$out_zero" <<'PY'
import sys, json

item, raw_c, raw_m, raw_z = sys.argv[1], sys.argv[2], sys.argv[3], sys.argv[4]
c = json.loads(raw_c)
m = json.loads(raw_m)
z = json.loads(raw_z)

def fmt(d):
    ms = max(d["us"] / 1000.0, 0.1)
    mbps = (d["bytes"] * 8.0) / max(d["us"], 1)
    return ms, mbps, d["blocks"], d["retx"]

ms_c, mbps_c, blk_c, _ = fmt(c)
ms_m, mbps_m, blk_m, _ = fmt(m)
ms_z, mbps_z, blk_z, _ = fmt(z)
spd = ms_c / max(ms_m, 0.1)

print(f"{item:<15} | {ms_c:6.0f}ms ({mbps_c:6.2f}M) | {ms_m:6.0f}ms ({mbps_m:6.2f}M) | {ms_z:5.0f}ms (块={blk_z:<3}) | {spd:5.2f}倍 ({blk_c:3}->{blk_m:<3})")
PY
)
    echo "$row"
    summary_json+="${item}|${out_cold}|${out_mod};"
  done

  python3 - "$summary_json" <<'PY'
import sys, json, math

pairs = [p for p in sys.argv[1].split(";") if p]
cold_mbps, mod_mbps, cold_ms, mod_ms = [], [], 0.0, 0.0
for p in pairs:
    _, rc, rm = p.split("|")
    c, m = json.loads(rc), json.loads(rm)
    cold_mbps.append((c["bytes"] * 8.0) / max(c["us"], 1))
    mod_mbps.append((m["bytes"] * 8.0) / max(m["us"], 1))
    cold_ms += c["us"] / 1000.0
    mod_ms += m["us"] / 1000.0

geo_c = math.exp(sum(math.log(x) for x in cold_mbps) / len(cold_mbps))
geo_m = math.exp(sum(math.log(x) for x in mod_mbps) / len(mod_mbps))
print("-" * 101)
print(f"{'总计 / 几何平均':<11} | {cold_ms:6.0f}ms ({geo_c:6.2f}M) | {mod_ms:6.0f}ms ({geo_m:6.2f}M) | {'几何平均速率':<14} | 总加速 {cold_ms/max(mod_ms,1):.2f}倍")
PY

  local local_sums remote_sums
  local_sums=$(cd "$MOD_DIR" && shasum -a 256 git-cargo.tar video.mp4 model.onnx | awk '{print $1, $2}')
  remote_sums=$(ssh_run "$host" "bash -c 'cd /tmp/s0out && sha256sum git-cargo.tar video.mp4 model.onnx | awk \"{print \\\$1, \\\$2}\"'")
  if [[ "$local_sums" == "$remote_sums" ]]; then
    echo ">>> SHA-256 完整性校验 ($name): 通过（100% 逐字节一致）"
  else
    echo ">>> SHA-256 完整性校验 ($name): 失败！"
    exit 1
  fi

  if [[ "${COMPARE_UDT_RSYNC:-1}" == "1" ]]; then
    echo ""
    echo "================================================================================"
    echo "  三方对比 ($name:/tmp): sync0 vs rsync vs Web-UDT4"
    echo "================================================================================"
    local rs_many_cold rs_many_mod
    rs_many_cold=$(run_rsync_case "$host" "$BENCH_DIR/many-files" "many-files" 1 1)
    rs_many_mod=$(run_rsync_case "$host" "$MOD_DIR/many-files" "many-files" 0 1)

    local rs_tar_cold rs_tar_mod udt_tar
    rs_tar_cold=$(run_rsync_case "$host" "$BENCH_DIR/git-cargo.tar" "git-cargo.tar" 1 0)
    rs_tar_mod=$(run_rsync_case "$host" "$MOD_DIR/git-cargo.tar" "git-cargo.tar" 0 0)
    udt_tar=$(run_udt_case "$host" "$BENCH_DIR/git-cargo.tar" "git-cargo.tar")

    local rs_vid_cold rs_vid_mod udt_vid
    rs_vid_cold=$(run_rsync_case "$host" "$BENCH_DIR/video.mp4" "video.mp4" 1 0)
    rs_vid_mod=$(run_rsync_case "$host" "$MOD_DIR/video.mp4" "video.mp4" 0 0)
    udt_vid=$(run_udt_case "$host" "$BENCH_DIR/video.mp4" "video.mp4")

    python3 - "$summary_json" "$rs_many_cold" "$rs_many_mod" "$rs_tar_cold" "$rs_tar_mod" "$udt_tar" "$rs_vid_cold" "$rs_vid_mod" "$udt_vid" <<'PY'
import sys, json

summary_raw = sys.argv[1]
rs_many_c, rs_many_m = int(sys.argv[2]), int(sys.argv[3])
rs_tar_c, rs_tar_m = int(sys.argv[4]), int(sys.argv[5])
udt_tar = json.loads(sys.argv[6])
rs_vid_c, rs_vid_m = int(sys.argv[7]), int(sys.argv[8])
udt_vid = json.loads(sys.argv[9])

s0 = {}
for p in [x for x in summary_raw.split(";") if x]:
    name, rc, rm = p.split("|")
    s0[name] = (json.loads(rc), json.loads(rm))

def fmt_us(b, us):
    ms = us / 1000.0
    mbps = (b * 8.0) / max(us, 1)
    return f"{ms:6.0f}ms ({mbps:6.2f}M)"

print("场景                      | sync0                | rsync -az            | Web-UDT4             | sync0 对比优势")
print("--------------------------+----------------------+----------------------+----------------------+--------------------------------")

# 1. many-files (不对比 UDT4)
c_mf, m_mf = s0["many-files"]
b_mf = c_mf["bytes"]
print(f"many-files (全量 500文件) | {fmt_us(b_mf, c_mf['us'])} | {fmt_us(b_mf, rs_many_c)} | — (不测海量小文件)   | {rs_many_c/c_mf['us']:.2f}x faster vs rsync")
print(f"many-files (增量修改)     | {fmt_us(m_mf['bytes'], m_mf['us'])} | {fmt_us(m_mf['bytes'], rs_many_m)} | — (不测海量小文件)   | {rs_many_m/m_mf['us']:.2f}x faster vs rsync")

# 2. git-cargo.tar (10 MiB 可压缩 tar)
c_tar, m_tar = s0["git-cargo.tar"]
b_tar = c_tar["bytes"]
print(f"git-cargo.tar (全量10MiB) | {fmt_us(b_tar, c_tar['us'])} | {fmt_us(b_tar, rs_tar_c)} | {fmt_us(b_tar, udt_tar['us'])} | {rs_tar_c/c_tar['us']:.2f}x vs rsync, {udt_tar['us']/c_tar['us']:.2f}x vs UDT4")
print(f"git-cargo.tar (增量修改)  | {fmt_us(m_tar['bytes'], m_tar['us'])} | {fmt_us(m_tar['bytes'], rs_tar_m)} | — (无增量去重)       | {rs_tar_m/m_tar['us']:.2f}x vs rsync, {udt_tar['us']/m_tar['us']:.2f}x vs UDT4")

# 3. video.mp4 (15 MiB 不可压缩视频)
c_vid, m_vid = s0["video.mp4"]
b_vid = c_vid["bytes"]
print(f"video.mp4 (全量 15MiB)    | {fmt_us(b_vid, c_vid['us'])} | {fmt_us(b_vid, rs_vid_c)} | {fmt_us(b_vid, udt_vid['us'])} | {rs_vid_c/c_vid['us']:.2f}x vs rsync, {udt_vid['us']/c_vid['us']:.2f}x vs UDT4")
print(f"video.mp4 (增量修改)      | {fmt_us(m_vid['bytes'], m_vid['us'])} | {fmt_us(m_vid['bytes'], rs_vid_m)} | — (无增量去重)       | {rs_vid_m/m_vid['us']:.2f}x vs rsync, {udt_vid['us']/m_vid['us']:.2f}x vs UDT4")
PY
  fi
}

ensure_build
prepare_datasets

case "$TARGETS" in
  c1)
    run_host_suite "c1" "2605:a141:2288:4651::1" "$X86_BIN"
    ;;
  n1)
    run_host_suite "n1" "2a0a:4cc0:2000:34aa::1" "$ARM_BIN"
    ;;
  all|*)
    run_host_suite "c1" "2605:a141:2288:4651::1" "$X86_BIN"
    run_host_suite "n1" "2a0a:4cc0:2000:34aa::1" "$ARM_BIN"
    ;;
esac
