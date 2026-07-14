#!/usr/bin/env node
import { execFileSync, spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, readFileSync, renameSync, statSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = dirname(fileURLToPath(import.meta.url));

process.on("uncaughtException", (error) => {
  console.error(`Error: ${error instanceof Error ? error.message : String(error)}`);
  process.exit(1);
});

function parseArgs(argv) {
  const options = {
    repoRoot: resolve(scriptDir, ".."),
  };

  for (let index = 2; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === "--repo-root") {
      options.repoRoot = resolve(argv[++index] ?? "");
      continue;
    }
    throw new Error(`Unknown argument: ${arg}`);
  }

  return options;
}

function getHostTarget() {
  const osByPlatform = {
    win32: "windows",
    darwin: "macos",
    linux: "linux",
  };
  const os = osByPlatform[process.platform];
  if (!os) {
    throw new Error(`Unsupported release host OS: ${process.platform}`);
  }

  const archByProcess = {
    x64: "x64",
    arm64: "arm64",
    ia32: "ia32",
  };
  const arch = archByProcess[process.arch] ?? process.arch;

  return {
    os,
    arch,
    target: `${os}-${arch}`,
    executableSuffix: os === "windows" ? ".exe" : "",
  };
}

function run(command, args, options = {}) {
  const result = spawnSync(command, args, { stdio: "inherit", ...options });
  if (result.error) {
    if (result.error.code === "ENOENT") {
      throw new Error(`Required command not found: ${command}`);
    }
    throw result.error;
  }
  if (result.status !== 0) {
    throw new Error(`${command} ${args.join(" ")} failed with exit code ${result.status}.`);
  }
}

function output(command, args, cwd) {
  return execFileSync(command, args, { cwd, encoding: "utf8" }).trim();
}

function sha256File(path) {
  return createHash("sha256").update(readFileSync(path)).digest("hex").toUpperCase();
}

const { repoRoot } = parseArgs(process.argv);
const repo = resolve(repoRoot);
const manifestPath = join(repo, "src-tauri", "Cargo.toml");
const tauriConfigPath = join(repo, "src-tauri", "tauri.conf.json");
const tauriConfig = JSON.parse(readFileSync(tauriConfigPath, "utf8"));
const target = getHostTarget();

run("cargo", ["build", "--manifest-path", manifestPath, "--release", "--bin", "node-tide-cli"], {
  cwd: repo,
});

const binaryName = `node-tide-cli${target.executableSuffix}`;
const binaryPath = join(repo, "src-tauri", "target", "release", binaryName);
if (!existsSync(binaryPath)) {
  throw new Error(`CLI release build did not produce: ${binaryPath}`);
}

const commitSha = output("git", ["rev-parse", "HEAD"], repo);
const dirtyStatus = output("git", ["status", "--porcelain"], repo);
const file = statSync(binaryPath);
const metadata = {
  schemaVersion: 1,
  generatedAt: new Date().toISOString(),
  commitSha,
  dirtyWorktree: dirtyStatus.length > 0,
  packageVersion: tauriConfig.version,
  target: target.target,
  fileName: binaryName,
  bytes: file.size,
  sha256: sha256File(binaryPath),
};

const metadataPath = join(repo, "src-tauri", "target", "release", "node-tide-cli.build.json");
const temporaryPath = `${metadataPath}.tmp`;
writeFileSync(temporaryPath, `${JSON.stringify(metadata, null, 2)}\n`, "utf8");
renameSync(temporaryPath, metadataPath);

console.log(`CLI release binary: ${binaryPath}`);
console.log(`CLI build metadata: ${metadataPath}`);
