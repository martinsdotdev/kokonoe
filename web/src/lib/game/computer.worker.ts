// The computer's Worker: searches the game it is sent and answers with its move, or with the
// error, so a failure reaches the page instead of vanishing as an unhandled rejection.
import { loadEngine, think, type Cell, type Record, type Seed } from '#lib/engine.js';

/** The page's question: the move for the game `record` leads to. */
export interface Ask {
	record: Record;
	level: number;
	seed: Seed;
	/** The page's compiled engine, so the Worker neither downloads nor compiles its own. */
	engine: WebAssembly.Module;
}

export type Answer = { cell: Cell } | { error: string };

self.addEventListener('message', async (event: MessageEvent<Ask>) => {
	const { record, level, seed, engine } = event.data;
	let answer: Answer;
	try {
		await loadEngine(engine);
		answer = { cell: think(record, level, seed) };
	} catch (error) {
		answer = { error: String(error) };
	}
	self.postMessage(answer);
});
