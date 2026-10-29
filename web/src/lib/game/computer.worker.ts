// One of the computer's Workers: searches its share of the Level for the game it is sent, and
// answers with what it found, or with the error, so a failure reaches the page instead of
// vanishing as an unhandled rejection.
import { loadEngine, search, type Record, type Searched, type Seed } from '#lib/engine.js';

/** The page's question: this Worker's share of the search for the game `record` leads to. */
export interface Ask {
	/** The request it belongs to, which the answer carries back. */
	id: number;
	record: Record;
	level: number;
	iterations: number;
	seed: Seed;
	/** When the page asked, its `Date.now()`: the Level's time cap counts from then. */
	startedAt: number;
	/** The page's compiled engine, so the Worker neither downloads nor compiles its own. */
	engine: WebAssembly.Module;
}

export type Answer = { id: number } & ({ searched: Searched } | { error: string });

self.addEventListener('message', async (event: MessageEvent<Ask>) => {
	const { id, record, level, iterations, seed, startedAt, engine } = event.data;
	let answer: Answer;
	try {
		await loadEngine(engine);
		answer = { id, searched: search(record, level, iterations, seed, startedAt) };
	} catch (error) {
		answer = { id, error: String(error) };
	}
	self.postMessage(answer);
});
