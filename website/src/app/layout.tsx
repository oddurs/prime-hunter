import type { Metadata } from "next";
import { Navbar } from "@/components/navbar";
import { Footer } from "@/components/footer";
import "./globals.css";

export const metadata: Metadata = {
  title: {
    default: "darkreach — AI-Driven Distributed Computing",
    template: "%s — darkreach",
  },
  description:
    "AI-driven distributed computing platform. Autonomously researches, optimizes, and orchestrates scientific discoveries. Currently hunting primes.",
  icons: {
    icon: "/favicon.svg",
  },
  openGraph: {
    title: "darkreach — AI-Driven Distributed Computing",
    description:
      "AI-driven distributed computing platform. Autonomously researches, optimizes, and orchestrates scientific discoveries. Currently hunting primes.",
    url: "https://darkreach.ai",
    siteName: "darkreach",
    type: "website",
  },
  twitter: {
    card: "summary_large_image",
    title: "darkreach — AI-Driven Distributed Computing",
    description:
      "AI-driven distributed computing platform. Currently hunting primes.",
  },
};

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  return (
    <html lang="en">
      <body className="min-h-screen bg-background text-foreground antialiased">
        <Navbar />
        <main className="pt-16">{children}</main>
        <Footer />
      </body>
    </html>
  );
}
