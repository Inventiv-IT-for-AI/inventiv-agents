// Version information for frontend
// This file reads version from VERSION file at build time

import { apiUrl } from "./api";

// Read version from environment variable (set at build time via next.config.ts)
// Fallback to reading from VERSION file if env var not set
let frontendVersion: string = "unknown";

if (typeof process !== "undefined" && process.env.NEXT_PUBLIC_APP_VERSION) {
  frontendVersion = process.env.NEXT_PUBLIC_APP_VERSION;
} else {
  // Fallback: default version (will be replaced at build time)
  frontendVersion = "0.5.0";
}

export const FRONTEND_VERSION = frontendVersion;

// Type for version info from backend
export type BackendVersionInfo = {
  backend_version: string;
  build_time: string;
};

// Fetch backend version from API
export async function getBackendVersion(): Promise<BackendVersionInfo | null> {
  try {
    const url = apiUrl("/version");
    const response = await fetch(url);
    if (!response.ok) {
      return null;
    }
    return await response.json();
  } catch (e) {
    console.warn("Could not fetch backend version:", e);
    return null;
  }
}
