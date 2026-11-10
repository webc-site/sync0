#!/usr/bin/env -S bun
// sync0 广域网与跨洋直连基准测试：
//   1. wan      : c1 (x86_64 US) -> n1 (aarch64 EU)，s1 (x86_64 EU) 协同做种（Linux 7.3.3 全量数据集）
//   2. local_c1 : 本机 (macOS/Linux) -> c1 (x86_64 US)，s1 协同做种（跨太平洋 Tailscale 实测数据集）
//
// 四档数据集特征：
//   xz        不可压缩高熵归档
//   tar       可压缩归档大文件
//   tar_delta 局部覆写 + 尾部追加增量同步
//   dir       海量源码小文件目录树
//
// 对比模式：
//   sp        sync0 单点直传 (bench-send / bench-recv)
//   p2p_1     sync0 P2P 单点拉取 (share / pull, 1 seeder)
//   p2p       sync0 P2P 双点并发拉取 (share + sub / pull, 2 seeders)
//   rsync     rsync 基准对照 (-a / -az / -az --inplace --no-whole-file)

import { $ } from 'zx'
import { basename, dirname, join } from 'node:path'
import yargs from 'yargs'
import { hideBin } from 'yargs/helpers'

$.quiet = true
$.verbose = false

const env = (key, def) => process.env[key] ?? def,
  argv = await yargs(hideBin(process.argv))
    .scriptName('bench.js')
    .usage('$0 [options]')
    .option('c1', { type: 'string', default: env('C1', '2605:a141:2288:4651::1'), describe: 'c1 数据地址（公网 IPv6）' })
    .option('n1', { type: 'string', default: env('N1', '2a0a:4cc0:2000:34aa::1'), describe: 'n1 数据地址（公网 IPv6）' })
    .option('s1', { type: 'string', default: env('S1', 's1'), describe: 's1 主机名' })
    .option('c1-ssh', { type: 'string', default: env('C1_SSH', '100.64.1.1'), describe: 'c1 SSH 地址' })
    .option('n1-ssh', { type: 'string', default: env('N1_SSH', '100.105.13.82'), describe: 'n1 SSH 地址' })
    .option('s1-ssh', { type: 'string', default: env('S1_SSH', '100.107.76.41'), describe: 's1 SSH 地址' })
    .option('c1-ts', { type: 'string', default: env('C1_TS', '100.64.1.1'), describe: 'c1 Tailscale 地址' })
    .option('n1-ts', { type: 'string', default: env('N1_TS', '100.105.13.82'), describe: 'n1 Tailscale 地址' })
    .option('s1-ts', { type: 'string', default: env('S1_TS', '100.107.76.41'), describe: 's1 Tailscale 地址' })
    .option('local-ts', { type: 'string', default: env('LOCAL_TS', ''), describe: '本机 Tailscale 地址（默认自动探测）' })
    .option('ssh-key', { type: 'string', default: env('SSH_KEY', `${process.env.HOME}/.ssh/id_ed25519.ai`), describe: 'SSH 私钥路径' })
    .option('ctrl-port', { type: 'string', default: env('CTRL_PORT', '60088'), describe: 'P2P 控制面 TCP 端口' })
    .option('sp-port', { type: 'string', default: env('SP_PORT', '60090'), describe: '单点直传 UDP 端口' })
    .option('routes', { type: 'string', default: env('ROUTES', 'wan local_c1'), describe: '测试链路，空格分隔：wan / local_c1' })
    .option('modes', { type: 'string', default: env('MODES', 'sp p2p_1 p2p rsync'), describe: '测试模式，空格分隔：sp / p2p_1 / p2p / rsync' })
    .option('datasets', { type: 'string', default: env('DATASETS', 'xz tar tar_delta dir'), describe: '数据集，空格分隔：xz / tar / tar_delta / dir' })
    .option('skip-build', { type: 'boolean', default: env('SKIP_BUILD', '0') === '1', describe: '复用已有编译产物' })
    .option('print-only', { type: 'boolean', default: env('PRINT_ONLY', '0') === '1', describe: '仅读取已有结果打印多表与几何平均值' })
    .option('refresh', { type: 'boolean', default: env('REFRESH', '0') === '1', describe: '强制重新生成/下载数据集' })
    .help('help')
    .alias('h', 'help')
    .version(false)
    .wrap(null)
    .strict()
    .parse(),
  C1 = argv.c1,
  N1 = argv.n1,
  C1_SSH = argv['c1-ssh'],
  N1_SSH = argv['n1-ssh'],
  S1_SSH = argv['s1-ssh'],
  C1_TS = argv['c1-ts'],
  N1_TS = argv['n1-ts'],
  S1_TS = argv['s1-ts'],
  SSH_KEY = argv['ssh-key'],
  SSH_OPT_LI = [
    '-i', SSH_KEY,
    '-o', 'StrictHostKeyChecking=no',
    '-o', 'ConnectTimeout=15',
    '-o', 'ServerAliveInterval=15',
    '-o', 'ServerAliveCountMax=3',
    '-o', 'ControlMaster=auto',
    '-o', 'ControlPath=/tmp/s0-ssh-%h',
    '-o', 'ControlPersist=120s',
  ],
  DIR = import.meta.dirname,
  BASE = '/tmp/s0bench',
  DATA = `${BASE}/data`,
  OUT = `${BASE}/out`,
  LBASE = '/tmp/s0bench_local',
  LDATA = `${LBASE}/data`,
  LOUT = `${LBASE}/out`,
  KERNEL_URL = 'https://www.kernel.org/pub/linux/kernel/v7.x/linux-7.3.3.tar.xz',
  CTRL_PORT = argv['ctrl-port'],
  SP_PORT = argv['sp-port'],
  ROUTE_LI = argv.routes.split(/\s+/).filter(Boolean),
  MODE_LI = argv.modes.split(/\s+/).filter(Boolean),
  DATASET_LI = argv.datasets.split(/\s+/).filter(Boolean),
  SKIP_BUILD = argv['skip-build'],
  PRINT_ONLY = argv['print-only'],
  REFRESH = argv.refresh ? '1' : '0',
  LOCAL_BIN = join(DIR, 'target/release/sync0'),
  X86_BIN = join(DIR, 'target/x86_64-unknown-linux-gnu/release/sync0'),
  ARM_BIN = join(DIR, 'target/aarch64-unknown-linux-gnu/release/sync0'),
  RESULT_YML = join(DIR, 'bench/kernel-results.yml'),
  ROUTE_ORDER_LI = ['wan', 'local_c1'],
  DATASET_ORDER_LI = ['xz', 'tar', 'tar_delta', 'dir'],
  MODE_ORDER_LI = ['sp', 'p2p_1', 'p2p', 'rsync']

