import fs from "node:fs";
import path from "node:path";

const repoRoot = process.cwd();

function readText(p) {
  return fs.readFileSync(p, "utf8");
}

function readJson(p) {
  return JSON.parse(readText(p));
}

function writeJson(p, obj) {
  const next = JSON.stringify(obj, null, 2) + "\n";
  fs.writeFileSync(p, next, "utf8");
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

const targets = [
  {
    name: "ia-designsys",
    packageJsonPath: path.join(repoRoot, "inventiv-ui/ia-designsys/package.json"),
  },
  {
    name: "ia-widgets",
    packageJsonPath: path.join(repoRoot, "inventiv-ui/ia-widgets/package.json"),
    fixDeps: (pkg) => {
      pkg.dependencies ??= {};
      // Always link workspace-local (no publication for now).
      pkg.dependencies["ia-designsys"] = "workspace:*";
      return pkg;
    },
  },
  {
    name: "inventiv-frontend",
    packageJsonPath: path.join(repoRoot, "inventiv-frontend/package.json"),
    fixDeps: (pkg) => {
      pkg.dependencies ??= {};
      // Always link workspace-local (no publication for now).
      pkg.dependencies["ia-designsys"] = "workspace:*";
      pkg.dependencies["ia-widgets"] = "workspace:*";
      return pkg;
    },
  },
];

for (const t of targets) {
  const pkgPath = t.packageJsonPath;
  const pkg = readJson(pkgPath);
  if (!pkg || typeof pkg !== "object") throw new Error(`Invalid JSON at ${pkgPath}`);

  if (t.name === "ia-designsys" || t.name === "ia-widgets") {
    pkg.version = version;
  }

  if (typeof t.fixDeps === "function") t.fixDeps(pkg);
  writeJson(pkgPath, pkg);
}

console.log(`Synced UI package versions to ${version}`);


