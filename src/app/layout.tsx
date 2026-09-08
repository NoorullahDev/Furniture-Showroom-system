import type { Metadata } from "next";
import "../styles/globals.css";

export const metadata: Metadata = {
  title: "Furniture Shop — Technical Proof",
  description: "Phase 0 technical proof for the Furniture Shop Management System",
};

export default function RootLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return (
    <html lang="en">
      <body className="min-h-screen">{children}</body>
    </html>
  );
}