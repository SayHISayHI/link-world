#!/usr/bin/env node
import { execFileSync, spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  copyFileSync,
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { join, resolve, dirname } from "node:path";
import { tmpdir } from "node:os";
import { fileURLToPath } from "node:url";

const scriptDir = dirname(fileURLToPath(import.meta.url));

process.on("uncaughtException", (error) => {
  console.error(`Error: ${error instanceof Error ? error.message : String(error)}`);
  process.exit(1);
});

function parseArgs(argv) {
  const options = {
    repoRoot: resolve(scriptDir, ".."),
    outputDirectory: "",
    readinessReport: "",
    allowDirty: false,
  };

  for (let index = 2; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === "--repo-root") {
      options.repoRoot = resolve(argv[++index] ?? "");
      continue;
    }
    if (arg === "--output-directory") {
      options.outputDirectory = argv[++index] ?? "";
      continue;
    }
    if (arg === "--readiness-report") {
      options.readinessReport = argv[++index] ?? "";
      continue;
    }
    if (arg === "--allow-dirty") {
      options.allowDirty = true;
      continue;
    }
    throw new Error(`Unknown argument: ${arg}`);
  }

  return options;
}

function readJson(path) {
  return JSON.parse(readFileSync(path, "utf8"));
}

function getHostTarget() {
  if (process.platform !== "darwin") {
    throw new Error("macOS release packaging must run on a macOS host after npm run tauri:build:macos and npm run build:cli.");
  }

  const archByProcess = {
    x64: "x64",
    arm64: "arm64",
  };
  const arch = archByProcess[process.arch] ?? process.arch;

  return {
    os: "macos",
    arch,
    target: `macos-${arch}`,
  };
}

function commandAvailable(command) {
  const result = spawnSync("command", ["-v", command], { shell: true, stdio: "ignore" });
  return !result.error && result.status === 0;
}

function git(repo, args) {
  return execFileSync("git", ["-C", repo, ...args], { encoding: "utf8" }).trim();
}

function sha256File(path) {
  return createHash("sha256").update(readFileSync(path)).digest("hex").toUpperCase();
}

function bytes(path) {
  return statSync(path).size;
}

