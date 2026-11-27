import { spawn } from "node:child_process";
import { homedir } from "node:os";
import { createInterface } from "node:readline";

// Personal mobile builds carry only names and short descriptions, never files or paths.
const child = spawn("codex", ["app-server", "--listen", "stdio://"], {
  cwd: homedir(),
  stdio: ["pipe", "pipe", "ignore"],
});
const lines = createInterface({ input: child.stdout });
const send = (value) => child.stdin.write(`${JSON.stringify(value)}\n`);
let finished = false;
const timer = setTimeout(() => finish(), 5000);
function finish(catalog) {
  if (finished) return;
  finished = true;
  clearTimeout(timer);
  if (catalog) process.stdout.write(JSON.stringify(catalog));
  else process.exitCode = 1;
  lines.close();
  child.stdin.end();
  child.kill("SIGKILL");
}

child.on("error", () => finish());
child.on("exit", () => finish());
child.stdin.on("error", () => finish());
lines.on("line", (line) => {
  if (line.length > 2 * 1024 * 1024) return finish();
  let value;
  try {
    value = JSON.parse(line);
  } catch {
    return;
  }
  if (value.id === 1 && value.result) {
    send({ method: "initialized" });
    send({
      id: 2,
      method: "skills/list",
      params: { cwds: [homedir()], forceReload: true },
    });
  } else if (value.id === 2 && Array.isArray(value.result?.data)) {
    const skills = value.result.data
      .flatMap((entry) => entry.skills ?? [])
      .filter(
        (skill) =>
          skill.enabled === true &&
          typeof skill.name === "string" &&
          /^[a-z0-9_:.-]{1,96}$/i.test(skill.name),
      );
    const catalog = new Map();
    for (const { name, description } of skills) {
      const text = Array.from(typeof description === "string"
        ? description.trim().replace(/\s+/g, " ")
        : "").slice(0, 260).join("");
      catalog.set(name, !catalog.has(name) || catalog.get(name) === text ? text : "");
    }
    finish([...catalog].sort(([a], [b]) => a.localeCompare(b)).slice(0, 256)
      .map(([name, description]) => ({ name, description })));
  } else if (value.error) finish();
});
send({
  id: 1,
  method: "initialize",
  params: {
    clientInfo: { name: "jimin-os-mobile-build", version: "0.1.0" },
    capabilities: { experimentalApi: false },
  },
});
