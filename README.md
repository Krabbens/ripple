# Ripple

Projekt w Rust i TypeScript.

## Struktura

- `crates/ripple-core/` — początkowa biblioteka Rust w Cargo workspace.
- `packages/core/` — początkowy pakiet TypeScript w npm workspaces.

Oba moduły są puste i gotowe do implementacji.

## Przygotowanie

Potrzebne są Rust stable przez `rustup` oraz Node.js 24+ z npm.
Plik `.nvmrc` wskazuje wersję Node.js używaną w CI.

```sh
npm ci
rustup show
```

## Rust

```sh
cargo build --workspace --locked
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
```

Formatowanie: `cargo fmt --all`.

## TypeScript

```sh
npm run typecheck
npm run build
npm run format:check
```

Formatowanie: `npm run format`.

TypeScript używa ESM i trybu `strict`. Nowe pakiety dodawaj w `packages/`,
a nowe crate'y w `crates/`. Pliki `Cargo.lock` i `package-lock.json` są wersjonowane.

GitHub Actions sprawdza formatowanie, Clippy, testy Rust oraz typy i build TypeScript.
