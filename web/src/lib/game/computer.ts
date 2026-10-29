import {
	choose,
	compiledEngine,
	newSeed,
	shares,
	type Cell,
	type Record,
	type Visit
} from '#lib/engine.js';
import type { Answer, Ask } from './computer.worker.js';

/** The computer opponent: its move for the game a Record leads to, at a Level (spec §6). */
export interface Computer {
	/** One request at a time. Rejects when `signal` aborts, and never answers after. */
	think(record: Record, level: number, signal: AbortSignal): Promise<Cell>;
	/** Stops any search and frees what the computer holds. */
	dispose(): void;
}

export interface Options {
	/** How many Workers Levels 5 to 8 split across, instead of what the device's cores allow. */
	workers?: number;
	/**
	 * The page's clock, read when it asks: `Date.now` unless a test brings its own. A time in the
	 * future keeps the Level's cap from binding, so only the iteration budget does.
	 */
	now?: () => number;
}

/**
 * The computer in Web Workers, so the page stays responsive while it searches (spec §6). Below
 * Level 5 one Worker searches the whole budget; from Level 5 up to four share it, root-parallel,
 * each with its own engine and tree and no memory shared, and the page sums their visits and
 * chooses once. Every share stops at the Level's cap counted from the moment the page asked.
 *
 * A search is one synchronous WASM call that nothing can interrupt, so an abort terminates every
 * Worker, and the next request starts new ones. Each request carries the page's compiled engine,
 * so a Worker only instantiates it. Nothing starts a Worker before the first request, so pass
 * and play never does.
 */
export class WorkerComputer implements Computer {
	#workers: Worker[] = [];
	#cores: number;
	#now: () => number;
	/** Each request's id, so an answer to an earlier one is never taken for this one's. */
	#asks = 0;
	/** The iterations behind the last move it chose, summed over its Workers: 0 for a forced one. */
	searched = 0;

	constructor({ workers, now = Date.now }: Options = {}) {
		// `shares` gives min(4, cores − 1) Workers from Level 5, so n Workers take n + 1 cores.
		this.#cores = workers ? workers + 1 : navigator.hardwareConcurrency || 1;
		this.#now = now;
	}

	async think(record: Record, level: number, signal: AbortSignal): Promise<Cell> {
		const engine = await compiledEngine();
		signal.throwIfAborted();
		const asked = this.#now();
		const id = ++this.#asks;
		const iterations = [...shares(level, this.#cores)];
		while (this.#workers.length < iterations.length) {
			this.#workers.push(
				new Worker(new URL('./computer.worker.ts', import.meta.url), { type: 'module' })
			);
		}
		const workers = this.#workers.slice(0, iterations.length);
		// Every listener goes when the request settles.
		const listening = new AbortController();
		const on = { signal: listening.signal };
		try {
			const found = await new Promise<Visit[][] | Cell>((resolve, reject) => {
				const stop = (error: unknown) => {
					this.#stop();
					reject(error);
				};
				const searches: Visit[][] = [];
				for (const [i, worker] of workers.entries()) {
					worker.addEventListener(
						'message',
						({ data }: MessageEvent<Answer>) => {
							if (data.id !== id) return;
							if ('error' in data) return stop(new Error(data.error));
							// Every Worker finds the same forced move, so the first decides.
							if ('Forced' in data.searched) return resolve(data.searched.Forced);
							searches.push(data.searched.Visits);
							if (searches.length === workers.length) resolve(searches);
						},
						on
					);
					worker.addEventListener(
						'messageerror',
						() => stop(new Error('The computer’s answer couldn’t be read.')),
						on
					);
					worker.addEventListener(
						'error',
						(event) => stop(new Error(event.message || 'The computer’s Worker failed to start.')),
						on
					);
					const ask: Ask = {
						id,
						record,
						level,
						iterations: iterations[i],
						seed: newSeed(),
						asked,
						engine
					};
					worker.postMessage(ask);
				}
				signal.addEventListener('abort', () => stop(signal.reason), on);
			});
			if (typeof found === 'number') {
				this.searched = 0;
				return found;
			}
			this.searched = found.flat().reduce((total, visit) => total + visit.visits, 0);
			return choose(level, record.ruleset.depth, found.flat(), newSeed());
		} finally {
			listening.abort();
		}
	}

	dispose() {
		this.#stop();
	}

	#stop() {
		for (const worker of this.#workers) worker.terminate();
		this.#workers = [];
	}
}
