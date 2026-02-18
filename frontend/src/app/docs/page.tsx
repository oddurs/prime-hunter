"use client";

/**
 * @module docs/page
 *
 * Documentation viewer page. Fetches markdown documents from the Rust
 * backend's `/api/docs` endpoint and renders them with:
 *
 * - **GitHub-Flavored Markdown** (tables, task lists, strikethrough)
 * - **KaTeX math rendering** (inline `$...$` and display `$$...$$`)
 * - **Syntax highlighting** for code blocks
 * - **Sidebar navigation** with search across all documents
 *
 * Documents are sourced from the `docs/` directory in the repository,
 * including roadmaps, research notes, and algorithm documentation.
 */

import { useEffect, useState, useCallback, Suspense, useMemo } from "react";
import { useSearchParams, useRouter, usePathname } from "next/navigation";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import remarkMath from "remark-math";
import rehypeKatex from "rehype-katex";
import "katex/dist/katex.min.css";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Input } from "@/components/ui/input";
import { ScrollArea } from "@/components/ui/scroll-area";
import {
  Sheet,
  SheetContent,
  SheetHeader,
  SheetTitle,
  SheetTrigger,
} from "@/components/ui/sheet";
import Link from "next/link";
import { BookOpen, Map, Bot, Search, X, Menu } from "lucide-react";
import { API_BASE } from "@/lib/format";
import { useIsMobile } from "@/hooks/use-mobile";

interface DocEntry {
  slug: string;
  title: string;
  form?: string;
  category?: string;
}

interface DocContent {
  slug: string;
  title: string;
  content: string;
  category?: string;
}

interface SearchSnippet {
  text: string;
  line: number;
}

interface SearchResult {
  slug: string;
  title: string;
  snippets: SearchSnippet[];
  category?: string;
}

interface FormCount {
  form: string;
  count: number;
}

type TabValue = "research" | "roadmaps" | "agent";

const TABS: { value: TabValue; label: string; icon: typeof BookOpen }[] = [
  { value: "research", label: "Research", icon: BookOpen },
  { value: "roadmaps", label: "Roadmaps", icon: Map },
  { value: "agent", label: "Agent Files", icon: Bot },
];

function tabForDoc(doc: DocEntry): TabValue {
  if (doc.category === "roadmaps") return "roadmaps";
  if (doc.category === "agent") return "agent";
  return "research";
}

