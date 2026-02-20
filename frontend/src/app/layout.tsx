"use client";

/**
 * @module layout
 *
 * Root layout for the entire dashboard. Wraps all pages in:
 *
 * 1. `<AuthProvider>` — Supabase Auth session management
 * 2. `<WebSocketProvider>` — single WebSocket connection to the Rust backend
 * 3. `<AppHeader>` — top navigation bar
 * 4. `<NotificationToaster>` — invisible prime discovery notifier
 * 5. `<Toaster>` — Sonner toast container
 *
 * Unauthenticated users see the login page instead of the dashboard.
 * Dark mode class is applied to the `<html>` element via `useTheme()`.
 */

import "./globals.css";

import { AuthProvider, useAuth } from "@/contexts/auth-context";
import { WebSocketProvider } from "@/contexts/websocket-context";
import { AppHeader } from "@/components/app-header";
import { NotificationToaster } from "@/components/prime-notifier";
import { Toaster } from "sonner";
import LoginPage from "@/app/login/page";

function AuthenticatedApp({ children }: { children: React.ReactNode }) {
  const { user, loading } = useAuth();

  if (loading) {
    return (
      <div className="flex items-center justify-center h-screen">
        <div className="text-muted-foreground text-sm">Loading...</div>
      </div>
    );
  }

  if (!user) {
    return <LoginPage />;
  }

  return (
    <WebSocketProvider>
      <div className="flex h-full flex-col">
        <AppHeader />
        <main className="flex-1 overflow-y-auto px-6">
          <div className="mx-auto max-w-6xl py-6">{children}</div>
        </main>
      </div>
      <NotificationToaster />
    </WebSocketProvider>
  );
}

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  return (
    <html lang="en" className="dark" suppressHydrationWarning>
      <head>
        <title>darkreach dashboard</title>
        <link rel="manifest" href="/manifest.json" />
        <meta name="theme-color" content="#f78166" />
        <meta name="apple-mobile-web-app-capable" content="yes" />
        <meta
          name="apple-mobile-web-app-status-bar-style"
          content="black-translucent"
        />
        <link rel="icon" type="image/svg+xml" href="/favicon.svg" />
        <link rel="apple-touch-icon" href="/icon-192.png" />
        <script
          dangerouslySetInnerHTML={{
            __html: `try{document.documentElement.className=localStorage.getItem('darkreach-theme')||'dark'}catch(e){}`,
          }}
        />
        <script
          dangerouslySetInnerHTML={{
            __html: `if('serviceWorker' in navigator){window.addEventListener('load',function(){navigator.serviceWorker.register('/sw.js')})}`,
          }}
        />
      </head>
      <body className="antialiased">
        <AuthProvider>
          <AuthenticatedApp>{children}</AuthenticatedApp>
        </AuthProvider>
        <Toaster
          position="bottom-right"
          toastOptions={{
            style: {
              background: "var(--card)",
              border: "1px solid var(--border)",
              color: "var(--foreground)",
            },
          }}
        />
      </body>
    </html>
  );
}
