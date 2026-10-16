# Ripple

Framework w Rust i TypeScript z własnym runtime stron i osadzonym trybem
deweloperskim. Zaznaczasz element, przeciągasz go albo rysujesz uwagę, a GPT-6 Luna
przekłada ten kontekst na zmianę strony.

## Uruchomienie prototypu

Potrzebne są Rust stable przez `rustup` oraz Node.js 24+ z npm.

```sh
npm ci
npm run dev
```

Otwórz `http://127.0.0.1:4310`. Strona działa przed logowaniem. W panelu:

1. Wybierz **Zaznacz** i kliknij element. Przycisk **↑** zaznacza jego rodzica.
2. Przeciągnij element przed lub za inny; znacznik pokazuje miejsce wstawienia.
3. Wybierz **Rysuj**, aby dodać uwagi. **Cofnij gest** cofa lokalny podgląd.
4. Użyj **Continue with ChatGPT**, aby połączyć własną subskrypcję.
5. Dopisz instrukcję i wybierz **Zastosuj zmiany**.

Model otrzymuje dokument, podgląd układu, stałe ID elementów, gesty i ewentualny
schemat PNG z rysunkami. Po zakończonej odpowiedzi Rust sprawdza dokument i zapisuje
`ripple.page.json`; runtime wyświetla nową wersję. **Wyczyść** odrzuca niezapisany
podgląd. Panel można zwinąć przyciskiem **Zwiń panel**; na telefonie startuje zwinięty.

```sh
# Inny port lub plik strony
npm run dev -- --port 4311 --page moja-strona.page.json
```

## Model i logowanie

Ripple używa własnego OAuth i żądań HTTP, z domyślnym modelem `gpt-6-luna`
i `reasoning.effort: low`. Nie wymaga instalacji Codexa ani klucza API.

Pierwsze logowanie korzysta z dynamicznej rejestracji klienta **Ripple**, PKCE
i lokalnego callbacku na `127.0.0.1`. Host ma zapisany raz identyfikator UUIDv4
w formacie `urn:uuid:…`. Starsze identyfikatory z prototypu są migrowane bez usuwania
kont i tokenów. Tokeny pozostają w procesie Rust i chronionym
pliku `~/.config/ripple/accounts.json`. `XDG_CONFIG_HOME` lub `RIPPLE_HOME` zmieniają
lokalizację. Tylko jeden proces może jednocześnie korzystać z danego magazynu kont.

Aktualny katalog modeli jest odczytywany dla zalogowanego konta. Jeśli konto nie
udostępnia Luny, żądanie kończy się błędem; Ripple nie podstawia innego modelu.

Integracja stosuje opisany przez OpenAI przepływ
[ChatGPT plan usage](https://developers.openai.com/siwc/token-sharing-open-source/sign-in)
oraz [bezpośredni Responses API](https://developers.openai.com/siwc/token-sharing-open-source/models-and-inference):
`store: false`, `stream: true`, pełny kontekst w każdym żądaniu.

## Struktura

UI edytora stosuje wariant `minimalist-ui` z [taste-skill](https://github.com/Leonxlnx/taste-skill).
Przypięta kopia i licencja są w `.agents/skills/minimalist-ui/`.

- `packages/core/` — format dokumentu, operacje na drzewie i renderer DOM.
- `packages/harness/` — osadzony panel w Shadow DOM, zaznaczanie, przenoszenie
  i rysowanie. Montowany jawnie tylko w punkcie wejścia deweloperskiego.
- `crates/ripple-core/` — OAuth, magazyn kont, walidacja dokumentu i transport modeli.
- `crates/ripple-dev/` — lokalny serwer deweloperski.
- `examples/playground/` — punkt wejścia przykładowej strony.
- `ripple.page.json` — przykładowa strona i źródło zmian prototypu.

## Zakres pierwszej wersji

Strony są drzewami dokumentu Ripple ze stylami inline. Ten prototyp obsługuje
podstawowe elementy HTML; nie generuje jeszcze dowolnej logiki TypeScript ani
komponentów React. Rysunki trafiają do modelu jako wektory oraz schemat PNG
z granicami elementów, a nie zrzut wyglądu strony.

Przenoszenie i cofanie działają na lokalnym podglądzie. Zapis wymaga zakończonej
odpowiedzi modelu i poprawnego dokumentu. Zmieniona w międzyczasie wersja pliku
powoduje odrzucenie odpowiedzi. Prototyp ma limit 500 elementów i 32 poziomów drzewa.
Serwer słucha wyłącznie na `127.0.0.1`.

Testy nie logują się na prawdziwe konto ani nie zużywają subskrypcji. Pełne
potwierdzenie dostępu konta do Luny wymaga udanej, rzeczywistej odpowiedzi po OAuth.

## Weryfikacja

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
npm run format:check
npm run typecheck
npm test
```

Formatowanie: `cargo fmt --all` oraz `npm run format`.
GitHub Actions wykonuje te kontrole. Lockfile'y są wersjonowane.
