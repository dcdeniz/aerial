import { existsSync, readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";

const packageRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const manifest = JSON.parse(readFileSync(resolve(packageRoot, "package.json"), "utf8"));
const binary = resolve(packageRoot, "vendor", "aerial.exe");

if (!existsSync(binary)) {
  console.error(`Missing ${binary}. Build Aerial and copy target/release/aerial.exe before packing.`);
  process.exit(1);
}

if (manifest.os?.join(",") !== "win32" || manifest.cpu?.join(",") !== "x64") {
  console.error("The current native package must be restricted to win32-x64.");
  process.exit(1);
}
