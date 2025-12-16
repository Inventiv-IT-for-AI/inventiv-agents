import type { NextConfig } from "next";

const nextConfig: NextConfig = {
  // NOTE: `/api/backend/*` is implemented as a Next route handler
  // (`src/app/api/backend/[...path]/route.ts`) to make proxying more stable in dev.
};

export default nextConfig;
