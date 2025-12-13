// API configuration
// Use NEXT_PUBLIC_API_URL from environment, fallback to localhost for dev
export const API_BASE_URL = process.env.NEXT_PUBLIC_API_URL || 'http://localhost:8003';

// Helper function to build API URLs
export const apiUrl = (path: string) => `${API_BASE_URL}${path.startsWith('/') ? path : `/${path}`}`;
