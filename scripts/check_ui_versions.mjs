import fs from "node:fs";
import path from "node:path";

const repoRoot = process.cwd();

function readText(p) {
  return fs.readFileSync(p, "utf8");
}

function readJson(p) {
  return JSON.parse(readText(p));
}

function readRepoVersion() {
  const raw = readText(path.join(repoRoot, "VERSION")).trim();
  const v = raw.split(/\s+/)[0];
  if (!/^\d+\.\d+\.\d+/.test(v)) {
    throw new Error(`Invalid VERSION file content: "${raw}"`);
  }
  return v;
}

const version = readRepoVersion();

const checks = [
  {
    name: "ia-designsys",
    path: "inventiv-ui/ia-designsys/package.json",
    expectVersion: true,
    expectWorkspaceDeps: [],
  },
  {
    name: "ia-widgets",
    path: "inventiv-ui/ia-widgets/package.json",
    expectVersion: true,
    expectWorkspaceDeps: ["ia-designsys"],
  },
  {
    name: "inventiv-frontend",
    path: "inventiv-frontend/package.json",
    expectVersion: false,
    expectWorkspaceDeps: ["ia-designsys", "ia-widgets"],
  },
];

const errors = [];
for (const c of checks) {
  const pkgPath = path.join(repoRoot, c.path);
  const pkg = readJson(pkgPath);

  if (c.expectVersion) {
    if (pkg.version !== version) {
      errors.push(`${c.name}: version=${pkg.version} (expected ${version})`);
    }
  }

  for (const dep of c.expectWorkspaceDeps) {
    const val = pkg.dependencies?.[dep];
    if (val !== "workspace:*") {
      errors.push(`${c.name}: dependency "${dep}"=${JSON.stringify(val)} (expected "workspace:*")`);
    }
  }
}

if (errors.length) {
  console.error("UI version checks failed:\n" + errors.map((e) => `- ${e}`).join("\n"));
  process.exit(1);
}

console.log(`OK: UI packages aligned to VERSION=${version}`);


