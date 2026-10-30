import type { Metadata } from "next";
import type { ReactNode } from "react";
import "./globals.css";
import "./themes.css";

export const metadata: Metadata = {
  title: "LunoRX",
};

export default function RootLayout({ children }: { children: ReactNode }) {
  return (
    <html lang="en">
      <body>
        {children}
      </body>
    </html>
  );
}
