// Offline play from the home screen: every same-origin GET goes to the network first (so a new
// build is picked up at once) and falls back to the last copy cached here.
const CACHE = "tama";

self.addEventListener("install", () => self.skipWaiting());
// A tap on a care-call notification brings the game forward.
self.addEventListener("notificationclick", e => {
  e.notification.close();
  e.waitUntil((async () => {
    const [win] = await clients.matchAll({ type: "window", includeUncontrolled: true });
    return win ? win.focus() : clients.openWindow("./");
  })());
});
self.addEventListener("activate", e => e.waitUntil(self.clients.claim()));
self.addEventListener("fetch", e => {
  const req = e.request;
  if (req.method !== "GET" || new URL(req.url).origin !== location.origin) return;
  e.respondWith((async () => {
    const cache = await caches.open(CACHE);
    try {
      // Revalidate past the HTTP cache (GitHub Pages sends max-age=600).
      const res = await fetch(req.url, { cache: "no-cache" });
      if (res.ok) cache.put(req, res.clone());
      return res;
    } catch (err) {
      const hit = await cache.match(req, { ignoreSearch: true });
      if (hit) return hit;
      throw err;
    }
  })());
});
