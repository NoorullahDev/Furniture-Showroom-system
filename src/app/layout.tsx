import type { Metadata } from "next";

import { ErrorBoundary } from "@/components/error/error-boundary";
import { Providers } from "@/components/providers";
import "../styles/globals.css";

export const metadata: Metadata = {
  title: "Furniture Shop",
  description: "Furniture Shop Management System — offline-first desktop app",
};

export default function RootLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return (
    <html lang="en">
      <body className="min-h-screen">
        <ErrorBoundary>
          <Providers>{children}</Providers>
        </ErrorBoundary>
      </body>
    </html>
  );
}