#!/usr/bin/env bun
// 跨平台 GitHub Actions 实测脚本（linux-x64 / linux-arm64 / macos / windows）：
// 在当前平台上实测 sync0 与 rsync 在 4 类数据集 × 3 阶段（首次全量传输 / 修改增量同步 / 零改动复传）的速度对比与 SHA-256 校验。
import { spawn, spawnSync } from "node:child_process";
import { createHash, randomBytes } from "node:crypto";
import {
  appendFileSync,
  cpSync,
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  rmSync,
  statSync,
  utimesSync,
  writeFileSync,
} from "node:fs";
import { join, posix } from "node:path";

const PLATFORM = process.env.GHA_PLATFORM || `${process.platform}-${process.arch}`;
const IS_WIN = process.platform === "win32";
const SYNC0_BIN =
  process.env.SYNC0_BIN ||
  (IS_WIN ? "target/release/sync0.exe" : "target/release/sync0");
const PORT_BASE = 29200;

function findRsync() {
  const candidates = IS_WIN
    ? ["rsync", "C:\\msys64\\usr\\bin\\rsync.exe"]
    : ["rsync"];
  for (const c of candidates) {
    const r = spawnSync(c, ["--version"], { stdio: "pipe" });
    if (r.status === 0) return c;
  }
  if (IS_WIN && existsSync("C:\\msys64\\usr\\bin\\pacman.exe")) {
    spawnSync("C:\\msys64\\usr\\bin\\pacman.exe", ["-S", "--noconfirm", "rsync"], {
      stdio: "inherit",
    });
    if (existsSync("C:\\msys64\\usr\\bin\\rsync.exe")) {
      return "C:\\msys64\\usr\\bin\\rsync.exe";
    }
  }
  return null;
}

const RSYNC_BIN = findRsync();

const ROOT = "bench_tmp";
const BASE_DIR = `${ROOT}/base`;
const MOD_DIR = `${ROOT}/mod`;
const OUT_S0 = `${ROOT}/out_s0`;
const OUT_RSYNC = `${ROOT}/out_rsync`;

rmSync(ROOT, { recursive: true, force: true });
mkdirSync(`${BASE_DIR}/many-files`, { recursive: true });
mkdirSync(MOD_DIR, { recursive: true });
mkdirSync(OUT_S0, { recursive: true });
mkdirSync(OUT_RSYNC, { recursive: true });

// 1. 生成 4 类基准数据集与增量修改数据集
// (a) git-cargo.tar: ~16 MiB 可压缩归档数据
{
  const chunks = [];
  const sample = readFileSync("Cargo.lock");
  while (Buffer.concat(chunks).length < 16 * 1024 * 1024) {
    chunks.push(sample);
  }
  const baseBuf = Buffer.concat(chunks).subarray(0, 16 * 1024 * 1024);
  writeFileSync(`${BASE_DIR}/git-cargo.tar`, baseBuf);
  const mid = 8 * 1024 * 1024;
  const modBuf = Buffer.concat([
    baseBuf.subarray(0, mid),
    randomBytes(65536),
    baseBuf.subarray(mid),
  ]);
  writeFileSync(`${MOD_DIR}/git-cargo.tar`, modBuf);
}

// (b) many-files: 500 个源码小文件 (~2 MiB)
{
  const seed = Buffer.concat(Array(2).fill(readFileSync("crate/s0_udp/src/lib.rs"))).subarray(0, 4096);
  for (let i = 0; i < 500; i++) {
    const dir = `${BASE_DIR}/many-files/d${Math.floor(i / 25)}`;
    mkdirSync(dir, { recursive: true });
    const body = Buffer.concat([
      Buffer.from(`// file ${i}\n`),
      seed,
      Buffer.from(`\npub const ID_${i}: usize = ${i};\n`),
    ]);
    writeFileSync(`${dir}/f_${i}.rs`, body);
  }
  cpSync(`${BASE_DIR}/many-files`, `${MOD_DIR}/many-files`, { recursive: true });
  for (let i = 0; i < 10; i++) {
    writeFileSync(
      `${MOD_DIR}/many-files/d0/f_${i}.rs`,
      Buffer.concat([randomBytes(2048), Buffer.from(`\n// modified ${i}\n`)]),
    );
  }
  for (let i = 0; i < 5; i++) {
    writeFileSync(`${MOD_DIR}/many-files/d0/new_${i}.rs`, randomBytes(4096));
  }
}

// (c) video.mp4: 16 MiB 不可压缩高熵流 + 64 KiB 局部覆写
{
  const baseBuf = randomBytes(16 * 1024 * 1024);
  writeFileSync(`${BASE_DIR}/video.mp4`, baseBuf);
  const modBuf = Buffer.from(baseBuf);
  randomBytes(65536).copy(modBuf, 6 * 1024 * 1024);
  writeFileSync(`${MOD_DIR}/video.mp4`, modBuf);
}

