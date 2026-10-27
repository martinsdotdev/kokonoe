import { compiledEngine, newSeed, type Cell, type Record } from '#lib/engine.js';
import type { Answer, Ask } from './computer.worker.js';

/** The computer opponent: its move for the game a Record leads to, at a Level (spec §6). */
export interface Computer {
	/** One request at a time. Rejects when `signal` aborts, and never answers after. */
	think(record: Record, level: number, signal: AbortSignal): Promise<Cell>;
	/** Stops any search and frees what the computer holds. */
	dispose(): void;
}

/**
 * The computer in a Web Worker, so the page stays responsive while it searches. The search is one
 * synchronous WASM call that nothing can interrupt, so an abort terminates the Worker, and the
 * next request starts a new one. Each request carries the page's compiled engine, so a Worker
 * only instantiates it. Nothing starts a Worker before the first request, so pass and play never
 * does.
 */
export class WorkerComputer implements Computer {
	#worker: Worker | null = null;

	async think(record: Record, level: number, signal: AbortSignal): Promise<Cell> {
		const engine = await compiledEngine();
		return new Promise((resolve, reject) => {
			if (signal.aborted) return reject(signal.reason);
			const worker = (this.#worker ??= new Worker(
				new URL('./computer.worker.ts', import.meta.url),
				{ type: 'module' }
			));
			const settle = () => {
				worker.removeEventListener('message', onmessage);
				worker.removeEventListener('error', onerror);
				signal.removeEventListener('abort', onabort);
			};
			const onmessage = ({ data }: MessageEvent<Answer>) => {
				settle();
				if ('cell' in data) resolve(data.cell);
				else reject(new Error(data.error));
			};
			const onerror = (event: ErrorEvent) => {
				settle();
				this.#stop();
				reject(new Error(event.message || 'The computer’s Worker failed to start.'));
			};
			const onabort = () => {
				settle();
				this.#stop();
				reject(signal.reason);
			};
			worker.addEventListener('message', onmessage);
			worker.addEventListener('error', onerror);
			signal.addEventListener('abort', onabort);
			worker.postMessage({ record, level, seed: newSeed(), engine } satisfies Ask);
		});
	}

	dispose() {
		this.#stop();
	}

	#stop() {
		this.#worker?.terminate();
		this.#worker = null;
	}
}
