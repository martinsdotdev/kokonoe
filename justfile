# Kokonoe's commands, from the repo root. `just` lists them, and `just ci` runs both of CI's jobs.

# Recipes run in sh, or in PowerShell on Windows, where cmd would resolve bash to WSL.
set windows-shell := ["pwsh", "-NoLogo", "-NoProfile", "-Command"]

# The server's database while serving locally: `just db` runs it. No test reads it; the
# server's database tests start their own.
export DATABASE_URL := env_var_or_default("DATABASE_URL", "postgres://postgres:postgres@localhost:5432/kokonoe")

[private]
default:
    @just --list

# Install the SPA's packages.
install:
    pnpm -C web install

# Serve the app.
dev:
    pnpm -C web dev

# Start a local Postgres for `just serve` in Docker, creating it the first time.
[script('node')]
db:
    const { spawnSync } = require('node:child_process');
    if (spawnSync('docker', ['start', 'kokonoe-db'], { stdio: 'ignore' }).status !== 0) {
        const run = ['run', '-d', '--name', 'kokonoe-db', '-p', '5432:5432'];
        const env = ['-e', 'POSTGRES_PASSWORD=postgres', '-e', 'POSTGRES_DB=kokonoe'];
        const created = spawnSync('docker', [...run, ...env, 'postgres:18-alpine'], { stdio: 'inherit' });
        process.exit(created.status ?? 1);
    }

# Build the SPA and serve it from the server, as production does, on port 8080.
serve: build
    cargo run -p kokonoe-server

# Write the static SPA to web/build.
build:
    pnpm -C web build

# Build the engine's WASM and write the browser bindings into web/src/lib/engine.
bindings:
    node scripts/bindings.mjs

# Format the Rust and the SPA.
fmt:
    cargo fmt --all
    pnpm -C web format

# Check formatting, lints and types without running tests.
lint:
    cargo fmt --all -- --check
    cargo clippy --workspace --all-targets --locked -- -D warnings
    pnpm -C web check
    pnpm -C web lint

# Run the Rust tests, then the SPA's (Vitest in Chromium, then Playwright).
test:
    cargo test --workspace --locked
    pnpm -C web test

# CI's Rust job. Its bindings check reads only web/src/lib/engine, so other work in progress passes.
rust: engine-free
    cargo fmt --all -- --check
    cargo clippy --workspace --all-targets --locked -- -D warnings
    cargo test --workspace --locked
    node scripts/bindings.mjs
    git diff --exit-code -- web/src/lib/engine

# CI's Web job, once `just install` has run.
web:
    pnpm -C web check
    pnpm -C web lint
    pnpm -C web test

# Both of CI's jobs.
ci: rust web

# Fail if the engine depends on wasm-bindgen: only kokonoe-wasm may know about JavaScript.
[script('node')]
engine-free:
    const { spawnSync } = require('node:child_process');
    // Normal and build dependencies on every target: what the server would link.
    const tree = spawnSync(
        'cargo',
        ['tree', '-p', 'kokonoe-engine', '-e', 'normal,build', '--target', 'all', '--prefix', 'none', '--locked'],
        { encoding: 'utf8' },
    );
    if (tree.status !== 0) {
        process.stderr.write(tree.stderr);
        process.exit(1);
    }
    if (tree.stdout.includes('wasm-bindgen')) {
        console.error('kokonoe-engine depends on wasm-bindgen. Only kokonoe-wasm may know about JavaScript.');
        process.exit(1);
    }

# Play adjacent Levels against each other, e.g. `just ladder --preset d3 --pairs 5-8`.
ladder *args:
    cargo run --release --example ladder -- {{args}}

# Print random playouts per second at both depths.
playouts *args:
    cargo run --release --example playouts -- {{args}}
