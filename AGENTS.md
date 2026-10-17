# Ripple

Projekt rozwijamy w Rust i TypeScript. Rozmawiaj z użytkownikiem po polsku.

## Kierunek produktu

- Ripple jest pluginem deweloperskim dla Astro, React/Vite i Next.js.
  Używamy DOM-u i HMR gospodarza; nie przejmuj renderowania ani stanu frameworka.
  Własny runtime JSON zachowujemy wyłącznie jako kompatybilny prototyp.
- W trybie deweloperskim elementy mają być zaznaczane, przeciągane między inne
  elementy oraz opisywane rysunkami i tekstem.
- Używamy bezpośrednio modeli z własnej subskrypcji ChatGPT przez OAuth.
  Nie uruchamiaj Codex CLI, SDK ani app-server jako agenta Ripple.
- Żądany model to `gpt-6-luna`, reasoning effort `low`. Nie zmieniaj modelu po cichu.
- OAuth i tokeny należą do lokalnego procesu Rust. Przeglądarka dostaje tylko
  podstawowe informacje o koncie i stan edytora.
- Element wskazuje źródło przez `data-ripple-source` lub konfigurację pluginu.
  `ripple.page.json` należy do starszego prototypu, nie jest wymagany przez plugin.
- Ripple ma wspierać fullstack: kod TypeScript, backend Rust, API, migracje i testy.
  `ripple.project.json` określa źródła projektu, lokalne originy i komendy kontrolne.
  Domyślnie pracujemy małymi krokami: do 3 plików i 1–2 odpowiedzi Luny, poprawki
  tekstu zamiast przepisywania całych plików. Zmiany w kopii roboczej zapisujemy po
  kontrolach. UI pokazuje rzeczywiste etapy pracy przez strumień SSE.
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
