const { spawnSync } = require("node:child_process");

const configured = process.env.PYTHON ? [[process.env.PYTHON, []]] : [];
const platformCandidates =
  process.platform === "win32"
    ? [
        ["py", ["-3"]],
        ["python", []],
        ["python3", []],
      ]
    : [
        ["python3", []],
        ["python", []],
      ];

for (const [command, prefix] of [...configured, ...platformCandidates]) {
  const probe = spawnSync(command, [...prefix, "--version"], { encoding: "utf8" });
  if (probe.status !== 0) continue;

  const result = spawnSync(
    command,
    [
      ...prefix,
      "-m",
      "unittest",
      "discover",
      "-s",
      "docker/yt-dlp-service",
      "-p",
      "test_*.py",
    ],
    { stdio: "inherit" },
  );
  process.exit(result.status ?? 1);
}

console.error("Python 3 was not found. Install it or set PYTHON to the Python executable.");
process.exit(1);