const ERR_SHARE = 1, ERR_SEED = 2, ERR_HASH = 3

const log = (...a) => console.log('>>>', ...a),
  warn = (...a) => console.error('!!!', ...a),
  shq = (s) => `'${String(s).replaceAll("'", `'\\''`)}'`,
  geoMean = (xs) => {
    const valid = xs.map(Number).filter((x) => Number.isFinite(x) && x > 0)
    if (!valid.length) return 0
    return Math.exp(valid.reduce((acc, x) => acc + Math.log(x), 0) / valid.length)
  }

const localShell = async (script, var_map = {}) => {
  const prelude = Object.entries(var_map)
      .map(([key, val]) => `export ${key}=${shq(val)}`)
      .join('\n'),
    input = prelude ? `${prelude}\n${script}` : script,
    out = await $({ input })`bash -s`
  return out.stdout.trimEnd()
}

const remote = async (host, script, var_map = {}) => {
  const prelude = Object.entries(var_map)
      .map(([key, val]) => `export ${key}=${shq(val)}`)
      .join('\n'),
    input = prelude ? `${prelude}\n${script}` : script
  for (let attempt = 1; ; attempt++) {
    try {
      const out = await $({ input })`ssh ${SSH_OPT_LI} root@${host} bash -s`
      return out.stdout.trimEnd()
    } catch (err) {
      if (attempt < 4 && (err?.exitCode === 255 || String(err).includes('mux_client'))) {
        await new Promise((r) => setTimeout(r, 1000 * attempt))
        continue
      }
      throw err
    }
  }
}

const detectLocalTs = async () => {
  if (argv['local-ts']) return argv['local-ts']
  const out = await localShell(
    `tailscale ip -4 2>/dev/null | head -n1 || ifconfig 2>/dev/null | awk '/inet 100\\./ {print $2; exit}' || ip -4 addr 2>/dev/null | awk '/inet 100\\./ {sub(/\\/.*/, "", $2); print $2; exit}'`,
  ).catch(() => '')
  return out.trim() || '100.65.0.2'
}

const fileSha256 = async (path) =>
  new Bun.CryptoHasher('sha256')
    .update(new Uint8Array(await Bun.file(path).arrayBuffer()))
    .digest('hex')

// ---------------------------------------------------------------- 构建与部署
const binBuild = async () => {
  if (SKIP_BUILD) return log('跳过编译 (SKIP_BUILD=1)')
  if (ROUTE_LI.includes('local_c1')) {
    log('编译本机 release 二进制 (target/release/sync0)')
    await $`cargo build --release -p sync0 -q`
  }
  log('编译 Linux 远端 release 二进制')
  await $`cargo zigbuild --release -p sync0 --target x86_64-unknown-linux-gnu -q`
  if (ROUTE_LI.includes('wan')) {
    await $`cargo zigbuild --release -p sync0 --target aarch64-unknown-linux-gnu -q`
  }
}

const hostDeploy = async (host, bin) => {
  const sha = await fileSha256(bin),
    remote_sha = (
      await remote(host, 'sha256sum /tmp/sync0 2>/dev/null | cut -d" " -f1 || true').catch(() => '')
    ).trim()
  if (sha === remote_sha) return log(`跳过部署 ${host}（sha256 一致）`)

  const target_triple = basename(dirname(dirname(bin))),
    zst = `/tmp/s0_deploy_${host.replaceAll(/[^a-zA-Z0-9]/g, '_')}.zst`
  log(`部署 ${target_triple} -> ${host}:/tmp/sync0`)
  await $`zstd -19 -f ${bin} -o ${zst} -q`
  await $({ input: Buffer.from(await Bun.file(zst).arrayBuffer()) })`ssh ${SSH_OPT_LI} root@${host} 'cat > /tmp/sync0.zst'`
  await remote(
    host,
    [
      'pkill -9 -x sync0 2>/dev/null || true',
      'zstd -df /tmp/sync0.zst -o /tmp/sync0',
      'chmod +x /tmp/sync0',
      'rm -f /tmp/sync0.zst',
    ].join('\n'),
  )
  await $`rm -f ${zst}`
}

const binDeploy = async () => {
  const tasks = [hostDeploy(C1_SSH, X86_BIN)]
  if (MODE_LI.includes('p2p')) tasks.push(hostDeploy(S1_SSH, X86_BIN))
  if (ROUTE_LI.includes('wan')) tasks.push(hostDeploy(N1_SSH, ARM_BIN))
  await Promise.all(tasks)
}

// ---------------------------------------------------------------- WAN 数据集 (c1 -> n1)
const DOWNLOAD = `[ -f linux-7.3.3.tar.xz ] || curl -fL --retry 5 --retry-delay 2 \\
  -o linux-7.3.3.tar.xz.part ${shq(KERNEL_URL)}
[ -f linux-7.3.3.tar.xz ] || mv linux-7.3.3.tar.xz.part linux-7.3.3.tar.xz
xz -t linux-7.3.3.tar.xz`,
  DELTA = `if [ ! -f linux-7.3.3-delta.tar ]; then
  python3 -c 'import hashlib; open("extra_patch.bin","wb").write(b"".join(hashlib.sha256(f"s0-delta-{i}".encode()).digest() for i in range(131072)))'
  chmod 0644 extra_patch.bin
  cp --reflink=auto linux-7.3.3.tar linux-7.3.3-delta.tar.tmp 2>/dev/null || cp linux-7.3.3.tar linux-7.3.3-delta.tar.tmp
  dd if=extra_patch.bin of=linux-7.3.3-delta.tar.tmp bs=4096 seek=131072 conv=notrunc status=none
  tar --format=gnu --numeric-owner --owner=0 --group=0 --mtime='@1700000000' -rf linux-7.3.3-delta.tar.tmp extra_patch.bin
  mv -f linux-7.3.3-delta.tar.tmp linux-7.3.3-delta.tar
fi`

const datasetPrepareWan = async () => {
  log(`准备 WAN 数据集 ${DATA}（首次下载，后续复用缓存）`)
  return remote(
    C1_SSH,
    `set -euo pipefail
B=/tmp/s0bench; D=$B/data
mkdir -p "$D" "$B/share_xz" "$B/share_tar" "$B/share_tar_delta"
cd "$D"
if [ "$REFRESH" = "1" ] && [ -f linux-7.3.3.tar.xz ]; then
  rm -f linux-7.3.3.tar linux-7.3.3-delta.tar linux-7.3.3.tar.xz extra_patch.bin
  rm -rf linux-7.3.3
