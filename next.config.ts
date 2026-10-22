import type { NextConfig } from "next";

// Tauri loads the frontend from static files, so Next.js must export a static site (`out/`).
const nextConfig: NextConfig = {
  output: "export",
  // The dev badge ("N 3 Issues") floats over the player bar inside the Tauri window.
  devIndicators: false,
};

export default nextConfig;
