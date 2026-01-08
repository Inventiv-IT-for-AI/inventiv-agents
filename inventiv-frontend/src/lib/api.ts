// API configuration
//
// IMPORTANT:
// - Client-side code must NOT default to localhost (would call the user's browser localhost).
// - In all environments we prefer going through the same-origin Next.js proxy at `/api/backend/*`
//   (implemented as route handlers under `src/app/api/backend/*`) so cookies/session auth works reliably.
//
// Server-side: talk directly to the internal service (docker network) when available.
const SERVER_API_BASE_URL =
  process.env.API_INTERNAL_URL ??
  process.env.NEXT_PUBLIC_API_URL ??
  "http://127.0.0.1:8003";

// Browser-side: always call same-origin proxy (Next route handlers).
const BROWSER_API_BASE_URL = "/api/backend";

export const API_BASE_URL =
  typeof window === "undefined" ? SERVER_API_BASE_URL : BROWSER_API_BASE_URL;

// Helper function to build API URLs
export const apiUrl = (path: string) =>
  `${API_BASE_URL}${path.startsWith("/") ? path : `/${path}`}`;

// Re-export api-client functions for convenience
export { apiFetch, apiRequest, apiJson } from "./api-client";
