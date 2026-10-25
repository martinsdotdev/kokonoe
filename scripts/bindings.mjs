// Regenerates the engine's browser bindings: kokonoe-wasm built for wasm32 with the `wasm`
// profile, then wasm-bindgen writing the JS glue, the TypeScript and the .wasm into
// web/src/lib/engine. CI fails if the checked-in glue or types differ afterwards. The SPA's dev,
// build and test scripts run it first, because the .wasm isn't checked in and must match the
// glue. Node rather than a shell script, so it runs the same from cmd, PowerShell and bash.
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const root = fileURLToPath(new URL('..', import.meta.url));
const target = 'wasm32-unknown-unknown';
const run = (command, args) => execFileSync(command, args, { cwd: root, stdio: 'inherit' });
const read = (command, args) =>
	execFileSync(command, args, { cwd: root, encoding: 'utf8', stdio: ['ignore', 'pipe', 'ignore'] });

const tree = read('cargo', [
	'tree',
	...['-p', 'kokonoe-wasm', '-e', 'normal', '--target', target, '--prefix', 'none', '--locked']
]);
const version = tree.match(/^wasm-bindgen v(\S+)/m)?.[1];
if (!version) throw new Error('kokonoe-wasm no longer depends on wasm-bindgen');

run('cargo', ['build', '-p', 'kokonoe-wasm', '--target', target, '--profile', 'wasm', '--locked']);

// The CLI must match the crate's wasm-bindgen exactly, or it refuses the module.
let installed;
try {
	installed = read('wasm-bindgen', ['--version']).trim();
} catch {
	installed = undefined;
}
if (installed !== `wasm-bindgen ${version}`) {
	console.log(`Installing wasm-bindgen-cli ${version} to match kokonoe-wasm.`);
	run('cargo', ['install', 'wasm-bindgen-cli', '--version', version, '--locked']);
}

// The name section only helps debugging, and it was a fifth of the module's brotli size.
run('wasm-bindgen', [
	...['--target', 'web', '--remove-name-section', '--remove-producers-section'],
	...['--out-dir', 'web/src/lib/engine', `target/${target}/wasm/kokonoe_wasm.wasm`]
]);
console.log(`Wrote the bindings for wasm-bindgen ${version} to web/src/lib/engine.`);
