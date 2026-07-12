#!/usr/bin/env node

import { existsSync } from "node:fs";
import { spawn } from "node:child_process";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";

const packageRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const binary = process.env.AERIAL_BINARY_PATH || resolve(packageRoot, "vendor", "aerial.exe");

if (process.platform !== "win32" || process.arch !== "x64") {
  console.error(`aerial-local supports win32-x64; detected ${process.platform}-${process.arch}`);
  process.exit(1);
}

if (!existsSync(binary)) {
  console.error(`aerial-local could not find its native binary at ${binary}`);
  console.error("Reinstall aerial-local, or set AERIAL_BINARY_PATH for local development.");
  process.exit(1);
}

const child = spawn(binary, process.argv.slice(2), {
  stdio: "inherit",
  windowsHide: false
});

for (const signal of ["SIGINT", "SIGTERM"]) {
  process.on(signal, () => {
    if (!child.killed) child.kill(signal);
  });
}

child.on("error", (error) => {
  console.error(`aerial-local failed to start ${binary}: ${error.message}`);
  process.exitCode = 1;
});

child.on("exit", (code, signal) => {
  process.exitCode = code ?? (signal ? 1 : 0);
});
