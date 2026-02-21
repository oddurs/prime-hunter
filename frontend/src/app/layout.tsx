"use client";

/**
 * @module layout
 *
 * Root layout for the entire dashboard. Wraps all pages in:
 *
 * 1. `<AuthProvider>` — Supabase Auth session management
 * 2. `<WebSocketProvider>` — single WebSocket connection to the Rust backend
 * 3. `<SidebarProvider>` + `<AppSidebar>` — collapsible sidebar navigation
 * 4. `<TopBar>` — thin header with breadcrumbs and utilities
 * 5. `<NotificationToaster>` — invisible prime discovery notifier
 * 6. `<Toaster>` — Sonner toast container
 *
 * Unauthenticated users see the login page instead of the dashboard.
 * Dark mode class is applied to the `<html>` element via `useTheme()`.
 */

import "./globals.css";

import { AuthProvider, useAuth } from "@/contexts/auth-context";
import { WebSocketProvider } from "@/contexts/websocket-context";
import { AppSidebar } from "@/components/app-sidebar";
import { TopBar } from "@/components/top-bar";
import { SidebarProvider, SidebarInset } from "@/components/ui/sidebar";
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
      <SidebarProvider className="h-full !min-h-0 overflow-hidden">
        <AppSidebar />
        <SidebarInset className="overflow-hidden">
          <TopBar />
          <div className="flex-1 overflow-y-auto">
            <div className="container mx-auto px-6 py-6">{children}</div>
          </div>
        </SidebarInset>
      </SidebarProvider>
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
        <meta name="theme-color" content="#0c0a1d" />
        <meta name="apple-mobile-web-app-capable" content="yes" />
        <meta
          name="apple-mobile-web-app-status-bar-style"
          content="black-translucent"
        />
        <link rel="icon" type="image/svg+xml" href="/favicon.svg" />
        <link rel="apple-touch-icon" href="/icon-192.png" />
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
