import {
	choose,
	compiledEngine,
	newSeed,
	shares,
	type Cell,
	type Record,
	type Searched
} from '#lib/engine.js';
import type { Answer, Ask } from './computer.worker.js';

/** The computer opponent: its move for the game a Record leads to, at a Level (spec §6). */
export interface Computer {
	/** One request at a time. Rejects when `signal` aborts, and never answers after. */
	think(record: Record, level: number, signal: AbortSignal): Promise<Cell>;
	/** Stops any search and frees what the computer holds. The next request starts afresh. */
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

/** One Worker's answer to `ask`, rejecting on its error, or when the answer can't be read. */
const answer = (worker: Worker, ask: Ask, signal: AbortSignal) =>
	new Promise<Searched>((resolve, reject) => {
		const on = { signal };
		worker.addEventListener(
			'message',
			({ data }: MessageEvent<Answer>) => {
				if (data.id !== ask.id) return;
				if ('error' in data) reject(new Error(data.error));
				else resolve(data.searched);
			},
			on
		);
		worker.addEventListener(
			'messageerror',
			() => reject(new Error('The computer’s answer couldn’t be read.')),
			on
		);
		worker.postMessage(ask);
	});

/**
 * The computer in Web Workers, so the page stays responsive while it searches (spec §6). Below
 * Level 5 one Worker searches the whole budget; from Level 5 up to four share it, root-parallel,
 * each with its own engine and tree and no memory shared, and the page sums their visits and
 * chooses once. Every share stops at the Level's cap counted from the moment the page asked.
 *
 * A search is one synchronous WASM call that nothing can interrupt, so an abort or any failure
 * terminates every Worker, and the next request starts new ones. Each request carries the page's
 * compiled engine, so a Worker only instantiates it. Nothing starts a Worker before the first
 * request, so pass and play never does.
 */
export class WorkerComputer implements Computer {
	#workers: Worker[] = [];
	#cores: number;
	#now: () => number;
	/** Each request's id, so an answer to an earlier one is never taken for this one's. */
	#asks = 0;
	#iterations = 0;
	/** Ends the request in flight, when a Worker fails. */
	#interrupt: ((error: unknown) => void) | null = null;

	constructor({ workers, now = Date.now }: Options = {}) {
		// `shares` gives min(4, cores − 1) Workers from Level 5, so n Workers take n + 1 cores.
		this.#cores = workers ? workers + 1 : navigator.hardwareConcurrency || 1;
		this.#now = now;
	}

	/** The iterations behind the last move it chose, over all its Workers: 0 for a forced one. */
	get iterations(): number {
		return this.#iterations;
	}

	async think(record: Record, level: number, signal: AbortSignal): Promise<Cell> {
		const engine = await compiledEngine();
		signal.throwIfAborted();
		const startedAt = this.#now();
		const id = ++this.#asks;
		const iterations = [...shares(level, this.#cores)];
		const workers = this.#pool(iterations.length);
		// Every listener goes when the request settles.
		const listening = new AbortController();
		const interrupted = new Promise<never>((_, reject) => (this.#interrupt = reject));
		signal.addEventListener('abort', () => this.#interrupt?.(signal.reason), {
			signal: listening.signal
		});
		try {
			const asks = workers.map((worker, i) => {
				const ask = { id, record, level, iterations: iterations[i], seed: newSeed(), startedAt };
				return answer(worker, { ...ask, engine }, listening.signal);
			});
			const searches = await Promise.race([Promise.all(asks), interrupted]);
			// Every Worker finds the same forced move.
			const forced = searches.find((searched) => 'Forced' in searched);
			if (forced && 'Forced' in forced) {
				this.#iterations = 0;
				return forced.Forced;
			}
			const visits = searches.flatMap((searched) => ('Visits' in searched ? searched.Visits : []));
			this.#iterations = visits.reduce((total, visit) => total + visit.count, 0);
			return choose(level, record.ruleset.depth, visits, newSeed());
		} catch (error) {
			this.#stop();
			throw error;
		} finally {
			this.#interrupt = null;
			listening.abort();
		}
	}

	dispose() {
		this.#stop();
	}

	/** Exactly `count` Workers: the ones it holds, more started, or the surplus terminated. */
	#pool(count: number): Worker[] {
		for (const surplus of this.#workers.splice(count)) surplus.terminate();
		while (this.#workers.length < count) {
			const worker = new Worker(new URL('./computer.worker.ts', import.meta.url), {
				type: 'module'
			});
			// A Worker that fails, even between moves, takes the pool with it, so no request waits
			// on it. Handled here, so the page doesn't report it as an uncaught error too.
			worker.addEventListener('error', (event) => {
				event.preventDefault();
				this.#stop();
				this.#interrupt?.(new Error(event.message || 'The computer’s Worker failed.'));
			});
			this.#workers.push(worker);
		}
		return [...this.#workers];
	}

	#stop() {
		for (const worker of this.#workers) worker.terminate();
		this.#workers = [];
	}
}
