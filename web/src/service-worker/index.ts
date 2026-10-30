// Loads the app with no connection (spec §5.1). Installing keeps the build, but its fonts, and the
// fallback page every route boots from; a font is kept once the browser uses it, since most are
// alphabets the app never shows. A new build takes over as soon as it has installed, so the next
// reload is the new build.
import { immutable, assets } from '$app/manifest';
import { version } from '$app/env';
import { self } from '$app/service-worker';

const CACHE = `kokonoe-${version}`;
const FONT = /\.woff2?$/;
/** The app's root, where the host answers every route with the SPA's fallback page. */
const SHELL = self.registration.scope;
/** How long a navigation waits for a network that's connected but stuck before the kept page. */
const STALLED_MS = 4_000;

const path = (file: { path: string }) => new URL(file.path, SHELL).pathname;
const kept = new Set([...immutable, ...assets].map(path).filter((file) => !FONT.test(file)));

// While developing, `immutable` is empty and nothing is kept or intercepted.
const built = immutable.length > 0;

self.addEventListener('install', (event) => {
	event.waitUntil(
		(async () => {
			if (built) {
				const cache = await caches.open(CACHE);
				await cache.addAll([...kept]);
				const shell = await fetch(SHELL, { cache: 'no-cache' });
				const page = await shell.text();
				// A fallback page from another deploy would boot files this cache doesn't hold, so
				// the install fails and the browser tries again later.
				if (!shell.ok || !page.includes(version)) throw new Error(`no shell for ${version}`);
				// Only the type: the text is already decoded, so an encoding header would be wrong.
				const type = shell.headers.get('content-type') ?? 'text/html; charset=utf-8';
				await cache.put(SHELL, new Response(page, { headers: { 'content-type': type } }));
			}
			await self.skipWaiting();
		})()
	);
});

self.addEventListener('activate', (event) => {
	event.waitUntil(
		(async () => {
			for (const key of await caches.keys()) {
				if (key.startsWith('kokonoe-') && key !== CACHE) await caches.delete(key);
			}
			await self.clients.claim();
		})()
	);
});

self.addEventListener('fetch', (event) => {
	const { request } = event;
	const url = new URL(request.url);
	if (!built || request.method !== 'GET' || url.origin !== self.location.origin) return;
	if (request.mode === 'navigate') {
		event.respondWith(fromNetworkOrShell(request));
	} else if (kept.has(url.pathname)) {
		event.respondWith(fromCache(request));
	} else if (FONT.test(url.pathname)) {
		event.respondWith(fromCacheKeepingIt(request));
	}
});

/** A page from the network, or the kept fallback page when there's none or it's stuck. */
async function fromNetworkOrShell(request: Request): Promise<Response> {
	const network = fetch(request);
	network.catch(() => {}); // the kept page answers instead
	const shell = async () => (await caches.match(SHELL, { cacheName: CACHE })) ?? network;
	const stalled = new Promise((resolve) => setTimeout(resolve, STALLED_MS)).then(shell);
	return Promise.race([network, stalled]).catch(shell);
}

async function fromCache(request: Request): Promise<Response> {
	return (await caches.match(request, { cacheName: CACHE })) ?? fetch(request);
}

async function fromCacheKeepingIt(request: Request): Promise<Response> {
	const cache = await caches.open(CACHE);
	const hit = await cache.match(request);
	if (hit) return hit;
	const response = await fetch(request);
	// Shown either way: a full cache only means it's fetched again next time.
	if (response.ok) await cache.put(request, response.clone()).catch(() => {});
	return response;
}