// (d) model.onnx: 16 MiB 不可压缩模型参数 + 64 KiB 尾部追加与局部微调
{
  const baseBuf = randomBytes(16 * 1024 * 1024);
  writeFileSync(`${BASE_DIR}/model.onnx`, baseBuf);
  const modBuf = Buffer.from(baseBuf);
  randomBytes(32768).copy(modBuf, 4 * 1024 * 1024);
  writeFileSync(
    `${MOD_DIR}/model.onnx`,
    Buffer.concat([modBuf, randomBytes(32768)]),
  );
}

function hashTree(targetPath) {
  const st = statSync(targetPath);
  if (!st.isDirectory()) {
    const h = createHash("sha256");
    h.update(readFileSync(targetPath));
    return { size: st.size, hash: h.digest("hex") };
  }
  const files = [];
  function walk(cur, relBase) {
    for (const e of readdirSync(cur, { withFileTypes: true })) {
      if (e.name.endsWith(".s0tmp") || e.name.endsWith(".s0part")) continue;
      const full = join(cur, e.name);
      const rel = relBase ? posix.join(relBase, e.name) : e.name;
      if (e.isDirectory()) walk(full, rel);
      else if (e.isFile()) files.push({ full, rel });
    }
  }
  walk(targetPath, "");
  files.sort((a, b) => (a.rel < b.rel ? -1 : a.rel > b.rel ? 1 : 0));
  const root = createHash("sha256");
  let size = 0;
  for (const f of files) {
    const buf = readFileSync(f.full);
    size += buf.length;
    const fh = createHash("sha256").update(buf).digest("hex");
    root.update(`${fh}  ./${f.rel}\n`);
  }
  return { size, hash: root.digest("hex") };
}

function touchFuture(targetPath) {
  const future = new Date(Date.now() + 120_000);
  const st = statSync(targetPath);
  if (!st.isDirectory()) {
    utimesSync(targetPath, future, future);
    return;
  }
  function walk(cur) {
    for (const e of readdirSync(cur, { withFileTypes: true })) {
      const full = join(cur, e.name);
      if (e.isDirectory()) walk(full);
      else if (e.isFile()) utimesSync(full, future, future);
    }
  }
  walk(targetPath);
}

let nextPort = PORT_BASE;

function collectProc(proc) {
  return new Promise((resolve) => {
    const out = [];
    const err = [];
    proc.stdout?.on("data", (c) => out.push(c));
    proc.stderr?.on("data", (c) => err.push(c));
    proc.on("close", (code) => {
      resolve({
        code: code ?? 1,
        stdout: Buffer.concat(out).toString("utf8"),
        stderr: Buffer.concat(err).toString("utf8"),
      });
    });
  });
}

async function runSync0Phase(srcPath, dstPath) {
  const port = nextPort++;
  const addr = `127.0.0.1:${port}`;
  const recvProc = spawn(SYNC0_BIN, ["bench-recv", "--quiet", addr, dstPath], {
    stdio: ["ignore", "pipe", "pipe"],
  });
  const recvDone = collectProc(recvProc);
  await new Promise((r) => setTimeout(r, 80));
  const t0 = performance.now();
  const sendProc = spawn(
    SYNC0_BIN,
    ["bench-send", "--quiet", srcPath, addr, "--rate", "0"],
    { stdio: ["ignore", "pipe", "pipe"] },
  );
  const { code: sendCode, stdout: sendOut, stderr: sendErr } = await collectProc(sendProc);
  const wallMs = Math.max(1, Math.round(performance.now() - t0));
  await recvDone;
  if (sendCode !== 0) {
    throw new Error(`sync0 bench-send failed (${sendCode}): ${sendErr}`);
  }
  const line = sendOut.split("\n").find((l) => l.trim().startsWith("{")) || "{}";
  const stats = JSON.parse(line);
  const us = stats.us || wallMs * 1000;
  return { ms: +(us / 1000).toFixed(1), blocks: stats.blocks ?? 0 };
}

function runRsyncPhase(srcPath, dstPath, isDir, incremental = false) {
  if (!RSYNC_BIN) return null;
  const srcArg = isDir ? `${srcPath.replaceAll("\\", "/")}/` : srcPath.replaceAll("\\", "/");
  const dstArg = isDir ? `${dstPath.replaceAll("\\", "/")}/` : dstPath.replaceAll("\\", "/");
  if (isDir) mkdirSync(dstPath, { recursive: true });
  const env = IS_WIN
    ? { ...process.env, PATH: `C:\\msys64\\usr\\bin;${process.env.PATH || ""}` }
    : process.env;
  const rsh = "sh -c 'shift; exec \"$@\"' --";
  const remoteDst = `localhost:${dstArg}`;
  const args = incremental
    ? ["-az", "--inplace", "--no-whole-file", "-e", rsh, srcArg, remoteDst]
    : ["-az", "-e", rsh, srcArg, remoteDst];
  const t0 = performance.now();
  let r = spawnSync(RSYNC_BIN, args, { stdio: "pipe", env });
  let ms = Math.max(1, +(performance.now() - t0).toFixed(1));
  if (r.status !== 0) {
    const fallbackArgs = ["-az", "--no-whole-file", srcArg, dstArg];
    const t1 = performance.now();
    r = spawnSync(RSYNC_BIN, fallbackArgs, { stdio: "pipe", env });
    ms = Math.max(1, +(performance.now() - t1).toFixed(1));
    if (r.status !== 0) return null;
  }
  return { ms };
}

