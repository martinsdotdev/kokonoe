// For engine.browser.spec.ts: opens the Record it is sent and answers with the game's Status.
import { Game, loadEngine, type Record } from './engine.js';

self.addEventListener('message', async (event: MessageEvent<Record>) => {
	await loadEngine();
	const game = Game.open(event.data);
	self.postMessage(game instanceof Game ? game.status() : game);
});
