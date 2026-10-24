import type { NextConfig } from "next";
import pkg from "./package.json" with { type: "json" };

// Tauri loads the frontend from static files, so Next.js must export a static site (`out/`).
const nextConfig: NextConfig = {
  output: "export",
  // The dev badge ("N 3 Issues") floats over the player bar inside the Tauri window.
  devIndicators: false,
  // Only the version goes into the app. Importing package.json itself would ship every dependency name with it.
  env: { NEXT_PUBLIC_APP_VERSION: pkg.version },
};

export default nextConfig;