fi
${DOWNLOAD}
[ -f linux-7.3.3.tar ] || xz -dkT0 linux-7.3.3.tar.xz
${DELTA}
[ -d linux-7.3.3 ] || tar -xf linux-7.3.3.tar
rm -f linux-7.3.3/.s0_subs.bin "$B/share_xz/.s0_subs.bin" "$B/share_tar/.s0_subs.bin" "$B/share_tar_delta/.s0_subs.bin"
ln -f linux-7.3.3.tar.xz "$B/share_xz/linux-7.3.3.tar.xz"
ln -f linux-7.3.3.tar "$B/share_tar/linux-7.3.3.tar"
ln -f linux-7.3.3-delta.tar "$B/share_tar_delta/linux-7.3.3.tar"
mkdir -p "$B/cfg_c1" "$B/cfg_n1"
printf 'XZ=%s\\nTAR=%s\\nTAR_DELTA=%s\\nDIR=%s\\nFILES=%s\\n' \\
  "$(stat -c %s linux-7.3.3.tar.xz)" \\
  "$(stat -c %s linux-7.3.3.tar)" \\
  "$(stat -c %s linux-7.3.3-delta.tar)" \\
  "$(du -sb linux-7.3.3 | cut -f1)" \\
  "$(find linux-7.3.3 -type f | wc -l)" > "$B/dataset.meta"
cat "$B/dataset.meta"`,
    { REFRESH },
  )
}

const s1DataPrepareWan = async () => {
  if (!MODE_LI.includes('p2p')) return
  log('准备 s1 WAN 做种副本')
  return remote(
    S1_SSH,
    `set -euo pipefail
B=/tmp/s0bench; D=$B/data
mkdir -p "$D" "$B/s1/xz" "$B/s1/tar" "$B/s1/tar_delta" "$B/s1/dir" "$B/cfg_s1"
cd "$D"
if [ "$REFRESH" = "1" ]; then
  rm -f linux-7.3.3.tar linux-7.3.3-delta.tar linux-7.3.3.tar.xz extra_patch.bin
  rm -rf linux-7.3.3
fi
${DOWNLOAD}
[ -f linux-7.3.3.tar ] || xz -dkT0 linux-7.3.3.tar.xz
${DELTA}
[ -d linux-7.3.3 ] || tar -xf linux-7.3.3.tar
ln -f linux-7.3.3.tar.xz "$B/s1/xz/linux-7.3.3.tar.xz"
ln -f linux-7.3.3.tar "$B/s1/tar/linux-7.3.3.tar"
ln -f linux-7.3.3-delta.tar "$B/s1/tar_delta/linux-7.3.3.tar"
if [ -z "$(ls -A "$B/s1/dir" 2>/dev/null)" ]; then
  cp -al linux-7.3.3/. "$B/s1/dir/"
fi`,
    { REFRESH },
  )
}

const n1DataPrepareWan = async () => {
  log('准备 n1 接收端旧版基准缓存（供 tar_delta 增量预置）')
  return remote(
    N1_SSH,
    `set -euo pipefail
B=/tmp/s0bench; D=$B/data
mkdir -p "$D" "$B/cfg_n1"
cd "$D"
if [ "$REFRESH" = "1" ]; then
  rm -f linux-7.3.3.tar linux-7.3.3.tar.xz
fi
${DOWNLOAD}
[ -f linux-7.3.3.tar ] || xz -dkT0 linux-7.3.3.tar.xz`,
    { REFRESH },
  )
}

// ---------------------------------------------------------------- 本机 <-> c1 确定性跨平台数据集生成脚本
const LOCAL_GEN_PY = `
import hashlib, io, os, shutil, tarfile

B = "/tmp/s0bench_local"
D = os.path.join(B, "data")
os.makedirs(D, exist_ok=True)
for sub in ["share_xz", "share_tar", "share_tar_delta", "s1/xz", "s1/tar", "s1/tar_delta", "s1/dir", "cfg_local", "cfg_c1", "cfg_s1"]:
    os.makedirs(os.path.join(B, sub), exist_ok=True)

xz_path = os.path.join(D, "local.tar.xz")
tar_path = os.path.join(D, "local.tar")
delta_path = os.path.join(D, "local-delta.tar")
dir_path = os.path.join(D, "local_dir")
meta_path = os.path.join(B, "dataset.meta")

if os.environ.get("REFRESH") == "1":
    for p in [xz_path, tar_path, delta_path, meta_path]:
        if os.path.exists(p): os.remove(p)
    if os.path.exists(dir_path): shutil.rmtree(dir_path)

# 1. xz: 16 MiB 高熵不可压缩数据 (确定性流)
if not os.path.exists(xz_path):
    with open(xz_path, "wb") as f:
        for i in range(256):
            blk = hashlib.sha256(f"s0-xz-blk-{i}".encode()).digest()
            # 展开为 64 KiB 伪随机高熵块
            buf = bytearray(65536)
            st = blk
            for off in range(0, 65536, 32):
                st = hashlib.sha256(st + off.to_bytes(4, "little")).digest()
                buf[off:off+32] = st
            f.write(buf)

# 2. dir: 1,000 个源码小文件 (~15 MiB)
if not os.path.exists(dir_path):
    os.makedirs(dir_path, exist_ok=True)
    base_code = (
        "pub struct StreamEngine { pub cwnd: u64, pub pacing_bps: u64, pub rtt_us: u64 }\\n"
        "impl StreamEngine { pub fn on_ack(&mut self, delivered: u64, rtt: u64) { self.rtt_us = rtt; } }\\n"
    ).encode() * 72
    for i in range(1000):
        sub = os.path.join(dir_path, f"mod_{i // 40:02d}")
        os.makedirs(sub, exist_ok=True)
        fp = os.path.join(sub, f"unit_{i:04d}.rs")
        with open(fp, "wb") as f:
            f.write(f"// module unit {i:04d}\\n".encode())
            f.write(base_code)
            f.write(f"pub const UNIT_ID_{i}: u64 = {i * 2654435761};\\n".encode())

