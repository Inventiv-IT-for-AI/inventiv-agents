import { NextResponse } from "next/server";
import type { NextRequest } from "next/server";

export function middleware(request: NextRequest) {
  const sessionCookie = process.env.SESSION_COOKIE_NAME ?? "inventiv_session";
  const token = request.cookies.get(sessionCookie)?.value;

  const pathname = request.nextUrl.pathname;

  // Debug logging (always enabled for now to debug cookie issue)
  console.log("[Middleware] Path:", pathname);
  console.log("[Middleware] Cookie name:", sessionCookie);
  console.log("[Middleware] Cookie value:", token ? "present" : "missing");
  const allCookies = Array.from(request.cookies.getAll());
  console.log("[Middleware] All cookies:", allCookies.map(c => `${c.name}=${c.value.substring(0, 20)}...`));
  console.log("[Middleware] Cookie header:", request.headers.get("cookie"));

  // Allow access to login page, forgot password, reset password, and API routes
  if (
    pathname.startsWith("/login") ||
    pathname.startsWith("/forgot-password") ||
    pathname.startsWith("/reset-password") ||
    pathname.startsWith("/api")
  ) {
    return NextResponse.next();
  }

  // Protect all other routes - redirect to login if no token
  if (!token) {
    const loginUrl = new URL("/login", request.url);
    // Preserve the original path for redirect after login
    if (pathname !== "/" && !pathname.startsWith("/login")) {
      loginUrl.searchParams.set("redirect", pathname);
    }
    return NextResponse.redirect(loginUrl);
  }

  return NextResponse.next();
}

export const config = {
  matcher: [
    /*
     * Match all request paths except for the ones starting with:
     * - api (API routes)
     * - _next/static (static files)
     * - _next/image (image optimization files)
     * - favicon.ico (favicon file)
     */
    "/((?!api|_next/static|_next/image|favicon.ico).*)",
  ],
};

