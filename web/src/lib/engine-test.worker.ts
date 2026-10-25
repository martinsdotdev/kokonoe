// For engine.browser.spec.ts: opens the Record it is sent and answers with the game's Status, or
// with the error, so a failure reaches the test instead of vanishing as an unhandled rejection.
import { Game, loadEngine, type Record } from './engine.js';

self.addEventListener('message', async (event: MessageEvent<Record>) => {
	try {
		await loadEngine();
		const game = Game.open(event.data);
		self.postMessage(game instanceof Game ? game.status() : game);
	} catch (error) {
		self.postMessage({ error: String(error) });
	}
});