function stripLeadingHeading(content: string): string {
  // Always strip the first # heading — the title is shown in the UI header
  return content.replace(/^#\s+.+?\s*(?:\r?\n)+/, "");
}

function DocsPageInner() {
  const searchParams = useSearchParams();
  const router = useRouter();
  const pathname = usePathname();
  const isMobile = useIsMobile();
  const [docs, setDocs] = useState<DocEntry[]>([]);
  const [activeDoc, setActiveDoc] = useState<DocContent | null>(null);
  const [activeTab, setActiveTab] = useState<TabValue>("research");
  const [initialLoading, setInitialLoading] = useState(true);
  const [docSwitching, setDocSwitching] = useState(false);
  const [docsLoadError, setDocsLoadError] = useState<string | null>(null);
  const [docLoadError, setDocLoadError] = useState<string | null>(null);
  const [mobileNavOpen, setMobileNavOpen] = useState(false);

  // Search state
  const [searchQuery, setSearchQuery] = useState("");
  const [debouncedQuery, setDebouncedQuery] = useState("");
  const [searchResults, setSearchResults] = useState<SearchResult[]>([]);

  // Cross-reference state
  const [formCounts, setFormCounts] = useState<FormCount[]>([]);

  // Debounce search input
  useEffect(() => {
    const timer = setTimeout(() => {
      setDebouncedQuery(searchQuery);
    }, 300);
    return () => clearTimeout(timer);
  }, [searchQuery]);

  // Fetch search results
  useEffect(() => {
    if (!debouncedQuery) {
      return;
    }
    fetch(
      `${API_BASE}/api/docs/search?q=${encodeURIComponent(debouncedQuery)}`
    )
      .then((r) => r.json())
      .then((data) => {
        setSearchResults(data.results || []);
      })
      .catch(() => {
        setSearchResults([]);
      });
  }, [debouncedQuery]);

  // Fetch form counts for cross-references
  useEffect(() => {
    fetch(`${API_BASE}/api/stats`)
      .then((r) => r.json())
      .then((data) => {
        if (data.by_form) {
          setFormCounts(data.by_form);
        }
      })
      .catch(() => {});
  }, []);

  const loadDoc = useCallback((slug: string, isInitial = false) => {
    setDocLoadError(null);
    if (isInitial) setInitialLoading(true);
    else setDocSwitching(true);
    let url: string;
    if (slug.startsWith("roadmaps/")) {
      url = `${API_BASE}/api/docs/roadmaps/${slug.replace("roadmaps/", "")}`;
    } else if (slug.startsWith("agent/")) {
      url = `${API_BASE}/api/docs/agent/${slug.replace("agent/", "")}`;
    } else {
      url = `${API_BASE}/api/docs/${slug}`;
    }
    fetch(url)
      .then((r) => r.json())
      .then((data) => {
        setActiveDoc(data);
        setInitialLoading(false);
        setDocSwitching(false);
      })
      .catch(() => {
        setDocLoadError("Failed to load document");
        setInitialLoading(false);
        setDocSwitching(false);
      });
  }, []);

  const updateUrl = useCallback(
    (tab: TabValue, doc?: string) => {
      const params = new URLSearchParams();
      params.set("tab", tab);
      if (doc) params.set("doc", doc);
      const query = params.toString();
      router.replace(query ? `${pathname}?${query}` : pathname, {
        scroll: false,
      });
    },
    [pathname, router]
  );

  const openDoc = useCallback(
    (slug: string, clearSearch = false) => {
      loadDoc(slug);
      if (clearSearch) {
        setSearchQuery("");
      }
      if (isMobile) {
        setMobileNavOpen(false);
      }
      // Find the tab for this doc
      const docEntry = docs.find((d) => d.slug === slug);
      const tab = docEntry ? tabForDoc(docEntry) : activeTab;
      if (tab !== activeTab) {
        setActiveTab(tab);
      }
      updateUrl(tab, slug);
    },
    [isMobile, loadDoc, docs, activeTab, updateUrl]
  );

  const switchTab = useCallback(
    (tab: TabValue) => {
      setActiveTab(tab);
      setSearchQuery("");
      // Auto-select first doc in the new tab
      const tabDocs = docs.filter((d) => tabForDoc(d) === tab);
      if (tabDocs.length > 0) {
        loadDoc(tabDocs[0].slug);
        updateUrl(tab, tabDocs[0].slug);
      } else {
        setActiveDoc(null);
        updateUrl(tab);
      }
    },
    [docs, loadDoc, updateUrl]
  );

  // Load docs list and handle URL-driven doc selection
  useEffect(() => {
    fetch(`${API_BASE}/api/docs`)
      .then((r) => r.json())
      .then((data) => {
        setDocsLoadError(null);
        setDocs(data.docs);
        const tabParam = searchParams.get("tab") as TabValue | null;
        const docParam = searchParams.get("doc");

        // Set the active tab from URL
        if (tabParam && TABS.some((t) => t.value === tabParam)) {
          setActiveTab(tabParam);
        } else if (docParam) {
          // Infer tab from doc slug
          const docEntry = (data.docs as DocEntry[]).find(
            (d) => d.slug === docParam
          );
          if (docEntry) {
            setActiveTab(tabForDoc(docEntry));
          }
        }

        // Load the requested doc or first in tab
        if (
          docParam &&
          data.docs.some((d: DocEntry) => d.slug === docParam)
        ) {
          loadDoc(docParam, true);
        } else {
          const resolvedTab =
            tabParam && TABS.some((t) => t.value === tabParam)
              ? tabParam
              : "research";
          const tabDocs = (data.docs as DocEntry[]).filter(
            (d) => tabForDoc(d) === resolvedTab
          );
          if (tabDocs.length > 0) {
            loadDoc(tabDocs[0].slug, true);
          } else {
            setInitialLoading(false);
          }
        }
      })
      .catch(() => {
        setDocsLoadError("Failed to load docs index");
        setInitialLoading(false);
      });
  }, [searchParams, loadDoc]);

  // Docs filtered by active tab
  const tabDocs = useMemo(
    () => docs.filter((d) => tabForDoc(d) === activeTab),
    [docs, activeTab]
  );

  // Get form for the active doc
  const activeDocEntry = docs.find((d) => d.slug === activeDoc?.slug);
  const docForm = activeDocEntry?.form;
  const formCount = docForm
    ? formCounts.find((f) => f.form === docForm)?.count ?? 0
    : 0;

  const renderedDocContent = useMemo(() => {
    if (!activeDoc) return "";
    let content = stripLeadingHeading(activeDoc.content);
    // Auto-link bare OEIS references (A######) that aren't already in links
    content = content.replace(
      /(?<!\[)(?<!\(https?:\/\/oeis\.org\/)\b(A\d{6,7})\b(?!\])/g,
      "[$1](https://oeis.org/$1)"
    );
    return content;
  }, [activeDoc]);

  // Filter search results by active tab
  const filteredSearchResults = useMemo(() => {
    if (!debouncedQuery) return [];
    return searchResults.filter((r) => {
      if (activeTab === "roadmaps") return r.category === "roadmaps";
      if (activeTab === "agent") return r.category === "agent";
      return !r.category;
    });
  }, [searchResults, activeTab, debouncedQuery]);

  const sidebarContent = (
    <>
      {/* Search */}
      <div className="relative mb-3">
        <Search className="absolute left-2.5 top-1/2 -translate-y-1/2 h-4 w-4 text-muted-foreground" />
        <Input
          placeholder="Search..."
          value={searchQuery}
          onChange={(e) => setSearchQuery(e.target.value)}
          className="pl-9 pr-8 h-8 text-sm"
        />
        {searchQuery && (
          <button
            onClick={() => setSearchQuery("")}
            className="absolute right-2.5 top-1/2 -translate-y-1/2 text-muted-foreground hover:text-foreground"
            aria-label="Clear search"
          >
            <X className="h-3.5 w-3.5" />
          </button>
        )}
      </div>

      <ScrollArea
        className={
          isMobile
            ? "h-[calc(100vh-11rem)] pr-1"
            : "h-[calc(100vh-17rem)]"
        }
      >
        {debouncedQuery ? (
          <div>
            <h3 className="text-xs font-semibold text-muted-foreground uppercase tracking-wider mb-2 px-1">
              Results
            </h3>
            {filteredSearchResults.length === 0 ? (
              <p className="text-sm text-muted-foreground px-2">
                No results found
              </p>
            ) : (
              <nav className="flex flex-col gap-0.5">
                {filteredSearchResults.map((result) => (
                  <button
                    key={result.slug}
                    className="text-left px-2 py-1.5 rounded-md hover:bg-secondary/50 transition-colors"
                    onClick={() => openDoc(result.slug, true)}
                  >
                    <div className="text-sm font-medium text-foreground">
                      {result.title}
                    </div>
                    {result.snippets.length > 0 && (
                      <div className="text-xs text-muted-foreground mt-0.5 line-clamp-2">
                        {result.snippets[0].text}
                      </div>
                    )}
                  </button>
                ))}
              </nav>
            )}
          </div>
        ) : (
          <nav className="flex flex-col gap-0.5">
            {tabDocs.map((doc) => {
              const isActive = activeDoc?.slug === doc.slug;
              return (
                <button
                  key={doc.slug}
                  onClick={() => openDoc(doc.slug)}
                  className={`text-left text-sm px-2.5 py-1.5 rounded-md transition-colors ${
                    isActive
                      ? "bg-secondary text-foreground font-medium"
                      : "text-muted-foreground hover:text-foreground hover:bg-secondary/50"
                  }`}
                >
                  {doc.title}
                </button>
              );
            })}
          </nav>
        )}
      </ScrollArea>
    </>
  );

  const tabBar = (
    <div className="flex items-center gap-0">
      {TABS.map((tab) => {
        const Icon = tab.icon;
        const isActive = activeTab === tab.value;
        const count = docs.filter(
          (d) => tabForDoc(d) === tab.value
        ).length;
        return (
          <button
            key={tab.value}
            onClick={() => switchTab(tab.value)}
            className={`relative flex items-center gap-1.5 px-4 py-2.5 text-sm font-medium transition-colors ${
              isActive
                ? "text-foreground"
                : "text-muted-foreground hover:text-foreground"
            }`}
          >
            <Icon className="h-4 w-4" />
            {tab.label}
            {count > 0 && (
              <span className="inline-flex items-center justify-center min-w-[18px] h-[18px] px-1 text-[11px] font-medium leading-none rounded-full bg-secondary text-muted-foreground">
                {count}
              </span>
            )}
            {isActive && (
              <span className="absolute bottom-0 left-2 right-2 h-[2px] bg-[#f78166] rounded-full" />
            )}
          </button>
        );
      })}
    </div>
  );

  return (
    <div className="flex flex-col gap-0">
      {/* Tab bar + sidebar + content use the same flex columns */}
      <div className="flex flex-col md:flex-row gap-0 md:gap-6 border-b border-border">
        <div className="hidden md:block w-52 flex-shrink-0" />
        {tabBar}
      </div>

      {/* Main layout — same column widths as tab bar row */}
      <div className="flex flex-col md:flex-row gap-4 md:gap-6 pt-4">
        {/* Desktop sidebar */}
        <aside className="hidden md:block w-52 flex-shrink-0">
          {sidebarContent}
        </aside>

        {/* Content */}
        <main className="flex-1 min-w-0">
          {/* Mobile nav trigger */}
          <div className="md:hidden mb-3">
            <Sheet open={mobileNavOpen} onOpenChange={setMobileNavOpen}>
              <SheetTrigger asChild>
                <Button variant="outline" size="sm" className="w-full gap-2">
                  <Menu className="h-4 w-4" />
                  Browse docs
                </Button>
              </SheetTrigger>
              <SheetContent side="left" className="w-[92vw] max-w-[28rem]">
                <SheetHeader>
                  <SheetTitle>Documentation</SheetTitle>
                </SheetHeader>
                <div className="px-4 pb-4">{sidebarContent}</div>
              </SheetContent>
            </Sheet>
          </div>

          {docsLoadError ? (
            <div className="py-12 text-center text-muted-foreground space-y-3">
              <p>{docsLoadError}</p>
              <Button variant="outline" onClick={() => router.refresh()}>
                Retry
              </Button>
            </div>
          ) : initialLoading ? (
            <div className="py-12 text-center text-muted-foreground">
              Loading...
            </div>
          ) : docLoadError ? (
            <div className="py-12 text-center text-muted-foreground space-y-3">
              <p>{docLoadError}</p>
              {activeDoc?.slug && (
                <Button variant="outline" onClick={() => loadDoc(activeDoc.slug)}>
                  Retry
                </Button>
              )}
            </div>
          ) : activeDoc ? (
            <div className={`transition-opacity duration-100 ${docSwitching ? "opacity-50" : "opacity-100"}`}>
              {/* Cross-reference banner */}
              {docForm && (
                <div className="flex items-center gap-3 mb-4 px-4 py-2.5 rounded-md bg-secondary/30 border border-border">
                  <Badge variant="outline">{docForm}</Badge>
                  <span className="text-sm text-muted-foreground">
                    You&apos;ve found{" "}
                    <span className="text-foreground font-semibold">
                      {formCount}
                    </span>{" "}
                    {docForm} primes
                  </span>
                  <Link
                    href={`/?form=${docForm}`}
                    className="text-sm text-primary hover:underline ml-auto"
                  >
                    View in dashboard &rarr;
                  </Link>
                </div>
              )}
              <article className="docs-prose">
                <ReactMarkdown
                  remarkPlugins={[remarkGfm, remarkMath]}
                  rehypePlugins={[rehypeKatex]}
                >
                  {renderedDocContent}
                </ReactMarkdown>
              </article>
            </div>
          ) : (
            <div className="py-12 text-center text-muted-foreground">
              {tabDocs.length === 0
                ? activeTab === "agent"
                  ? "No CLAUDE.md agent files found."
                  : activeTab === "roadmaps"
                    ? "No roadmap documents found."
                    : "No research documents found. Place .md files in the docs/ directory."
                : "Select a document from the sidebar."}
            </div>
          )}
        </main>
      </div>
    </div>
  );
}

export default function DocsPage() {
  return (
    <Suspense
      fallback={
        <div className="text-center text-muted-foreground py-12">
          Loading...
        </div>
      }
    >
      <DocsPageInner />
    </Suspense>
  );
}
