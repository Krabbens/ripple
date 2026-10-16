# Ripple

Projekt rozwijamy w Rust i TypeScript. Rozmawiaj z użytkownikiem po polsku.

## Układ i narzędzia

- Rust: Cargo workspace, edition 2024, toolchain z `rust-toolchain.toml`.
- TypeScript: npm workspaces, ESM, wspólna konfiguracja `tsconfig.base.json` z `strict`.
- Crate'y umieszczaj w `crates/`, pakiety TypeScript w `packages/`.
- Wersjonuj `Cargo.lock` i `package-lock.json`. Nie mieszaj menedżerów pakietów.
- Preferuj standardowe biblioteki i małe zależności dobrane do konkretnej potrzeby.
- Nie projektuj dodatkowych warstw ani frameworków bez potrzeby wynikającej z zadania.

## Weryfikacja zmian

Dla Rust: `cargo fmt --all -- --check`,
`cargo clippy --workspace --all-targets --locked -- -D warnings`
i `cargo test --workspace --locked`.

Dla TypeScript: `npm run format:check`, `npm run typecheck` i `npm run build`.

Testy dodawaj dla istotnego zachowania i regresji. Sprawdzaj obie strony, gdy zmiana
dotyka współpracy Rust–TypeScript.
