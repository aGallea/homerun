import { chmodSync, copyFileSync, mkdirSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(scriptDir, "../../..");
const isWindows = process.platform === "win32";
const executableSuffix = isWindows ? ".exe" : "";

function run(command, args, options = {}) {
  const result = spawnSync(command, args, {
    cwd: repoRoot,
    encoding: "utf8",
    stdio: options.capture ? ["ignore", "pipe", "pipe"] : "inherit",
  });

  if (result.error) {
    throw new Error(`Failed to start ${command}: ${result.error.message}`);
  }
  if (result.status !== 0) {
    const stderr = result.stderr?.trim();
    const detail = stderr ? `: ${stderr}` : "";
    throw new Error(`${command} exited with status ${result.status}${detail}`);
  }
  return result.stdout ?? "";
}

const rustcOutput = run(isWindows ? "rustc.exe" : "rustc", ["-vV"], {
  capture: true,
});
const hostLine = rustcOutput.split(/\r?\n/).find((line) => line.startsWith("host: "));
if (!hostLine) {
  throw new Error("Unable to determine the Rust host target from `rustc -vV`");
}
const hostTarget = hostLine.slice("host: ".length).trim();
if (!hostTarget) {
  throw new Error("Rust host target is empty");
}

run(isWindows ? "cargo.exe" : "cargo", ["build", "-p", "homerund"]);

const source = join(repoRoot, "target", "debug", `homerund${executableSuffix}`);
const destinationDir = join(repoRoot, "apps", "desktop", "src-tauri", "binaries");
const destination = join(destinationDir, `homerund-${hostTarget}${executableSuffix}`);
mkdirSync(destinationDir, { recursive: true });
copyFileSync(source, destination);
if (!isWindows) {
  chmodSync(destination, 0o755);
}

console.log(`Prepared HomeRun sidecar: ${destination}`);
