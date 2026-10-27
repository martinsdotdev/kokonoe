/**
 * The rules engine, compiled to WASM by `scripts/bindings.mjs` into `./engine/`. The app imports
 * it from here: the page and every Worker share this one module.
 */
import init, { type Seed } from './engine/kokonoe_wasm.js';

export * from './engine/kokonoe_wasm.js';

const url = new URL('./engine/kokonoe_wasm_bg.wasm', import.meta.url);

let compiling: Promise<WebAssembly.Module> | undefined;
let loading: Promise<void> | undefined;

/** Lets the next caller retry a failed load, after a dropped connection say. */
const retrying =
	(reset: () => void) =>
	(error: unknown): never => {
		reset();
		throw error;
	};

/**
 * The engine's compiled module, fetched and compiled once per realm. The page hands it to its
 * Workers, since a Worker's bundle names its own copy of the `.wasm`, which it would download and
 * compile a second time.
 */
export const compiledEngine = (): Promise<WebAssembly.Module> =>
	(compiling ??= WebAssembly.compileStreaming(fetch(url))
		.catch(async (error: unknown) => {
			// Streaming needs `application/wasm`. As wasm-bindgen's own loader does, compile from
			// the bytes when a host sends another type, and give up on any other failure.
			const response = await fetch(url);
			if (!response.ok) throw error;
			return WebAssembly.compile(await response.arrayBuffer());
		})
		.catch(retrying(() => (compiling = undefined))));

/**
 * Instantiates the engine, once per realm (the page, or a Worker): from `module` when the page
 * sent one, or else from the compiled `.wasm`.
 */
export const loadEngine = (module?: WebAssembly.Module): Promise<void> =>
	(loading ??= init({ module_or_path: module ?? compiledEngine() }).then(
		() => {},
		retrying(() => (loading = undefined))
	));

/** A fresh Seed for an offline game, from the browser's CSPRNG. */
export const newSeed = (): Seed =>
	Array.from(crypto.getRandomValues(new Uint8Array(8)), (byte) =>
		byte.toString(16).padStart(2, '0')
	).join('');
