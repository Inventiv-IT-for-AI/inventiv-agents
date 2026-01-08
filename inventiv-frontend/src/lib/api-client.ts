/**
 * Centralized API client with automatic 401 handling
 * 
 * This wrapper around fetch automatically redirects to /login when a 401 response is received.
 * Use this instead of raw fetch() for all API calls that require authentication.
 */

import { apiUrl } from "./api";

// Track if we're already redirecting to avoid multiple redirects
let isRedirecting = false;

/**
 * Wrapper around fetch that automatically handles 401 responses by redirecting to /login
 * 
 * @param input - Same as fetch() first parameter (URL or Request)
 * @param init - Same as fetch() second parameter (RequestInit)
 * @returns Promise<Response>
 */
export async function apiFetch(
  input: RequestInfo | URL,
  init?: RequestInit
): Promise<Response> {
  // Ensure credentials are included for cookie-based auth
  const options: RequestInit = {
    credentials: "include",
    ...init,
    headers: {
      ...init?.headers,
    },
  };

  const response = await fetch(input, options);

  // Handle 401 Unauthorized responses
  // BUT: Don't redirect if we're already on the login page (prevents loops)
  if (response.status === 401 && typeof window !== "undefined") {
    const currentPath = window.location.pathname;
    
    // If we're already on login page, don't redirect (prevents loops)
    if (currentPath.startsWith("/login")) {
      console.log("[apiFetch] 401 on login page, skipping redirect to prevent loop");
      return response;
    }
    
    // Prevent multiple simultaneous redirects
    if (!isRedirecting) {
      isRedirecting = true;
      
      // Clear any existing session data
      try {
        // Clear any localStorage items if needed
        // localStorage.clear(); // Uncomment if you want to clear all localStorage
      } catch (e) {
        console.error("Failed to clear localStorage:", e);
      }

      // Redirect to login page
      window.location.href = "/login";
      
      // Reset flag after a delay (in case redirect fails)
      setTimeout(() => {
        isRedirecting = false;
      }, 1000);
    }
    
    // Return the response anyway so callers can handle it if needed
    return response;
  }

  return response;
}

/**
 * Convenience wrapper that uses apiUrl() and apiFetch()
 * 
 * @param path - API path (will be prefixed with /api/backend)
 * @param init - RequestInit options
 * @returns Promise<Response>
 */
export async function apiRequest(
  path: string,
  init?: RequestInit
): Promise<Response> {
  return apiFetch(apiUrl(path), init);
}

/**
 * JSON API request helper that automatically parses JSON and handles 401
 * 
 * @param path - API path
 * @param init - RequestInit options
 * @returns Promise with parsed JSON data
 */
export async function apiJson<T = unknown>(
  path: string,
  init?: RequestInit
): Promise<T> {
  const response = await apiRequest(path, {
    ...init,
    headers: {
      "Content-Type": "application/json",
      ...init?.headers,
    },
  });

  if (!response.ok) {
    // If it's a 401, the redirect will happen automatically
    // But we should still throw an error for the caller to handle
    const errorText = await response.text().catch(() => "Unknown error");
    throw new Error(`API request failed: ${response.status} ${errorText}`);
  }

  return response.json();
}

