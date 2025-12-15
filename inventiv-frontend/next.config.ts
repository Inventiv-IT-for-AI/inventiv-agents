import type { NextConfig } from "next";

const nextConfig: NextConfig = {
  async rewrites() {
    // For dev: defaults to localhost API.
    // For staging/prod: set API_INTERNAL_URL (recommended in docker-compose) or NEXT_PUBLIC_API_URL.
    const apiBase =
      process.env.API_INTERNAL_URL ??
      process.env.NEXT_PUBLIC_API_URL ??
      "http://127.0.0.1:8003";

    return [
      {
        source: "/api/backend/:path*",
        destination: `${apiBase}/:path*`,
      },
    ];
  },
};

export default nextConfig;