const ITEMS = [
  { name: "git-cargo.tar", dir: false },
  { name: "many-files", dir: true },
  { name: "video.mp4", dir: false },
  { name: "model.onnx", dir: false },
];

const rows = [];

for (const item of ITEMS) {
  const baseSrc = `${BASE_DIR}/${item.name}`;
  const modSrc = `${MOD_DIR}/${item.name}`;
  const s0Dst = `${OUT_S0}/${item.name}`;
  const rsDst = `${OUT_RSYNC}/${item.name}`;

  const baseInfo = hashTree(baseSrc);
  const modInfo = hashTree(modSrc);
  touchFuture(modSrc);

  // 1. 首次全量传输
  rmSync(s0Dst, { recursive: true, force: true });
  const s0Cold = await runSync0Phase(baseSrc, s0Dst);
  const s0ColdCheck = hashTree(s0Dst);
  if (s0ColdCheck.hash !== baseInfo.hash) {
    throw new Error(`SHA-256 mismatch on Cold sync0 for ${item.name}`);
  }

  rmSync(rsDst, { recursive: true, force: true });
  const rsCold = runRsyncPhase(baseSrc, rsDst, item.dir, false);

  // 2. 修改增量同步
  const s0Warm = await runSync0Phase(modSrc, s0Dst);
  const s0WarmCheck = hashTree(s0Dst);
  if (s0WarmCheck.hash !== modInfo.hash) {
    throw new Error(`SHA-256 mismatch on Warm CDC sync0 for ${item.name}`);
  }

  const rsWarm = runRsyncPhase(modSrc, rsDst, item.dir, true);

  // 3. 零改动复传
  const s0Hot = await runSync0Phase(modSrc, s0Dst);
  const rsHot = runRsyncPhase(modSrc, rsDst, item.dir, true);

  const mbps = (bytes, ms) => +(((bytes * 8) / 1e6 / (ms / 1000)).toFixed(1));

  rows.push({
    dataset: item.name,
    sizeMb: +(modInfo.size / 1e6).toFixed(2),
    s0ColdMs: s0Cold.ms,
    s0ColdMbps: mbps(baseInfo.size, s0Cold.ms),
    rsColdMs: rsCold ? rsCold.ms : null,
    rsColdMbps: rsCold ? mbps(baseInfo.size, rsCold.ms) : null,
    coldSpeedup: rsCold ? +(rsCold.ms / s0Cold.ms).toFixed(2) : null,
    s0WarmMs: s0Warm.ms,
    s0WarmMbps: mbps(modInfo.size, s0Warm.ms),
    rsWarmMs: rsWarm ? rsWarm.ms : null,
    rsWarmMbps: rsWarm ? mbps(modInfo.size, rsWarm.ms) : null,
    warmSpeedup: rsWarm ? +(rsWarm.ms / s0Warm.ms).toFixed(2) : null,
    s0HotMs: s0Hot.ms,
    rsHotMs: rsHot ? rsHot.ms : null,
    blocks: `${s0Cold.blocks}->${s0Warm.blocks}`,
  });
}

rmSync(ROOT, { recursive: true, force: true });

const mdLines = [
  `### 平台实测: \`${PLATFORM}\` (\`sync0\` vs \`rsync\`)`,
  "",
  "| 数据集 | 大小 (MB) | `sync0` 首次全量 | `rsync` 首次全量 | 全量加速比 | `sync0` 修改增量 | `rsync` 修改增量 | 增量加速比 | `sync0` 零改动 | 块缩减 |",
  "| :--- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | :---: |",
  ...rows.map((r) => {
    const rc = r.rsColdMs ? `${r.rsColdMs}ms (${r.rsColdMbps}M)` : "—";
    const cs = r.coldSpeedup ? `**${r.coldSpeedup}×**` : "—";
    const rw = r.rsWarmMs ? `${r.rsWarmMs}ms (${r.rsWarmMbps}M)` : "—";
    const ws = r.warmSpeedup ? `**${r.warmSpeedup}×**` : "—";
    return `| \`${r.dataset}\` | ${r.sizeMb} | **${r.s0ColdMs}ms (${r.s0ColdMbps}M)** | ${rc} | ${cs} | **${r.s0WarmMs}ms (${r.s0WarmMbps}M)** | ${rw} | ${ws} | **${r.s0HotMs}ms** | \`${r.blocks}\` |`;
  }),
  "",
];

const md = mdLines.join("\n");
console.log(md);

if (process.env.GITHUB_STEP_SUMMARY) {
  appendFileSync(process.env.GITHUB_STEP_SUMMARY, md + "\n");
}

mkdirSync("bench", { recursive: true });
writeFileSync(
  `bench/gha-${PLATFORM}.json`,
  JSON.stringify({ platform: PLATFORM, rows }, null, 2) + "\n",
);
