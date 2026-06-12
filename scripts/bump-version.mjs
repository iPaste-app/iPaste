import { readFile, writeFile } from "node:fs/promises";
import { createInterface } from "node:readline/promises";
import { stdin as input, stdout as output } from "node:process";
import { execFile } from "node:child_process";
import { promisify } from "node:util";

const execFileAsync = promisify(execFile);

const files = {
  packageJson: "package.json",
  packageLock: "package-lock.json",
  tauriConfig: "src-tauri/tauri.conf.json",
  cargoToml: "src-tauri/Cargo.toml",
};

const versionPattern =
  /^v?(\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?)$/;

function normalizeVersion(value) {
  const trimmed = value.trim();
  const match = trimmed.match(versionPattern);
  return match?.[1] ?? null;
}

async function readJson(path) {
  return JSON.parse(await readFile(path, "utf8"));
}

async function writeJson(path, data) {
  await writeFile(path, `${JSON.stringify(data, null, 2)}\n`);
}

async function updatePackageLock(version) {
  const packageLock = await readJson(files.packageLock);
  packageLock.version = version;

  if (packageLock.packages?.[""]) {
    packageLock.packages[""].version = version;
  }

  await writeJson(files.packageLock, packageLock);
}

async function updateCargoToml(version) {
  const cargoToml = await readFile(files.cargoToml, "utf8");
  const updated = cargoToml.replace(
    /^version\s*=\s*"[^"]+"/m,
    `version = "${version}"`,
  );

  if (updated === cargoToml) {
    throw new Error(`未能在 ${files.cargoToml} 中找到 package version 字段`);
  }

  await writeFile(files.cargoToml, updated);
}

async function getCurrentBranch() {
  try {
    const { stdout } = await execFileAsync("git", ["branch", "--show-current"]);
    return stdout.trim() || "main";
  } catch {
    return "main";
  }
}

async function main() {
  const packageJson = await readJson(files.packageJson);
  const currentVersion = packageJson.version;

  const rl = createInterface({ input, output });
  const answer = await rl.question(
    `请输入新版本号（当前 ${currentVersion}，例如 0.1.1）：`,
  );
  rl.close();

  const nextVersion = normalizeVersion(answer);

  if (!nextVersion) {
    console.log("未输入有效版本号，已取消。");
    return;
  }

  if (nextVersion === currentVersion) {
    console.log(`版本号仍为 ${currentVersion}，无需修改。`);
    return;
  }

  packageJson.version = nextVersion;
  await writeJson(files.packageJson, packageJson);

  await updatePackageLock(nextVersion);

  const tauriConfig = await readJson(files.tauriConfig);
  tauriConfig.version = nextVersion;
  await writeJson(files.tauriConfig, tauriConfig);

  await updateCargoToml(nextVersion);

  const currentBranch = await getCurrentBranch();

  console.log("");
  console.log(`版本号已更新：${currentVersion} -> ${nextVersion}`);
  console.log("");
  console.log("接下来可以这样提交并触发 release actions：");
  console.log("");
  console.log("  git add package.json package-lock.json src-tauri/tauri.conf.json src-tauri/Cargo.toml");
  console.log(`  git commit -m "chore: release v${nextVersion}"`);
  console.log(`  git tag v${nextVersion}`);
  console.log(`  git push origin ${currentBranch}`);
  console.log(`  git push origin v${nextVersion}`);
}

main().catch((error) => {
  console.error(error.message);
  process.exitCode = 1;
});