function listEntries(dir, predicate) {
  if (!existsSync(dir)) {
    return [];
  }
  return readdirSync(dir, { withFileTypes: true })
    .filter(predicate)
    .map((entry) => join(dir, entry.name));
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

function getCodeSignatureStatus(path) {
  if (!commandAvailable("codesign")) {
    return "signature_unverified_codesign_unavailable";
  }
  const result = spawnSync("codesign", ["--verify", "--deep", "--strict", "--verbose=2", path], {
    stdio: "ignore",
  });
  return result.status === 0 ? "signed_valid" : "unsigned_alpha";
}

function getNotarizationStatus(path) {
  if (!commandAvailable("xcrun")) {
    return "notarization_unverified_xcrun_unavailable";
  }
  const result = spawnSync("xcrun", ["stapler", "validate", path], { stdio: "ignore" });
  return result.status === 0 ? "notarized_stapled" : "notarization_unstapled_or_unverified";
}

const options = parseArgs(process.argv);
const repo = resolve(options.repoRoot);
const target = getHostTarget();
const tauriConfig = readJson(join(repo, "src-tauri", "tauri.conf.json"));
const migrationsDir = join(repo, "src-tauri", "migrations");
const migrationFiles = listEntries(
  migrationsDir,
  (entry) => entry.isFile() && entry.name.endsWith(".sql"),
).sort();
const schemaVersion =
  migrationFiles.length === 0 ? 0 : Number.parseInt(migrationFiles.at(-1).split("/").pop().split("_")[0], 10);
const commitSha = git(repo, ["rev-parse", "HEAD"]);
const shortCommit = git(repo, ["rev-parse", "--short=8", "HEAD"]);
const branch = git(repo, ["rev-parse", "--abbrev-ref", "HEAD"]);
const dirtyStatus = git(repo, ["status", "--porcelain"]);
const isDirty = dirtyStatus.length > 0;

if (isDirty && !options.allowDirty) {
  throw new Error("Release packaging requires a clean worktree. Commit or explicitly use --allow-dirty for a non-release rehearsal.");
}

const bundleRoot = join(repo, "src-tauri", "target", "release", "bundle");
const appCandidates = listEntries(
  join(bundleRoot, "macos"),
  (entry) => entry.isDirectory() && entry.name.endsWith(".app"),
);
const dmgCandidates = listEntries(
  join(bundleRoot, "dmg"),
  (entry) => entry.isFile() && entry.name.endsWith(".dmg"),
);
const cliCandidate = join(repo, "src-tauri", "target", "release", "node-tide-cli");
const cliBuildMetadataPath = join(repo, "src-tauri", "target", "release", "node-tide-cli.build.json");

if (appCandidates.length !== 1 || dmgCandidates.length !== 1) {
  throw new Error(`Expected exactly one .app bundle and one .dmg artifact; found ${appCandidates.length} app bundles and ${dmgCandidates.length} dmg files.`);
}

if (!dmgCandidates[0].includes(tauriConfig.version)) {
  throw new Error(`Artifact version does not match Tauri version ${tauriConfig.version}: ${dmgCandidates[0]}`);
}

if (!existsSync(cliCandidate)) {
  throw new Error("Missing macOS CLI artifact. Run npm run build:cli on macOS after the Tauri build.");
}

if (!existsSync(cliBuildMetadataPath)) {
  throw new Error("Missing CLI build metadata. Run npm run build:cli after the Tauri build.");
}

const cliBuildMetadata = readJson(cliBuildMetadataPath);
const cliHash = sha256File(cliCandidate);
if (cliBuildMetadata.commitSha !== commitSha) {
  throw new Error(`CLI build metadata commit does not match HEAD: ${cliBuildMetadata.commitSha} != ${commitSha}`);
}
if (cliBuildMetadata.packageVersion !== tauriConfig.version) {
  throw new Error(`CLI build metadata version does not match Tauri version ${tauriConfig.version}.`);
}
if (cliBuildMetadata.target !== target.target) {
  throw new Error(`CLI build metadata target does not match this host: ${cliBuildMetadata.target} != ${target.target}`);
}
if (cliBuildMetadata.sha256 !== cliHash) {
  throw new Error("CLI binary no longer matches its build metadata. Run npm run build:cli after the Tauri build.");
}
if (Number(cliBuildMetadata.bytes) !== bytes(cliCandidate)) {
  throw new Error("CLI binary size no longer matches its build metadata.");
}

const stamp = new Date()
  .toISOString()
  .replace(/[-:]/g, "")
  .replace(/\..+$/, "")
  .replace("T", "-");
const outputDirectory =
  options.outputDirectory || join(tmpdir(), `node-tide-macos-${tauriConfig.version}-${shortCommit}-${stamp}`);
const outputFullPath = resolve(outputDirectory);
if (existsSync(outputFullPath)) {
  throw new Error(`Output directory already exists: ${outputFullPath}`);
}
mkdirSync(outputFullPath, { recursive: false });

const appZipName = `node-tide-${tauriConfig.version}-${target.target}-${shortCommit}.app.zip`;
const appZipPath = join(outputFullPath, appZipName);
run("ditto", ["-c", "-k", "--sequesterRsrc", "--keepParent", appCandidates[0], appZipPath]);

const sourceArtifacts = [
  {
    packageType: "app",
    path: appZipPath,
    signaturePath: appCandidates[0],
    fileName: appZipName,
  },
  {
    packageType: "dmg",
    path: dmgCandidates[0],
    signaturePath: dmgCandidates[0],
    fileName: `node-tide-${tauriConfig.version}-${target.target}-${shortCommit}.dmg`,
  },
  {
    packageType: "cli",
    path: cliCandidate,
    signaturePath: cliCandidate,
    fileName: "node-tide-cli",
  },
];

const files = sourceArtifacts.map((artifact) => {
  const destination = join(outputFullPath, artifact.fileName);
  if (resolve(artifact.path) !== resolve(destination)) {
    copyFileSync(artifact.path, destination);
  }
  return {
    fileName: artifact.fileName,
    packageType: artifact.packageType,
    bytes: bytes(destination),
    sha256: sha256File(destination),
    signatureStatus: getCodeSignatureStatus(artifact.signaturePath),
    notarizationStatus:
      artifact.packageType === "app" || artifact.packageType === "dmg"
        ? getNotarizationStatus(artifact.signaturePath)
        : "not_applicable",
  };
});

let readiness = null;
if (options.readinessReport) {
  const reportPath = resolve(options.readinessReport);
  const report = readJson(reportPath);
  if (report.status !== "passed") {
    throw new Error("Readiness report is not passed.");
  }
  if (report.app.commitSha !== commitSha) {
    throw new Error(`Readiness report commit does not match HEAD: ${report.app.commitSha} != ${commitSha}`);
  }
  if (report.app.dirtyWorktree && !options.allowDirty) {
    throw new Error("Readiness report was generated from a dirty worktree.");
  }

  const reportName = "alpha-readiness.json";
  const reportDestination = join(outputFullPath, reportName);
  copyFileSync(reportPath, reportDestination);
  readiness = {
    fileName: reportName,
    status: report.status,
    sha256: sha256File(reportDestination),
  };
}

const signatureStatuses = [...new Set(files.map((file) => file.signatureStatus))];
const notarizationStatuses = [
  ...new Set(files.filter((file) => file.notarizationStatus !== "not_applicable").map((file) => file.notarizationStatus)),
];
const manifest = {
  schemaVersion: 1,
  generatedAt: new Date().toISOString(),
  channel: "alpha",
  productName: tauriConfig.productName,
  packageVersion: tauriConfig.version,
  schemaMigrationVersion: schemaVersion,
  commitSha,
  branch,
  dirtyWorktree: isDirty,
  target: { os: "macos", arch: target.arch },
  signatureStatus: signatureStatuses.length === 1 ? signatureStatuses[0] : "mixed",
  notarizationStatus:
    notarizationStatuses.length === 1
      ? notarizationStatuses[0]
      : notarizationStatuses.length === 0
        ? "not_applicable"
        : "mixed",
  files,
  cliBuild: {
    metadataSchemaVersion: cliBuildMetadata.schemaVersion,
    sha256: cliBuildMetadata.sha256,
    bytes: Number(cliBuildMetadata.bytes),
    dirtyWorktree: Boolean(cliBuildMetadata.dirtyWorktree),
  },
  readinessReport: readiness,
  limitations: [
    "Checksums prove artifact integrity, not publisher identity.",
    "Public macOS distribution requires Developer ID signing and Apple notarization.",
    "Unsigned or unstapled Alpha artifacts require an out-of-band trusted checksum and explicit tester instructions.",
    "This manifest does not replace the macOS install, Gatekeeper, Keychain, non-ASCII profile, proxy/offline, upgrade, uninstall, Intel/Apple Silicon, or user-feedback matrices.",
  ],
};

const manifestPath = join(outputFullPath, "release-manifest.json");
writeFileSync(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`, "utf8");

const checksumLines = files
  .slice()
  .sort((left, right) => left.fileName.localeCompare(right.fileName))
  .map((file) => `${file.sha256}  ${file.fileName}`);
if (readiness) {
  checksumLines.push(`${readiness.sha256}  ${readiness.fileName}`);
}
checksumLines.push(`${sha256File(manifestPath)}  release-manifest.json`);
writeFileSync(join(outputFullPath, "SHA256SUMS.txt"), `${checksumLines.join("\n")}\n`, "utf8");

console.log(`macOS release package: ${outputFullPath}`);
console.log(`Signature status: ${manifest.signatureStatus}`);
console.log(`Notarization status: ${manifest.notarizationStatus}`);
