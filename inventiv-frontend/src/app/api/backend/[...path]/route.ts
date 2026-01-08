import { NextRequest, NextResponse } from "next/server";

function apiBaseUrl() {
  return (
    process.env.API_INTERNAL_URL ??
    process.env.NEXT_PUBLIC_API_URL ??
    "http://127.0.0.1:8003"
  ).replace(/\/+$/, "");
}

function buildTargetUrl(req: NextRequest, pathParts: string[]) {
  const incoming = new URL(req.url);
  const target = new URL(`${apiBaseUrl()}/${pathParts.join("/")}`);
  target.search = incoming.search;
  return target.toString();
}

function filterHeaders(req: NextRequest) {
  const headers = new Headers();

  // Forward cookies so session auth works.
  const cookie = req.headers.get("cookie");
  if (cookie) headers.set("cookie", cookie);

  // Forward Authorization if ever used.
  const auth = req.headers.get("authorization");
  if (auth) headers.set("authorization", auth);

  // Preserve content-type for JSON bodies, etc.
  const ct = req.headers.get("content-type");
  if (ct) headers.set("content-type", ct);

  // Forward accept headers
  const accept = req.headers.get("accept");
  if (accept) headers.set("accept", accept);

  return headers;
}

function sanitizeResponseHeaders(upstream: Response) {
  const headers = new Headers(upstream.headers);
  // Avoid hop-by-hop headers.
  headers.delete("connection");
  headers.delete("transfer-encoding");
  headers.delete("keep-alive");
  headers.delete("proxy-authenticate");
  headers.delete("proxy-authorization");
  headers.delete("te");
  headers.delete("trailer");
  headers.delete("upgrade");
  // IMPORTANT: Preserve Set-Cookie headers so session cookies work
  // Next.js will automatically forward these to the browser
  return headers;
}

async function proxy(req: NextRequest, ctx: { params: Promise<{ path?: string[] }> }) {
  const { path = [] } = await ctx.params;
  const url = buildTargetUrl(req, path);
  const method = req.method.toUpperCase();
  console.log("[Proxy] Request:", method, url);
  console.log("[Proxy] API_INTERNAL_URL:", process.env.API_INTERNAL_URL);
  console.log("[Proxy] NEXT_PUBLIC_API_URL:", process.env.NEXT_PUBLIC_API_URL);

  // Retry only for idempotent methods on transient network failures.
  const maxAttempts = method === "GET" || method === "HEAD" ? 2 : 1;

  let lastErr: unknown = null;
  for (let attempt = 1; attempt <= maxAttempts; attempt++) {
    try {
      const body = method === "GET" || method === "HEAD" ? undefined : req.body;
      const upstream = await fetch(url, {
        method,
        headers: filterHeaders(req),
        body,
        // Next.js 15+ requires duplex option when sending a body.
        // `RequestDuplex` is not available in all TS lib.dom versions, so keep this cast minimal.
        ...(body ? ({ duplex: "half" } as unknown as { duplex: "half" }) : {}),
        // Never cache proxied API calls.
        cache: "no-store",
        redirect: "manual",
      });

      // Create response with proper cookie handling
      const response = new NextResponse(upstream.body, {
        status: upstream.status,
        headers: sanitizeResponseHeaders(upstream),
      });

      // Explicitly copy Set-Cookie headers from upstream to response
      // This ensures cookies are properly forwarded to the browser
      // Try both getSetCookie() (Next.js 15+) and manual header reading
      const setCookieHeaders = upstream.headers.getSetCookie();
      const setCookieHeaderRaw = upstream.headers.get("set-cookie");
      
      console.log("[Proxy] Set-Cookie headers from upstream (getSetCookie):", setCookieHeaders);
      console.log("[Proxy] Set-Cookie header raw:", setCookieHeaderRaw);
      
      // Use getSetCookie() if available (Next.js 15+)
      if (setCookieHeaders && setCookieHeaders.length > 0) {
        for (const cookie of setCookieHeaders) {
          response.headers.append("Set-Cookie", cookie);
          console.log("[Proxy] Added Set-Cookie to response:", cookie.substring(0, 50) + "...");
        }
      } 
      // Fallback: manually read set-cookie header if getSetCookie() doesn't work
      else if (setCookieHeaderRaw) {
        // Split multiple Set-Cookie headers (they can be comma-separated or multiple headers)
        const cookies = setCookieHeaderRaw.split(/\s*,\s*(?=[^=]+=)/);
        for (const cookie of cookies) {
          response.headers.append("Set-Cookie", cookie.trim());
          console.log("[Proxy] Added Set-Cookie to response (fallback):", cookie.substring(0, 50) + "...");
        }
      } else {
        console.warn("[Proxy] No Set-Cookie header found in upstream response!");
      }

      return response;
    } catch (e) {
      lastErr = e;
      console.error(`[Proxy] Error on attempt ${attempt}/${maxAttempts}:`, e);
      console.error(`[Proxy] Error details:`, {
        message: e instanceof Error ? e.message : String(e),
        stack: e instanceof Error ? e.stack : undefined,
        target: url,
      });
      // small backoff
      if (attempt < maxAttempts) {
        await new Promise((r) => setTimeout(r, 50));
        continue;
      }
    }
  }

  console.error(`[Proxy] All attempts failed. Last error:`, lastErr);
  return new Response(
    JSON.stringify({
      error: "proxy_error",
      message: lastErr instanceof Error ? lastErr.message : String(lastErr),
      target: url,
    }),
    { status: 502, headers: { "content-type": "application/json" } },
  );
}

export const GET = proxy;
export const POST = proxy;
export const PUT = proxy;
export const PATCH = proxy;
export const DELETE = proxy;


