import { mkdirSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import { spawn, spawnSync } from "node:child_process";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const packageRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const launcher = process.env.AERIAL_TEST_LAUNCHER || resolve(packageRoot, "bin", "aerial.js");
const worker = resolve(packageRoot, "test", "worker.mjs");
const expectedVersion = JSON.parse(readFileSync(resolve(packageRoot, "package.json"), "utf8")).version;
const tempRoot = process.env.AERIAL_TEST_TMPDIR || tmpdir();
mkdirSync(tempRoot, { recursive: true });
const temp = process.env.AERIAL_TEST_TMPDIR
  ? join(tempRoot, `run-${process.pid}`)
  : mkdtempSync(join(tempRoot, "aerial-npm-"));
mkdirSync(temp, { recursive: true });
const dataDir = temp;
const socket = join(dataDir, "aerial.sock");
const workerOutput = join(temp, "worker-output.txt");
const command = [process.execPath, launcher];
let daemon;
let daemonStderr = "";

function run(args, options = {}) {
  const result = spawnSync(command[0], [...command.slice(1), ...args], {
    encoding: "utf8",
    env: process.env,
    ...options
  });
  if (result.status !== 0) {
    throw new Error(`aerial ${args.join(" ")} failed:\n${result.stderr || result.stdout}`);
  }
  return result.stdout.trim();
}

function delay(milliseconds) {
  return new Promise((resolveDelay) => setTimeout(resolveDelay, milliseconds));
}

async function waitFor(predicate, message, timeout = 10000) {
  const deadline = Date.now() + timeout;
  while (Date.now() < deadline) {
    if (predicate()) return;
    await delay(50);
  }
  throw new Error(message);
}

function waitForExit(child, timeout = 10000) {
  return new Promise((resolveExit, reject) => {
    const timer = setTimeout(() => reject(new Error("child process did not exit")), timeout);
    child.once("error", reject);
    child.once("exit", (code) => {
      clearTimeout(timer);
      code === 0 ? resolveExit() : reject(new Error(`child exited with status ${code}`));
    });
  });
}

async function mcpInitialize() {
  const child = spawn(command[0], [...command.slice(1), "mcp", "--socket", socket], {
    env: process.env,
    stdio: ["pipe", "pipe", "pipe"]
  });
  let stdout = "";
  let stderr = "";
  child.stdout.setEncoding("utf8").on("data", (chunk) => { stdout += chunk; });
  child.stderr.setEncoding("utf8").on("data", (chunk) => { stderr += chunk; });
  child.stdin.end('{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05"}}\n');
  await waitForExit(child);
  if (stderr) throw new Error(`MCP wrote to stderr: ${stderr}`);
  const response = JSON.parse(stdout.trim());
  if (response.result?.serverInfo?.name !== "aerial") {
    throw new Error(`unexpected MCP response: ${stdout}`);
  }
}

try {
  const version = run(["--version"]);
  if (!version.includes(expectedVersion)) throw new Error(`unexpected version: ${version}`);

  daemon = spawn(command[0], [...command.slice(1), "serve", "--data-dir", dataDir], {
    env: process.env,
    stdio: ["ignore", "pipe", "pipe"]
  });
  daemon.stderr.setEncoding("utf8").on("data", (chunk) => { daemonStderr += chunk; });
  await waitFor(() => {
    if (daemon.exitCode !== null) {
      throw new Error(`daemon exited with status ${daemon.exitCode}: ${daemonStderr}`);
    }
    try {
      run(["history", "--socket", socket, "--limit", "1", "--json"]);
      return true;
    } catch {
      return false;
    }
  }, `daemon did not accept connections at ${socket}: ${daemonStderr}`);

  const exchange = JSON.parse(run([
    "exchange", "--socket", socket, "--from", "engineer", "--to", "researcher",
    "--body", "npm package smoke test", "--json"
  ]));
  if (exchange.pending.length !== 1) {
    throw new Error("exchange did not create one pending message");
  }

  const drained = JSON.parse(run(["drain", "--socket", socket, "researcher", "--json"]));
  if (drained.acked.length !== 1) throw new Error("drain did not ack the pending message");

  await mcpInitialize();

  const supervisor = spawn(command[0], [
    ...command.slice(1), "agent", "exec", "--socket", socket, "--once", "worker", "--",
    process.execPath, worker
  ], {
    env: { ...process.env, AERIAL_TEST_OUTPUT: workerOutput },
    stdio: ["ignore", "pipe", "pipe"]
  });
  await delay(300);
  const sent = JSON.parse(run([
    "tell", "--socket", socket, "--from", "engineer", "--to", "worker",
    "--body", "run through npm"
  ]));
  await waitForExit(supervisor);
  const captured = readFileSync(workerOutput, "utf8");
  if (captured !== `${sent.envelope.id}|run through npm`) {
    throw new Error(`supervisor captured unexpected data: ${captured}`);
  }
  const status = JSON.parse(run(["status", "--socket", socket, "worker", "--json"]));
  if (status.pending.length !== 0) throw new Error("supervisor did not ack its message");

  console.log("aerial-local npm smoke test passed");
} finally {
  if (daemon && daemon.exitCode === null) daemon.kill("SIGTERM");
  await delay(200);
  rmSync(temp, { recursive: true, force: true });
}
