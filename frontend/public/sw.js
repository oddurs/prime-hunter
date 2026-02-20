/**
 * Darkreach Dashboard Service Worker
 *
 * Provides offline caching for the static Next.js export:
 * - Install: pre-cache the app shell (page routes, icons, manifest)
 * - Activate: clean old caches, claim clients immediately
 * - Fetch: cache-first for static assets, network-first for API, skip WebSocket
 */

const CACHE_NAME = "darkreach-v1";

const SHELL_URLS = [
  "/",
  "/browse",
  "/searches",
  "/performance",
  "/agents",
  "/fleet",
  "/docs",
  "/prime",
  "/manifest.json",
  "/icon-192.png",
  "/icon-512.png",
];

self.addEventListener("install", (event) => {
  event.waitUntil(
    caches.open(CACHE_NAME).then((cache) => cache.addAll(SHELL_URLS))
  );
  self.skipWaiting();
});

self.addEventListener("activate", (event) => {
  event.waitUntil(
    caches.keys().then((keys) =>
      Promise.all(
        keys
          .filter((key) => key !== CACHE_NAME)
          .map((key) => caches.delete(key))
      )
    )
  );
  self.clients.claim();
});

self.addEventListener("fetch", (event) => {
  const { request } = event;
  const url = new URL(request.url);

  // Skip WebSocket and non-GET requests
  if (request.method !== "GET") return;
  if (url.protocol === "ws:" || url.protocol === "wss:") return;

  // Network-first for API calls
  if (url.pathname.startsWith("/api/")) {
    event.respondWith(
      fetch(request)
        .then((response) => {
          const clone = response.clone();
          caches.open(CACHE_NAME).then((cache) => cache.put(request, clone));
          return response;
        })
        .catch(() => caches.match(request))
    );
    return;
  }

  // Cache-first for static assets
  event.respondWith(
    caches.match(request).then((cached) => {
      if (cached) return cached;
      return fetch(request).then((response) => {
        // Only cache same-origin successful responses
        if (response.ok && url.origin === self.location.origin) {
          const clone = response.clone();
          caches.open(CACHE_NAME).then((cache) => cache.put(request, clone));
        }
        return response;
      });
    })
  );
});
