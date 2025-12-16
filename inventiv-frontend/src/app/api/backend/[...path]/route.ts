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

  // Retry only for idempotent methods on transient network failures.
  const maxAttempts = method === "GET" || method === "HEAD" ? 2 : 1;

  let lastErr: unknown = null;
  for (let attempt = 1; attempt <= maxAttempts; attempt++) {
    try {
      // IMPORTANT:
      // Avoid streaming bodies (ReadableStream) because Node/undici may require
      // `duplex: "half"` which is not always available in our TS build environment.
      // Buffering here is fine for our control-plane JSON payload sizes.
      const hasBody = method !== "GET" && method !== "HEAD";
      const body = hasBody ? await req.arrayBuffer() : undefined;
      const upstream = await fetch(url, {
        method,
        headers: filterHeaders(req),
        body: body ? new Uint8Array(body) : undefined,
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
      const h: any = upstream.headers as any;
      const setCookieHeaders: string[] | undefined =
        typeof h.getSetCookie === "function" ? h.getSetCookie() : undefined;
      if (Array.isArray(setCookieHeaders) && setCookieHeaders.length > 0) {
        for (const cookie of setCookieHeaders) {
          response.headers.append("set-cookie", cookie);
        }
      } else {
        const setCookie = upstream.headers.get("set-cookie");
        if (setCookie) response.headers.set("set-cookie", setCookie);
      }

      return response;
    } catch (e) {
      lastErr = e;
      // small backoff
      if (attempt < maxAttempts) {
        await new Promise((r) => setTimeout(r, 50));
        continue;
      }
    }
  }

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


