"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
import {
  BarChart3,
  BookOpen,
  Bot,
  FileText,
  FolderKanban,
  Globe,
  LayoutDashboard,
  LogOut,
  Rocket,
  Search,
  Table,
  Trophy,
  User,
  Zap,
} from "lucide-react";
import { useWs } from "@/contexts/websocket-context";
import { useAuth } from "@/contexts/auth-context";
import { cn } from "@/lib/utils";
import { DarkReachLogo } from "@/components/darkreach-logo";
import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarGroup,
  SidebarGroupContent,
  SidebarGroupLabel,
  SidebarHeader,
  SidebarMenu,
  SidebarMenuBadge,
  SidebarMenuButton,
  SidebarMenuItem,
  SidebarRail,
  SidebarSeparator,
  useSidebar,
} from "@/components/ui/sidebar";

type NavItem = {
  title: string;
  href: string;
  icon: React.ComponentType<{ className?: string }>;
  badge?: number;
};

type NavSection = {
  title: string;
  items: NavItem[];
};

export function AppSidebar() {
  const pathname = usePathname();
  const { searches, agentTasks } = useWs();
  const { user, role, signOut } = useAuth();
  const { state } = useSidebar();

  const runningCount = searches.filter((s) => s.status === "running").length;
  const activeAgentCount = agentTasks.filter(
    (t) => t.status === "in_progress"
  ).length;

  const isAdmin = role === "admin";

  const initials = user?.email
    ? user.email
        .split("@")[0]
        .split(/[._-]/)
        .slice(0, 2)
        .map((s) => s[0]?.toUpperCase() ?? "")
        .join("")
    : "?";

  function isActive(href: string) {
    return href === "/" ? pathname === "/" : pathname.startsWith(href);
  }

  if (!isAdmin) {
    const operatorItems: NavItem[] = [
      { title: "Dashboard", href: "/", icon: LayoutDashboard },
      { title: "Browse", href: "/browse", icon: Table },
      { title: "My Nodes", href: "/my-nodes", icon: Globe },
      { title: "Leaderboard", href: "/leaderboard", icon: Trophy },
      { title: "Account", href: "/account", icon: User },
    ];

    return (
      <Sidebar side="left" collapsible="icon">
        <SidebarHeader className="h-12 justify-center border-b border-sidebar-border">
          <SidebarMenu>
            <SidebarMenuItem>
              <SidebarMenuButton size="lg" asChild tooltip="darkreach" className="[&>svg]:size-auto">
                <Link href="/">
                  <DarkReachLogo size={32} className="text-[#6366f1] shrink-0" />
                  <span className="font-semibold tracking-tight text-base">darkreach</span>
                </Link>
              </SidebarMenuButton>
            </SidebarMenuItem>
          </SidebarMenu>
        </SidebarHeader>
        <SidebarContent>
          <SidebarGroup>
            <SidebarGroupContent>
              <SidebarMenu>
                {operatorItems.map((item) => (
                  <SidebarMenuItem key={item.href}>
                    <SidebarMenuButton
                      asChild
                      isActive={isActive(item.href)}
                      tooltip={item.title}
                      className={cn(
                        isActive(item.href) &&
                          "border-l-2 border-[#6366f1]"
                      )}
                    >
                      <Link href={item.href}>
                        <item.icon />
                        <span>{item.title}</span>
                      </Link>
                    </SidebarMenuButton>
                  </SidebarMenuItem>
                ))}
              </SidebarMenu>
            </SidebarGroupContent>
          </SidebarGroup>
          <SidebarSeparator />
          <SidebarGroup>
            <SidebarGroupContent>
              <SidebarMenu>
                <SidebarMenuItem>
                  <SidebarMenuButton asChild tooltip="Docs">
                    <Link href="/docs">
                      <BookOpen />
                      <span>Docs</span>
                    </Link>
                  </SidebarMenuButton>
                </SidebarMenuItem>
              </SidebarMenu>
            </SidebarGroupContent>
          </SidebarGroup>
        </SidebarContent>
        <SidebarFooter>
          {user && (
            <SidebarMenu>
              <SidebarMenuItem>
                <SidebarMenuButton
                  size="lg"
                  tooltip={user.email?.split("@")[0] ?? "User"}
                  className="cursor-default"
                >
                  <div className="flex size-8 items-center justify-center rounded-full bg-muted text-xs font-semibold shrink-0">
                    {initials}
                  </div>
                  <div className="flex flex-col min-w-0">
                    <div className="flex items-center gap-1.5">
                      <span className="text-sm font-medium truncate">
                        {user.email?.split("@")[0] ?? "User"}
                      </span>
                      {role && (
                        <span className="px-1 py-0.5 rounded text-[9px] font-semibold uppercase leading-none shrink-0 bg-emerald-500/20 text-emerald-400">
                          {role}
                        </span>
                      )}
                    </div>
                    <span className="text-xs text-muted-foreground truncate">
                      {user.email}
                    </span>
                  </div>
                </SidebarMenuButton>
              </SidebarMenuItem>
              <SidebarMenuItem>
                <SidebarMenuButton
                  onClick={() => signOut()}
                  tooltip="Sign out"
                  className="text-muted-foreground hover:text-foreground"
                >
                  <LogOut />
                  <span>Sign out</span>
                </SidebarMenuButton>
              </SidebarMenuItem>
            </SidebarMenu>
          )}
        </SidebarFooter>
        <SidebarRail />
      </Sidebar>
    );
  }

  // Admin navigation with grouped sections
  const overview: NavItem[] = [
    { title: "Dashboard", href: "/", icon: LayoutDashboard },
  ];

  const discovery: NavSection = {
    title: "Discovery",
    items: [
      { title: "Browse", href: "/browse", icon: Table },
      {
        title: "Searches",
        href: "/searches",
        icon: Search,
        badge: runningCount || undefined,
      },
      { title: "Projects", href: "/projects", icon: FolderKanban },
      { title: "Leaderboard", href: "/leaderboard", icon: Trophy },
    ],
  };

  const operations: NavSection = {
    title: "Operations",
    items: [
      { title: "Network", href: "/network", icon: Globe },
      { title: "Observability", href: "/performance", icon: BarChart3 },
      { title: "Logs", href: "/logs", icon: FileText },
      { title: "Releases", href: "/releases", icon: Rocket },
      { title: "Strategy", href: "/strategy", icon: Zap },
    ],
  };

  const intelligence: NavSection = {
    title: "Intelligence",
    items: [
      {
        title: "Agents",
        href: "/agents",
        icon: Bot,
        badge: activeAgentCount || undefined,
      },
    ],
  };

  const sections = [discovery, operations, intelligence];

  return (
    <Sidebar side="left" collapsible="icon">
      <SidebarHeader className="h-12 justify-center border-b border-sidebar-border">
        <SidebarMenu>
          <SidebarMenuItem>
            <SidebarMenuButton size="lg" asChild tooltip="darkreach" className="[&>svg]:size-auto">
              <Link href="/">
                <DarkReachLogo size={32} className="text-[#6366f1] shrink-0" />
                <span className="font-semibold tracking-tight text-base">darkreach</span>
              </Link>
            </SidebarMenuButton>
          </SidebarMenuItem>
        </SidebarMenu>
      </SidebarHeader>
      <SidebarContent>
        {/* Overview (Dashboard) */}
        <SidebarGroup>
          <SidebarGroupContent>
            <SidebarMenu>
              {overview.map((item) => (
                <SidebarMenuItem key={item.href}>
                  <SidebarMenuButton
                    asChild
                    isActive={isActive(item.href)}
                    tooltip={item.title}
                    className={cn(
                      isActive(item.href) &&
                        "border-l-2 border-[#6366f1]"
                    )}
                  >
                    <Link href={item.href}>
                      <item.icon />
                      <span>{item.title}</span>
                    </Link>
                  </SidebarMenuButton>
                </SidebarMenuItem>
              ))}
            </SidebarMenu>
          </SidebarGroupContent>
        </SidebarGroup>

        {/* Grouped sections: Discovery, Operations, Intelligence */}
        {sections.map((section) => (
          <SidebarGroup key={section.title}>
            <SidebarGroupLabel>{section.title}</SidebarGroupLabel>
            <SidebarGroupContent>
              <SidebarMenu>
                {section.items.map((item) => (
                  <SidebarMenuItem key={item.href}>
                    <SidebarMenuButton
                      asChild
                      isActive={isActive(item.href)}
                      tooltip={item.title}
                      className={cn(
                        isActive(item.href) &&
                          "border-l-2 border-[#6366f1]"
                      )}
                    >
                      <Link href={item.href}>
                        <item.icon />
                        <span>{item.title}</span>
                      </Link>
                    </SidebarMenuButton>
                    {item.badge != null && (
                      <SidebarMenuBadge>{item.badge}</SidebarMenuBadge>
                    )}
                  </SidebarMenuItem>
                ))}
              </SidebarMenu>
            </SidebarGroupContent>
          </SidebarGroup>
        ))}
      </SidebarContent>
      <SidebarFooter>
        <SidebarSeparator />
        <SidebarMenu>
          <SidebarMenuItem>
            <SidebarMenuButton asChild tooltip="Docs">
              <Link href="/docs">
                <BookOpen />
                <span>Docs</span>
              </Link>
            </SidebarMenuButton>
          </SidebarMenuItem>
        </SidebarMenu>
        {user && (
          <SidebarMenu>
            <SidebarMenuItem>
              <SidebarMenuButton
                size="lg"
                tooltip={user.email?.split("@")[0] ?? "User"}
                className="cursor-default"
              >
                <div className="flex size-8 items-center justify-center rounded-full bg-muted text-xs font-semibold shrink-0">
                  {initials}
                </div>
                <div className="flex flex-col min-w-0">
                  <div className="flex items-center gap-1.5">
                    <span className="text-sm font-medium truncate">
                      {user.email?.split("@")[0] ?? "User"}
                    </span>
                    {role && (
                      <span
                        className={cn(
                          "px-1 py-0.5 rounded text-[9px] font-semibold uppercase leading-none shrink-0",
                          role === "admin"
                            ? "bg-indigo-500/20 text-indigo-400"
                            : "bg-emerald-500/20 text-emerald-400"
                        )}
                      >
                        {role}
                      </span>
                    )}
                  </div>
                  <span className="text-xs text-muted-foreground truncate">
                    {user.email}
                  </span>
                </div>
              </SidebarMenuButton>
            </SidebarMenuItem>
            <SidebarMenuItem>
              <SidebarMenuButton
                onClick={() => signOut()}
                tooltip="Sign out"
                className="text-muted-foreground hover:text-foreground"
              >
                <LogOut />
                <span>Sign out</span>
              </SidebarMenuButton>
            </SidebarMenuItem>
          </SidebarMenu>
        )}
      </SidebarFooter>
      <SidebarRail />
    </Sidebar>
  );
}
