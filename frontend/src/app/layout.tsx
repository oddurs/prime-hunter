"use client";

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
        <title>primehunt dashboard</title>
        <script
          dangerouslySetInnerHTML={{
            __html: `try{document.documentElement.className=localStorage.getItem('primehunt-theme')||'dark'}catch(e){}`,
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
