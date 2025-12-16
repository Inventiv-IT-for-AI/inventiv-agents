import { NextRequest } from "next/server";

function apiBaseUrl() {
  return (
    process.env.API_INTERNAL_URL ??
    process.env.NEXT_PUBLIC_API_URL ??
    "http://127.0.0.1:8003"
  ).replace(/\/+$/, "");
}

function filterHeaders(req: NextRequest) {
  const headers = new Headers();
  const cookie = req.headers.get("cookie");
  if (cookie) headers.set("cookie", cookie);
  const auth = req.headers.get("authorization");
  if (auth) headers.set("authorization", auth);
  const ct = req.headers.get("content-type");
  if (ct) headers.set("content-type", ct);
  const accept = req.headers.get("accept");
  if (accept) headers.set("accept", accept);
  return headers;
}

function sanitizeResponseHeaders(upstream: Response) {
  const headers = new Headers(upstream.headers);
  headers.delete("connection");
  headers.delete("transfer-encoding");
  headers.delete("keep-alive");
  headers.delete("proxy-authenticate");
  headers.delete("proxy-authorization");
  headers.delete("te");
  headers.delete("trailer");
  headers.delete("upgrade");
  return headers;
}

async function proxyRoot(req: NextRequest) {
  const url = apiBaseUrl();
  const method = req.method.toUpperCase();
  const upstream = await fetch(url, {
    method,
    headers: filterHeaders(req),
    body: method === "GET" || method === "HEAD" ? undefined : req.body,
    cache: "no-store",
    redirect: "manual",
  });
  return new Response(upstream.body, {
    status: upstream.status,
    headers: sanitizeResponseHeaders(upstream),
  });
}

export const GET = proxyRoot;
export const POST = proxyRoot;
export const PUT = proxyRoot;
export const PATCH = proxyRoot;
export const DELETE = proxyRoot;