# 3. tar: ~64 MiB 可压缩 GNU tar 归档
if not os.path.exists(tar_path):
    base_line = (
        "/* sync0 adaptive congestion control & zero-copy io_uring pipeline */\\n"
        "pub fn compute_pacing_rate(bw: u64, rtt_us: u64, loss_ppm: u32) -> u64 { bw + rtt_us - u64::from(loss_ppm) }\\n"
    ).encode()
    chunk_text = (base_line * ((1048576 // len(base_line)) + 1))[:1048576]
    with tarfile.open(tar_path, "w", format=tarfile.GNU_FORMAT) as tf:
        for i in range(64):
            hdr = f"// archive entry {i:03d}\\n".encode()
            payload = hdr + chunk_text[len(hdr):]
            ti = tarfile.TarInfo(name=f"src/pkg_{i // 8:02d}/source_{i:03d}.rs")
            ti.size = len(payload)
            ti.mtime = 1700000000
            ti.mode = 0o644
            ti.uid = 0
            ti.gid = 0
            ti.uname = "root"
            ti.gname = "root"
            tf.addfile(ti, io.BytesIO(payload))

# 4. tar_delta: 局部覆写 512 KiB + 尾部追加 512 KiB 新条目
if not os.path.exists(delta_path):
    shutil.copyfile(tar_path, delta_path)
    patch = bytearray(524288)
    st = b"s0-delta-patch-seed"
    for off in range(0, len(patch), 32):
        st = hashlib.sha256(st + off.to_bytes(4, "little")).digest()
        patch[off:off+32] = st
    with tarfile.open(delta_path, "a", format=tarfile.GNU_FORMAT) as tf:
        ti = tarfile.TarInfo(name="src/appended_delta_patch.bin")
        ti.size = len(patch)
        ti.mtime = 1700000000
        ti.mode = 0o644
        ti.uid = 0
        ti.gid = 0
        ti.uname = "root"
        ti.gname = "root"
        tf.addfile(ti, io.BytesIO(bytes(patch)))
    with open(delta_path, "r+b") as f:
        f.seek(16 * (1048576 + 512) + 512)
        f.write(patch)

def link_force(src, dst):
    if os.path.exists(dst): os.remove(dst)
    os.link(src, dst)

link_force(xz_path, os.path.join(B, "share_xz/local.tar.xz"))
link_force(tar_path, os.path.join(B, "share_tar/local.tar"))
link_force(delta_path, os.path.join(B, "share_tar_delta/local.tar"))
link_force(xz_path, os.path.join(B, "s1/xz/local.tar.xz"))
link_force(tar_path, os.path.join(B, "s1/tar/local.tar"))
link_force(delta_path, os.path.join(B, "s1/tar_delta/local.tar"))

s1_dir = os.path.join(B, "s1/dir")
if not os.listdir(s1_dir):
    shutil.copytree(dir_path, s1_dir, dirs_exist_ok=True, copy_function=os.link)

subs_file = os.path.join(dir_path, ".s0_subs.bin")
if os.path.exists(subs_file):
    os.remove(subs_file)

dir_bytes = 0
dir_files = 0
for root, _, files in os.walk(dir_path):
    for fn in files:
        dir_files += 1
        dir_bytes += os.path.getsize(os.path.join(root, fn))

with open(meta_path, "w") as f:
    f.write(f"XZ={os.path.getsize(xz_path)}\\n")
    f.write(f"TAR={os.path.getsize(tar_path)}\\n")
    f.write(f"TAR_DELTA={os.path.getsize(delta_path)}\\n")
    f.write(f"DIR={dir_bytes}\\n")
    f.write(f"FILES={dir_files}\\n")
print(open(meta_path).read().strip())
`

const datasetPrepareLocal = async () => {
  log(`准备本机 -> c1 数据集 ${LDATA}`)
  const tasks = [
    localShell(`python3 -c ${shq(LOCAL_GEN_PY)}`, { REFRESH }),
    remote(C1_SSH, `python3 -c ${shq(LOCAL_GEN_PY)}`, { REFRESH }),
  ]
  if (MODE_LI.includes('p2p')) {
    tasks.push(remote(S1_SSH, `python3 -c ${shq(LOCAL_GEN_PY)}`, { REFRESH }))
  }
  const [local_meta] = await Promise.all(tasks)
  return local_meta
}

// ---------------------------------------------------------------- 计时与环境准备
const usNow = () => Math.round(performance.now() * 1000)

const outPrepareRemote = async (host, dst, kind, id, file_dst, base_tar) => {
  const dir = kind === 'dir' ? dst : dirname(dst)
  let script = `pkill -9 -x sync0 2>/dev/null || true
rm -rf ${shq(dst)}
mkdir -p ${shq(dir)}`
  if (id === 'tar_delta' && file_dst) {
    script += `
mkdir -p ${shq(dirname(file_dst))}
cp --reflink=auto ${shq(base_tar)} ${shq(file_dst)} 2>/dev/null || cp ${shq(base_tar)} ${shq(file_dst)}`
  }
  return remote(host, script)
}

// ---------------------------------------------------------------- WAN 传输执行 (c1 -> n1)
const spRunWan = async (src, dst, kind, id) => {
  await outPrepareRemote(N1_SSH, dst, kind, id, dst, '/tmp/s0bench/data/linux-7.3.3.tar')
  await remote(
    N1_SSH,
    `nohup /tmp/sync0 bench-recv -q '[::]:${SP_PORT}' ${shq(dst)} >/tmp/s0bench/recv.log 2>&1 </dev/null &
sleep 0.5`,
  )
  const out = await remote(
      C1_SSH,
      `/tmp/sync0 bench-send -q --delete ${shq(src)} "[${N1}]:${SP_PORT}" | tail -n 1`,
    ),
    st = JSON.parse(out.trim().split('\n').pop())
  await remote(
    N1_SSH,
    'for i in $(seq 1 300); do pgrep -x sync0 >/dev/null || break; sleep 0.1; done',
  )
  return [st.us, st.blocks, st.retx]
}

const p2pRunWan = async (share_root, dst, seed_local, kind, id, file_dst, use_s1 = true) => {
  await remote(S1_SSH, 'pkill -9 -x sync0 2>/dev/null || true').catch(() => {})
  const share_out = await remote(
      C1_SSH,
      `pkill -9 -x sync0 2>/dev/null || true
rm -f /tmp/s0bench/share.log ${shq(share_root)}/.s0_subs.bin
nohup /tmp/sync0 share "bench:${share_root}" -l ${shq(`${C1_TS}:${CTRL_PORT}`)} \\
  -c /tmp/s0bench/cfg_c1 -u seeder >/tmp/s0bench/share.log 2>&1 </dev/null &
for i in $(seq 1 200); do grep -q 'iroh=' /tmp/s0bench/share.log 2>/dev/null && break; sleep 0.05; done
sleep 0.2
cat /tmp/s0bench/share.log`,
    ),
    addr = share_out.match(/iroh=(\S+)/)?.[1]
  if (!addr) throw new Error(`${ERR_SHARE}: 无法解析 share 地址：${share_out}`)

  if (use_s1) {
    const sub_out = await remote(
      S1_SSH,
      `pkill -9 -x sync0 2>/dev/null || true
rm -f /tmp/s0bench/sub.log
mkdir -p ${shq(seed_local)} /tmp/s0bench/cfg_s1
nohup /tmp/sync0 sub 'bench://' ${shq(seed_local)} -p ${shq(addr)} \\
  -l ${shq(`${S1_TS}:${CTRL_PORT}`)} -c /tmp/s0bench/cfg_s1 -u seeder2 >/tmp/s0bench/sub.log 2>&1 </dev/null &
for i in $(seq 1 6000); do
  grep -q 'sub ready' /tmp/s0bench/sub.log && break
  grep -q '错误' /tmp/s0bench/sub.log && break
  sleep 0.1
done
cat /tmp/s0bench/sub.log`,
    )
    if (!sub_out.includes('sub ready')) {
      await remote(C1_SSH, 'pkill -9 -x sync0 2>/dev/null || true').catch(() => {})
      throw new Error(`${ERR_SEED}: s1 做种失败：${sub_out}`)
    }
  }

  await outPrepareRemote(N1_SSH, dst, kind, id, file_dst, '/tmp/s0bench/data/linux-7.3.3.tar')
  const started = usNow(),
    out = await remote(
      N1_SSH,
      `t0=$(date +%s%N)
/tmp/sync0 pull 'bench://' ${shq(dst)} -p ${shq(addr)} \\
  -c /tmp/s0bench/cfg_n1 -u leecher >/tmp/s0bench/pull.log 2>&1
rc=$?
t1=$(date +%s%N)
echo "US=$(( (t1 - t0) / 1000 )) RC=$rc"
cat /tmp/s0bench/pull.log`,
    ),
    wall_us = usNow() - started,
    us = Number(out.match(/^US=(\d+)/m)?.[1] ?? 0) || wall_us,
    peers = out.match(/peers=(\d+)/)?.[1] ?? '?'
  await Promise.all([
    remote(
      C1_SSH,
      `pkill -9 -x sync0 2>/dev/null || true; rm -f ${shq(share_root)}/.s0_subs.bin`,
    ).catch(() => {}),
    remote(S1_SSH, 'pkill -9 -x sync0 2>/dev/null || true').catch(() => {}),
  ])
  return [us, peers]
}

const rsyncRunWan = async (src, dst, kind, id) => {
  await outPrepareRemote(N1_SSH, dst, kind, id, dst, '/tmp/s0bench/data/linux-7.3.3.tar')
  const flags =
      id === 'xz' ? '-a'
      : id === 'tar_delta' ? '-az --inplace --no-whole-file'
      : '-az',
    src_arg = kind === 'dir' ? `${src}/` : src,
    dst_arg = kind === 'dir' ? `root@[${N1}]:${dst}/` : `root@[${N1}]:${dst}`,
    del = kind === 'dir' ? '--delete' : '',
    out = await remote(
      C1_SSH,
      `t0=$(date +%s%N)
rsync ${flags} ${del} -e "ssh -o StrictHostKeyChecking=no -o BatchMode=yes -o ConnectTimeout=10" \\
  ${shq(src_arg)} ${shq(dst_arg)}
t1=$(date +%s%N)
echo "US=$(( (t1 - t0) / 1000 ))"`,
    ),
    us = Number(out.match(/^US=(\d+)/m)?.[1] ?? 0),
    note =
      id === 'xz' ? 'rsync -a'
      : id === 'tar_delta' ? 'rsync -az --inplace --no-whole-file'
      : 'rsync -az'
  return [us, note]
}

// ---------------------------------------------------------------- 本机 -> c1 传输执行 (local_c1)
const spRunLocal = async (src, dst, kind, id) => {
  await outPrepareRemote(C1_SSH, dst, kind, id, dst, `${LDATA}/local.tar`)
  await remote(
    C1_SSH,
    `nohup /tmp/sync0 bench-recv -q '0.0.0.0:${SP_PORT}' ${shq(dst)} >${LBASE}/recv.log 2>&1 </dev/null &
sleep 0.4`,
  )
  const out = await localShell(
      `${shq(LOCAL_BIN)} bench-send -q --delete ${shq(src)} ${shq(`${C1_TS}:${SP_PORT}`)} | tail -n 1`,
    ),
    st = JSON.parse(out.trim().split('\n').pop())
  await remote(
    C1_SSH,
    'for i in $(seq 1 300); do pgrep -x sync0 >/dev/null || break; sleep 0.1; done',
  )
  return [st.us, st.blocks, st.retx]
}

const p2pRunLocal = async (local_ts, share_root, dst, seed_local, kind, id, file_dst, use_s1 = true) => {
  await localShell('pkill -9 -x sync0 2>/dev/null || true').catch(() => {})
  await remote(S1_SSH, 'pkill -9 -x sync0 2>/dev/null || true').catch(() => {})
  const share_out = await localShell(
      `rm -f ${LBASE}/share.log ${shq(share_root)}/.s0_subs.bin
nohup ${shq(LOCAL_BIN)} share "bench:${share_root}" -l ${shq(`${local_ts}:${CTRL_PORT}`)} \\
  -c ${LBASE}/cfg_local -u seeder >${LBASE}/share.log 2>&1 </dev/null &
for i in $(seq 1 200); do grep -q 'iroh=' ${LBASE}/share.log 2>/dev/null && break; sleep 0.05; done
sleep 0.2
cat ${LBASE}/share.log`,
    ),
    addr = share_out.match(/iroh=(\S+)/)?.[1]
  if (!addr) throw new Error(`${ERR_SHARE}: 本机 share 地址解析失败：${share_out}`)

  if (use_s1) {
    const sub_out = await remote(
      S1_SSH,
      `pkill -9 -x sync0 2>/dev/null || true
rm -f ${LBASE}/sub.log
mkdir -p ${shq(seed_local)} ${LBASE}/cfg_s1
nohup /tmp/sync0 sub 'bench://' ${shq(seed_local)} -p ${shq(addr)} \\
  -l ${shq(`${S1_TS}:${CTRL_PORT}`)} -c ${LBASE}/cfg_s1 -u seeder2 >${LBASE}/sub.log 2>&1 </dev/null &
for i in $(seq 1 6000); do
  grep -q 'sub ready' ${LBASE}/sub.log && break
  grep -q '错误' ${LBASE}/sub.log && break
  sleep 0.1
done
cat ${LBASE}/sub.log`,
    )
    if (!sub_out.includes('sub ready')) {
      await localShell('pkill -9 -x sync0 2>/dev/null || true').catch(() => {})
      throw new Error(`${ERR_SEED}: s1 做种失败：${sub_out}`)
    }
  }

  await outPrepareRemote(C1_SSH, dst, kind, id, file_dst, `${LDATA}/local.tar`)
  const started = usNow(),
    out = await remote(
      C1_SSH,
      `t0=$(date +%s%N)
/tmp/sync0 pull 'bench://' ${shq(dst)} -p ${shq(addr)} \\
  -c ${LBASE}/cfg_c1 -u leecher >${LBASE}/pull.log 2>&1
rc=$?
t1=$(date +%s%N)
echo "US=$(( (t1 - t0) / 1000 )) RC=$rc"
cat ${LBASE}/pull.log`,
    ),
    wall_us = usNow() - started,
    us = Number(out.match(/^US=(\d+)/m)?.[1] ?? 0) || wall_us,
    peers = out.match(/peers=(\d+)/)?.[1] ?? '?'
  await Promise.all([
    localShell(
      `pkill -9 -x sync0 2>/dev/null || true; rm -f ${shq(share_root)}/.s0_subs.bin`,
    ).catch(() => {}),
    remote(S1_SSH, 'pkill -9 -x sync0 2>/dev/null || true').catch(() => {}),
  ])
  return [us, peers]
}

const rsyncRunLocal = async (src, dst, kind, id) => {
  await outPrepareRemote(C1_SSH, dst, kind, id, dst, `${LDATA}/local.tar`)
  const flags =
      id === 'xz' ? '-a'
      : id === 'tar_delta' ? '-az --inplace --no-whole-file'
      : '-az',
    src_arg = kind === 'dir' ? `${src}/` : src,
    dst_arg = kind === 'dir' ? `root@${C1_TS}:${dst}/` : `root@${C1_TS}:${dst}`,
    del = kind === 'dir' ? '--delete' : '',
    ssh_cmd = `ssh -i ${SSH_KEY} -o StrictHostKeyChecking=no -o BatchMode=yes -o ConnectTimeout=10`,
    started = usNow()
  await localShell(`rsync ${flags} ${del} -e ${shq(ssh_cmd)} ${shq(src_arg)} ${shq(dst_arg)}`)
  const us = usNow() - started,
    note =
      id === 'xz' ? 'rsync -a'
      : id === 'tar_delta' ? 'rsync -az --inplace --no-whole-file'
      : 'rsync -az'
  return [us, note]
}

// ---------------------------------------------------------------- SHA-256 校验
const remoteTreeHash = async (host, path, kind) => {
  const script =
    kind === 'file'
      ? `set -e; sha256sum ${shq(path)} | cut -d' ' -f1`
      : `set -e
cd ${shq(path)}
find . -type f ! -name '*.s0tmp' ! -name '*.s0part' ! -name '*.s0_stage*' ! -name '.s0_subs.bin' -print0 \\
  | LC_ALL=C sort -z | xargs -0 -r sha256sum | sha256sum | cut -d' ' -f1`
  return (await remote(host, script)).trim()
}

const hashVerify = async (host, label, dst, kind, expect) => {
  const actual = await remoteTreeHash(host, dst, kind).catch(() => '')
  if (actual === expect) return log(`SHA-256 校验通过：${label}`)
  throw new Error(`${ERR_HASH}: SHA-256 校验失败：${label}\n  expect=${expect}\n  actual=${actual}`)
}

// ---------------------------------------------------------------- 结果读写与多表 + 几何平均输出
const rowsRead = async () => {
  try {
    const raw = await Bun.file(RESULT_YML).text(),
      parsed = Bun.YAML.parse(raw)
    return Array.isArray(parsed)
      ? parsed.map((row) => ({
          route: String(row.链路 ?? row.route ?? 'wan'),
          dataset: String(row.数据集 ?? row.dataset),
          bytes: +(row.大小字节 ?? row.bytes),
          files: +(row.文件数 ?? row.files),
          mode: String(row.模式 ?? row.mode),
          us: +(row.耗时微秒 ?? row.us),
          mbps: +(((row.大小字节 ?? row.bytes) * 8) / Math.max(row.耗时微秒 ?? row.us, 1)).toFixed(2),
          note: String(row.备注 ?? row.note ?? ''),
        }))
      : []
  } catch {
    return []
  }
}

const rowsSort = (row_li) =>
  row_li.sort(
    (a, b) =>
      ROUTE_ORDER_LI.indexOf(a.route) - ROUTE_ORDER_LI.indexOf(b.route) ||
      DATASET_ORDER_LI.indexOf(a.dataset) - DATASET_ORDER_LI.indexOf(b.dataset) ||
      MODE_ORDER_LI.indexOf(a.mode) - MODE_ORDER_LI.indexOf(b.mode),
  )

const rowsSave = async (row_li) => {
  const yml_li = row_li.map((row) => ({
    链路: row.route,
    数据集: row.dataset,
    大小字节: row.bytes,
    文件数: row.files,
    模式: row.mode,
    耗时微秒: row.us,
    速率Mbps: row.mbps,
    备注: row.note,
  }))
  await Bun.write(RESULT_YML, Bun.YAML.stringify(yml_li, null, 2))
}

const tablePrint = (row_li) => {
  const route_titles = {
      wan: '跨洲广域网 (`c1 -> n1`, `s1` 协同，Linux Kernel 7.3.3)',
      local_c1: '本机跨太平洋直连 (`本机 -> c1`, `s1` 协同，RTT ~180 ms)',
    },
    ds_names = {
      wan: {
        xz: 'linux-7.3.3.tar.xz（不可压缩）',
        tar: 'linux-7.3.3.tar（可压缩）',
        tar_delta: 'linux-7.3.3.tar（局部改动增量）',
        dir: 'linux-7.3.3/（海量小文件）',
      },
      local_c1: {
        xz: 'local.tar.xz（不可压缩）',
        tar: 'local.tar（可压缩）',
        tar_delta: 'local.tar（局部改动增量）',
        dir: 'local_dir/（1,000 个小文件）',
      },
    },
    cell = (row, bold = true) => (row ? (bold ? `**${row.mbps.toFixed(2)}**` : row.mbps.toFixed(2)) : '—'),
    tables = [
      ['sp', 'sync0 单点 (`sp`)', '单点 / rsync'],
      ['p2p_1', 'sync0 P2P 单点 (`p2p_1`)', 'P2P 单点 / rsync'],
      ['p2p', 'sync0 P2P 双点 (`p2p`)', 'P2P 双点 / rsync'],
    ],
    all_ratios = [],
    summary_rows = []

  for (const route of ROUTE_ORDER_LI) {
    const route_rows = row_li.filter((r) => r.route === route)
    if (!route_rows.length) continue
    console.log()
    console.log(`## ${route_titles[route] ?? route}`)
    const route_ratios = []

    for (const [mode_key, col_title, ratio_title] of tables) {
      const has_mode = route_rows.some((r) => r.mode === mode_key)
      if (!has_mode) continue
      const mode_ratios = [],
        mode_s0_mbps = [],
        mode_rs_mbps = []
      console.log()
      console.log(`### ${col_title} vs rsync`)
      console.log()
      console.log(`| 数据集 | 大小 | ${col_title}<br>(Mbps) | rsync<br>(Mbps) | ${ratio_title} |`)
      console.log('| :--- | ---: | ---: | ---: | ---: |')
      for (const ds of DATASET_ORDER_LI) {
        const sub_li = route_rows.filter((row) => row.dataset === ds)
        if (!sub_li.length) continue
        const by_mode = Object.fromEntries(sub_li.map((row) => [row.mode, row])),
          { bytes: b } = sub_li[0],
          size = b < 1024 ** 3 ? `${Math.round(b / 1024 / 1024)} MB` : `${(b / 1024 ** 3).toFixed(1)} GB`,
          cur = by_mode[mode_key],
          rs = by_mode.rsync
        let ratio_str = '—'
        if (cur && rs && rs.mbps > 0) {
          const r = cur.mbps / rs.mbps
          mode_ratios.push(r)
          route_ratios.push(r)
          all_ratios.push(r)
          mode_s0_mbps.push(cur.mbps)
          mode_rs_mbps.push(rs.mbps)
          ratio_str = `**${r.toFixed(2)}×**`
        }
        const label = ds_names[route]?.[ds] ?? ds
        console.log(
          `| ${label} | ${size} | ${cell(cur, true)} | ${cell(rs, false)} | ${ratio_str} |`,
        )
      }
      if (mode_ratios.length) {
        const gm_ratio = geoMean(mode_ratios),
          gm_s0 = geoMean(mode_s0_mbps),
          gm_rs = geoMean(mode_rs_mbps)
        console.log(
          `| **几何平均** | — | **${gm_s0.toFixed(2)}** | ${gm_rs.toFixed(2)} | **${gm_ratio.toFixed(2)}×** |`,
        )
        summary_rows.push({
          route: route === 'wan' ? '`c1 -> n1` (跨洲广域网)' : '`本机 -> c1` (跨洋直连)',
          mode: col_title,
          count: mode_ratios.length,
          gm_s0,
          gm_rs,
          gm_ratio,
        })
      }
    }

    if (route_ratios.length) {
      summary_rows.push({
        route: route === 'wan' ? '**`c1 -> n1` 全模式综合**' : '**`本机 -> c1` 全模式综合**',
        mode: '**链路几何平均**',
        count: route_ratios.length,
        gm_s0: null,
        gm_rs: null,
        gm_ratio: geoMean(route_ratios),
      })
    }
  }

  if (summary_rows.length) {
    const total_gm = geoMean(all_ratios)
    console.log()
    console.log('## 综合几何平均加速比汇总 (Geometric Mean vs `rsync`)')
    console.log()
    console.log('| 測試链路 | 模式 | 场景数 | `sync0` 几何平均<br>(Mbps) | `rsync` 几何平均<br>(Mbps) | 几何平均加速比 (`sync0 / rsync`) |')
    console.log('| :--- | :--- | ---: | ---: | ---: | ---: |')
    for (const s of summary_rows) {
      const s0_col = s.gm_s0 ? `**${s.gm_s0.toFixed(2)}**` : '—',
        rs_col = s.gm_rs ? `${s.gm_rs.toFixed(2)}` : '—'
      console.log(
        `| ${s.route} | ${s.mode} | ${s.count} | ${s0_col} | ${rs_col} | **${s.gm_ratio.toFixed(2)}×** |`,
      )
    }
    console.log(
      `| **全链路全场景总计** | **综合几何平均 (Overall GeoMean)** | **${all_ratios.length}** | — | — | **${total_gm.toFixed(2)}×** |`,
    )
  }

  console.log()
  console.log(`原始数据：${RESULT_YML}`)
}

// ---------------------------------------------------------------- 路由测试主流程
const runWanRoute = async (row_li) => {
  await datasetPrepareWan()
  await s1DataPrepareWan()
  await n1DataPrepareWan()
  log('WAN (c1 / s1 / n1) 数据准备完成')

  const meta = Object.fromEntries(
      (await remote(C1_SSH, 'cat /tmp/s0bench/dataset.meta'))
        .split('\n')
        .filter(Boolean)
        .map((line) => line.split('=')),
    ),
    [xz_hash, tar_hash, tar_delta_hash, dir_hash] = await Promise.all([
      remoteTreeHash(C1_SSH, `${DATA}/linux-7.3.3.tar.xz`, 'file'),
      remoteTreeHash(C1_SSH, `${DATA}/linux-7.3.3.tar`, 'file'),
      remoteTreeHash(C1_SSH, `${DATA}/linux-7.3.3-delta.tar`, 'file'),
      remoteTreeHash(C1_SSH, `${DATA}/linux-7.3.3`, 'dir'),
    ]),
    all = {
      xz: {
        label: 'wan/linux-7.3.3.tar.xz',
        bytes: +meta.XZ,
        files: 1,
        src: `${DATA}/linux-7.3.3.tar.xz`,
        share_root: `${BASE}/share_xz`,
        kind: 'file',
        hash: xz_hash,
      },
      tar: {
        label: 'wan/linux-7.3.3.tar',
        bytes: +meta.TAR,
        files: 1,
        src: `${DATA}/linux-7.3.3.tar`,
        share_root: `${BASE}/share_tar`,
        kind: 'file',
        hash: tar_hash,
      },
      tar_delta: {
        label: 'wan/linux-7.3.3.tar (delta)',
        bytes: +meta.TAR_DELTA,
        files: 1,
        src: `${BASE}/share_tar_delta/linux-7.3.3.tar`,
        share_root: `${BASE}/share_tar_delta`,
        kind: 'file',
        hash: tar_delta_hash,
      },
      dir: {
        label: 'wan/linux-7.3.3/',
        bytes: +meta.DIR,
        files: +meta.FILES,
        src: `${DATA}/linux-7.3.3`,
        share_root: `${DATA}/linux-7.3.3`,
        kind: 'dir',
        hash: dir_hash,
      },
    }

  for (const id of DATASET_LI) {
    const ds = all[id]
    if (!ds) continue
    for (const mode of MODE_LI) {
      const dst = `${OUT}/${mode}/${basename(ds.src)}`,
        p2p_dst = ds.kind === 'file' ? `${OUT}/${mode}` : dst,
        seed_local = `${BASE}/s1/${id}`
      log(`[${ds.label}] ${mode} 开始`)
      const started = usNow()
      let us = 0, note = ''
      if (mode === 'sp') {
        const [sp_us, blocks, retx] = await spRunWan(ds.src, dst, ds.kind, id)
        us = sp_us
        note = `blocks=${blocks} retx=${retx}`
      } else if (mode === 'p2p_1') {
        const [p2p_us, peers] = await p2pRunWan(ds.share_root, p2p_dst, seed_local, ds.kind, id, dst, false)
        us = p2p_us
        note = `peers=${peers}`
      } else if (mode === 'p2p') {
        const [p2p_us, peers] = await p2pRunWan(ds.share_root, p2p_dst, seed_local, ds.kind, id, dst, true)
        us = p2p_us
        note = `peers=${peers}`
      } else if (mode === 'rsync') {
        const [rsync_us, rsync_note] = await rsyncRunWan(ds.src, dst, ds.kind, id)
        us = rsync_us
        note = rsync_note
      } else continue
      if (!us) us = usNow() - started

      await hashVerify(N1_SSH, `${ds.label}/${mode}`, dst, ds.kind, ds.hash)

      const row = {
          route: 'wan',
          dataset: id,
          bytes: ds.bytes,
          files: ds.files,
          mode,
          us,
          mbps: +((ds.bytes * 8) / Math.max(us, 1)).toFixed(2),
          note,
        },
        idx = row_li.findIndex((item) => item.route === 'wan' && item.dataset === id && item.mode === mode)
      if (idx >= 0) row_li[idx] = row
      else row_li.push(row)
      rowsSort(row_li)
      await rowsSave(row_li)

      log(`[${ds.label}] ${mode} 完成：${(us / 1000).toFixed(1)} ms, ${row.mbps.toFixed(2)} Mbps`)
      await remote(N1_SSH, 'rm -rf /tmp/s0bench/out').catch(() => {})
    }
  }
}

const runLocalC1Route = async (row_li) => {
  const local_ts = await detectLocalTs(),
    meta_raw = await datasetPrepareLocal(),
    meta = Object.fromEntries(
      meta_raw
        .split('\n')
        .filter(Boolean)
        .map((line) => line.split('=')),
    ),
    [xz_hash, tar_hash, tar_delta_hash, dir_hash] = await Promise.all([
      remoteTreeHash(C1_SSH, `${LDATA}/local.tar.xz`, 'file'),
      remoteTreeHash(C1_SSH, `${LDATA}/local.tar`, 'file'),
      remoteTreeHash(C1_SSH, `${LDATA}/local-delta.tar`, 'file'),
      remoteTreeHash(C1_SSH, `${LDATA}/local_dir`, 'dir'),
    ]),
    all = {
      xz: {
        label: 'local_c1/local.tar.xz',
        bytes: +meta.XZ,
        files: 1,
        src: `${LDATA}/local.tar.xz`,
        share_root: `${LBASE}/share_xz`,
        kind: 'file',
        hash: xz_hash,
      },
      tar: {
        label: 'local_c1/local.tar',
        bytes: +meta.TAR,
        files: 1,
        src: `${LDATA}/local.tar`,
        share_root: `${LBASE}/share_tar`,
        kind: 'file',
        hash: tar_hash,
      },
      tar_delta: {
        label: 'local_c1/local.tar (delta)',
        bytes: +meta.TAR_DELTA,
        files: 1,
        src: `${LBASE}/share_tar_delta/local.tar`,
        share_root: `${LBASE}/share_tar_delta`,
        kind: 'file',
        hash: tar_delta_hash,
      },
      dir: {
        label: 'local_c1/local_dir/',
        bytes: +meta.DIR,
        files: +meta.FILES,
        src: `${LDATA}/local_dir`,
        share_root: `${LDATA}/local_dir`,
        kind: 'dir',
        hash: dir_hash,
      },
    }

  log(`本机 Tailscale IP: ${local_ts} -> c1 (${C1_TS})`)

  for (const id of DATASET_LI) {
    const ds = all[id]
    if (!ds) continue
    for (const mode of MODE_LI) {
      const dst = `${LOUT}/${mode}/${basename(ds.src)}`,
        p2p_dst = ds.kind === 'file' ? `${LOUT}/${mode}` : dst,
        seed_local = `${LBASE}/s1/${id}`
      log(`[${ds.label}] ${mode} 开始`)
      const started = usNow()
      let us = 0, note = ''
      if (mode === 'sp') {
        const [sp_us, blocks, retx] = await spRunLocal(ds.src, dst, ds.kind, id)
        us = sp_us
        note = `blocks=${blocks} retx=${retx}`
      } else if (mode === 'p2p_1') {
        const [p2p_us, peers] = await p2pRunLocal(local_ts, ds.share_root, p2p_dst, seed_local, ds.kind, id, dst, false)
        us = p2p_us
        note = `peers=${peers}`
      } else if (mode === 'p2p') {
        const [p2p_us, peers] = await p2pRunLocal(local_ts, ds.share_root, p2p_dst, seed_local, ds.kind, id, dst, true)
        us = p2p_us
        note = `peers=${peers}`
      } else if (mode === 'rsync') {
        const [rsync_us, rsync_note] = await rsyncRunLocal(ds.src, dst, ds.kind, id)
        us = rsync_us
        note = rsync_note
      } else continue
      if (!us) us = usNow() - started

      await hashVerify(C1_SSH, `${ds.label}/${mode}`, dst, ds.kind, ds.hash)

      const row = {
          route: 'local_c1',
          dataset: id,
          bytes: ds.bytes,
          files: ds.files,
          mode,
          us,
          mbps: +((ds.bytes * 8) / Math.max(us, 1)).toFixed(2),
          note,
        },
        idx = row_li.findIndex((item) => item.route === 'local_c1' && item.dataset === id && item.mode === mode)
      if (idx >= 0) row_li[idx] = row
      else row_li.push(row)
      rowsSort(row_li)
      await rowsSave(row_li)

      log(`[${ds.label}] ${mode} 完成：${(us / 1000).toFixed(1)} ms, ${row.mbps.toFixed(2)} Mbps`)
      await remote(C1_SSH, `rm -rf ${LOUT}`).catch(() => {})
    }
  }
}

const syncCleanup = async () =>
  Promise.allSettled([
    localShell('pkill -9 -x sync0 2>/dev/null || true'),
    ...[C1_SSH, N1_SSH, S1_SSH].map((host) =>
      remote(host, 'pkill -9 -x sync0 2>/dev/null || true'),
    ),
  ])

const main = async () => {
  const row_li = await rowsRead()
  if (PRINT_ONLY) {
    rowsSort(row_li)
    tablePrint(row_li)
    return
  }

  await binBuild()
  await binDeploy()

  if (ROUTE_LI.includes('wan')) {
    await runWanRoute(row_li)
  }
  if (ROUTE_LI.includes('local_c1')) {
    await runLocalC1Route(row_li)
  }

  rowsSort(row_li)
  await rowsSave(row_li)
  tablePrint(row_li)
}

try {
  await main()
} catch (err) {
  warn(err instanceof Error ? err.message : String(err))
  process.exitCode = 1
} finally {
  await syncCleanup().catch(() => {})
}

process.on('SIGINT', async () => {
  await syncCleanup().catch(() => {})
  process.exit(130)
})
process.on('SIGTERM', async () => {
  await syncCleanup().catch(() => {})
  process.exit(143)
})
