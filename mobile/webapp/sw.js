// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
// Service worker: PRIMA LA RETE per l'interfaccia (gli aggiornamenti si vedono
// subito), copia in cache solo come riserva. Le API e il WebSocket non passano
// mai dalla cache.
const CACHE = "nyctalus-v8";
const GUSCIO = ["/", "/index.html", "/app.js", "/stile.css", "/logo.svg", "/sfondo-scuro.svg",
                "/sfondo-chiaro.svg", "/manifest.webmanifest", "/icone/icona-192.png"];
self.addEventListener("install", (e) => { e.waitUntil(caches.open(CACHE).then((c) => c.addAll(GUSCIO)).catch(() => {})); self.skipWaiting(); });
self.addEventListener("activate", (e) => {
  e.waitUntil(caches.keys().then((k) => Promise.all(k.filter((x) => x !== CACHE).map((x) => caches.delete(x)))));
  self.clients.claim();
});
self.addEventListener("fetch", (e) => {
  const u = new URL(e.request.url);
  if (e.request.method !== "GET" || u.pathname.startsWith("/api/") || u.pathname === "/ws") return;
  e.respondWith(fetch(e.request).then((r) => {
    const copia = r.clone(); caches.open(CACHE).then((c) => c.put(e.request, copia)); return r;
  }).catch(() => caches.match(e.request)));
});
