import type { NextConfig } from "next";

const nextConfig: NextConfig = {
  async rewrites() {
    return [
      {
        source: '/rpc/:path*',
        destination: 'https://soroban-testnet.stellar.org/:path*',
      },
    ];
  }
};

export default nextConfig;
