import { defineConfig } from '@playwright/test';

export default defineConfig({
	webServer: {
		command: 'pnpm build && pnpm exec sirv build --single 200.html --port 4173 --quiet',
		port: 4173
	},
	// The service worker would hide requests from the tests that count or hold them; the offline
	// tests turn it on.
	use: { baseURL: 'http://localhost:4173', serviceWorkers: 'block' },
	testMatch: '**/*.e2e.{ts,js}'
});
