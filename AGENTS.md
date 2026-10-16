# Ripple

Projekt rozwijamy w Rust i TypeScript. Rozmawiaj z użytkownikiem po polsku.

## Kierunek produktu

- Ripple ma własny runtime stron i osadzony harness deweloperski.
- W trybie deweloperskim elementy mają być zaznaczane, przeciągane między inne
  elementy oraz opisywane rysunkami i tekstem.
- Używamy bezpośrednio modeli z własnej subskrypcji ChatGPT przez OAuth.
  Nie uruchamiaj Codex CLI, SDK ani app-server jako agenta Ripple.
- Żądany model to `gpt-6-luna`, reasoning effort `low`. Nie zmieniaj modelu po cichu.
- OAuth i tokeny należą do lokalnego procesu Rust. Przeglądarka dostaje tylko
  podstawowe informacje o koncie i stan edytora.
- Źródłem strony w prototypie jest `ripple.page.json`; elementy mają stałe ID.
- Ripple ma wspierać fullstack: kod TypeScript, backend Rust, API, migracje i testy.
  `ripple.project.json` określa źródła projektu i komendy kontrolne. Wykonawca Luny
  edytuje je w kopii roboczej, sprawdza i dopiero wtedy zapisuje. Nie ograniczaj
  przyszłych funkcji do samego dokumentu strony.
- Logowanie dewelopera przez ChatGPT służy dostępowi do modelu; logowanie i sesje
  użytkowników budowanej aplikacji wymagają osobnej implementacji.

## Układ i narzędzia

- Przy tworzeniu UI stosuj [.agents/skills/minimalist-ui/SKILL.md](.agents/skills/minimalist-ui/SKILL.md)
  z `Leonxlnx/taste-skill`; wariant jest przeznaczony do interfejsów aplikacji.

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
