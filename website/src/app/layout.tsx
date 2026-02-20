import type { Metadata } from "next";
import "./globals.css";

export const metadata: Metadata = {
  title: "Darkreach — Distributed Prime Discovery",
  description:
    "Hunt special-form primes across CPU clusters. 12 algorithms. Deterministic proofs. Open source.",
  icons: {
    icon: "/favicon.svg",
  },
};

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  return (
    <html lang="en">
      <body>{children}</body>
    </html>
  );
}
