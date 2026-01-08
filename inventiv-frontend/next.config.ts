import type { NextConfig } from "next";
import path from "node:path";
import fs from "node:fs";

// Read VERSION file from project root
let appVersion = "unknown";
try {
  const versionPath = path.join(__dirname, "..", "VERSION");
  if (fs.existsSync(versionPath)) {
    appVersion = fs.readFileSync(versionPath, "utf8").trim();
  }
} catch (e) {
  console.warn("Could not read VERSION file:", e);
}

const nextConfig: NextConfig = {
  // NOTE: `/api/backend/*` is implemented as a Next route handler
  // (`src/app/api/backend/[...path]/route.ts`) to make proxying more stable in dev.
  transpilePackages: ["ia-widgets", "ia-designsys"],
  experimental: {
    // Allow transpiling packages that live outside the Next.js app dir (monorepo/file deps).
    externalDir: true,
  },
  env: {
    NEXT_PUBLIC_APP_VERSION: appVersion,
  },
  webpack: (config) => {
    // When `ia-widgets` is resolved to a path outside `/app` (Docker bind mount),
    // webpack may not find deps like `@radix-ui/*` because it searches relative to that external dir.
    // Ensure `/app/node_modules` and parent `node_modules` (monorepo) are part of the module resolution chain.
    config.resolve = config.resolve || {};
    const modules = config.resolve.modules || [];
    const appNodeModules = path.resolve(__dirname, "node_modules");
    const parentNodeModules = path.resolve(__dirname, "..", "node_modules");
    // Add parent node_modules first to prioritize hoisted packages in monorepo
    config.resolve.modules = [parentNodeModules, appNodeModules, ...modules.filter((m: string) => m !== appNodeModules && m !== parentNodeModules)];
    return config;
  },
};

export default nextConfig;
