/**
 * The rules engine, compiled to WASM by `scripts/bindings.sh` into `./engine/`. The app imports
 * it from here: the page and every Worker share this one module.
 */
import init, { type Seed } from './engine/kokonoe_wasm.js';

export * from './engine/kokonoe_wasm.js';

let loading: Promise<void> | undefined;

/** Fetches and instantiates the module, once per realm (the page, or a Worker). */
export const loadEngine = (): Promise<void> =>
	(loading ??= init().then(
		() => {},
		(error: unknown) => {
			// Let the next caller retry, after a dropped connection say.
			loading = undefined;
			throw error;
		}
	));

/** A fresh Seed for an offline game, from the browser's CSPRNG. */
export const newSeed = (): Seed =>
	Array.from(crypto.getRandomValues(new Uint8Array(8)), (byte) =>
		byte.toString(16).padStart(2, '0')
	).join('');
