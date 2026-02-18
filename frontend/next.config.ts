import type { NextConfig } from "next";

const proxyTarget = process.env.DEV_PROXY_TARGET?.replace(/\/+$/, "");

const nextConfig: NextConfig = {
  output: "export",
  trailingSlash: true,
  images: { unoptimized: true },
  async rewrites() {
    if (!proxyTarget) return [];
    return [
      {
        source: "/api/:path*",
        destination: `${proxyTarget}/api/:path*`,
      },
      {
        source: "/ws",
        destination: `${proxyTarget}/ws`,
      },
    ];
  },
};

export default nextConfig;
