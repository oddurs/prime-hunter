export interface DocNavItem {
  title: string;
  href: string;
}

export interface DocNavSection {
  title: string;
  items: DocNavItem[];
}

export const docsNav: DocNavSection[] = [
  {
    title: "Overview",
    items: [
      { title: "Documentation", href: "/docs" },
      { title: "Getting Started", href: "/docs/getting-started" },
    ],
  },
  {
    title: "Guides",
    items: [
      { title: "Architecture", href: "/docs/architecture" },
      { title: "Prime Forms", href: "/docs/prime-forms" },
    ],
  },
  {
    title: "Reference",
    items: [
      { title: "API Reference", href: "/docs/api" },
      { title: "Contributing", href: "/docs/contributing" },
    ],
  },
];
