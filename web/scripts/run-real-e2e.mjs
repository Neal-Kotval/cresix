import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { spawn } from "node:child_process";
import { createServer } from "node:net";

const webRoot = resolve(import.meta.dirname, "..");
const repoRoot = resolve(webRoot, "..");
const token = process.env.C6_BOOTSTRAP_TOKEN ?? "c6-e2e-bootstrap-token-32-characters-minimum";

function run(command, args, options = {}) {
  return new Promise((resolvePromise, reject) => {
    const child = spawn(command, args, { stdio: "inherit", ...options });
    child.on("error", reject);
    child.on("exit", (code) => code === 0 ? resolvePromise() : reject(new Error(`${command} exited ${code}`)));
  });
}

async function testProject(project, port) {
  const dataDir = await mkdtemp(join(tmpdir(), `c6-web-e2e-${project}-`));
  const origin = `http://127.0.0.1:${port}`;
  let server;
  try {
    server = spawn(resolve(repoRoot, "target/debug/c6-server"), [], { cwd: repoRoot, stdio: ["ignore", "inherit", "inherit"], env: { ...process.env, C6_DATA_DIR: dataDir, C6_PORT: String(port), C6_PUBLIC_BASE_URL: origin, C6_BOOTSTRAP_TOKEN: token, C6_WEB_DIST: resolve(webRoot, "dist") } });
    let serverExit;
    server.once("exit", (code, signal) => { serverExit = new Error(`C6 server exited before the test completed (${signal ?? code})`); });
    for (let attempt = 0; attempt < 100; attempt += 1) {
      if (serverExit) throw serverExit;
      try { const response = await fetch(`${origin}/healthz`); if (response.ok) break; } catch { /* startup */ }
      if (attempt === 99) throw new Error("C6 server did not become healthy");
      await new Promise((resolvePromise) => setTimeout(resolvePromise, 100));
    }
    await run("npx", ["playwright", "test", "e2e/real-backend.spec.ts", `--project=${project}`], { cwd: webRoot, env: { ...process.env, C6_REAL_BACKEND: "1", C6_BOOTSTRAP_TOKEN: token, C6_E2E_BASE_URL: origin } });
  } finally {
    server?.kill("SIGTERM");
    await rm(dataDir, { recursive: true, force: true });
  }
}

function freePort() {
  return new Promise((resolvePromise, reject) => {
    const probe = createServer();
    probe.once("error", reject);
    probe.listen(0, "127.0.0.1", () => {
      const address = probe.address();
      if (!address || typeof address === "string") return reject(new Error("Could not allocate an E2E port"));
      probe.close((error) => error ? reject(error) : resolvePromise(address.port));
    });
  });
}

if (process.env.C6_E2E_SKIP_BUILD !== "1") {
  await run("npm", ["run", "build"], { cwd: webRoot });
  await run("cargo", ["build", "-p", "c6-server"], { cwd: repoRoot });
}
const requestedPort = process.env.C6_E2E_PORT ? Number(process.env.C6_E2E_PORT) : undefined;
const selectedProject = process.env.C6_E2E_PROJECT;
if (!selectedProject || selectedProject === "chromium") await testProject("chromium", requestedPort ?? await freePort());
if (!selectedProject || selectedProject === "mobile") await testProject("mobile", requestedPort ? requestedPort + 1 : await freePort());
