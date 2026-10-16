# Ripple

Framework w Rust i TypeScript z własnym runtime stron i osadzonym trybem
deweloperskim. Zaznaczasz element, przeciągasz go albo rysujesz uwagę, a GPT-6 Luna
przekłada ten kontekst na zmianę strony i kodu projektu.

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
schemat PNG z rysunkami. W trybie projektu Luna korzysta też z lokalnych narzędzi
do czytania i edycji źródeł oraz uruchamiania kontroli. Rust zapisuje sprawdzone
zmiany, a runtime wyświetla nowy dokument. **Wyczyść** odrzuca niezapisany
podgląd. Panel można zwinąć przyciskiem **Zwiń panel**; na telefonie startuje zwinięty.

```sh
# Inny port lub plik strony
npm run dev -- --port 4311 --page moja-strona.page.json

# Inny projekt
npm run dev -- --project /sciezka/aplikacja/ripple.project.json
```

## Praca fullstack

Ripple automatycznie używa `ripple.project.json` z bieżącego katalogu. Bez manifestu
lub przy jawnym `--page` działa prostsza edycja samego dokumentu strony. Dołączony
manifest pozwala rozwijać samo repozytorium frameworka: `crates/`, `packages/`,
`examples/` i `migrations/` oraz pliki Cargo/npm. Własna aplikacja powinna mieć swój
manifest w swoim katalogu głównym, np.:

```json
{
  "version": 1,
  "page": "ripple.page.json",
  "sources": [
    "ripple.page.json",
    "server",
    "web",
    "migrations",
    "Cargo.toml",
    "Cargo.lock",
    "package.json",
    "package-lock.json",
    "tsconfig.json"
  ],
  "checks": [
    {
      "name": "backend",
      "command": ["cargo", "test", "--locked"],
      "timeoutSeconds": 120
    },
    {
      "name": "frontend",
      "command": ["npm", "run", "typecheck"],
      "timeoutSeconds": 120
    }
  ],
  "instructions": "Backend Rust, frontend TypeScript. Zachowaj istniejące kontrakty API."
}
```

Ścieżki odnoszą się do katalogu manifestu. `sources` zawiera pliki albo katalogi;
może obejmować również katalog przyszłych migracji. Projekt musi zawierać wszystkie
źródła i konfigurację potrzebne do jego sprawdzenia. Zależności trzeba wcześniej
zainstalować. Na macOS/Linux wykonawca wykorzystuje główne `node_modules`, przekierowuje
linki paczek workspace do kopii roboczej i współdzieli cache Cargo w `target/`.

Luna ma narzędzia `list_files`, `read_file`, `write_file`, `delete_file` i `run_check`
w namespace `workspace`. Może więc przejść od zaznaczonego formularza do handlera
API, kodu klienta i pliku migracji SQL. Kontrola zwraca kod wyjścia i ograniczone
logi; model może poprawić błąd i sprawdzić kod ponownie. To własna pętla narzędzi
Ripple, korzystająca z [function calling](https://developers.openai.com/api/docs/guides/function-calling)
w formie obsługiwanej przez [OAuth subskrypcji](https://developers.openai.com/siwc/token-sharing-open-source/preview-limitations).

Edycje trafiają do katalogu tymczasowego. Zapis do projektu następuje po zakończonej
odpowiedzi i przejściu wszystkich kontroli na dokładnie tej wersji źródeł. Zmiana
dowolnego źródła lub manifestu w trakcie pracy odrzuca zapis. Błąd odpowiedzi,
przekroczenie limitu lub nieudane kontrole pozostawiają oryginalne źródła bez zmian.
Panel pokazuje podsumowanie, zmienione pliki i wykonane kontrole.

Narzędzia plikowe pomijają ukryte pliki, magazyn kont, klucze, pliki baz danych
i wygenerowane katalogi; odrzucają wyjście poza `sources` oraz symlinki. Komendy
kontrolne są zamrożone z manifestu, a model wybiera jedynie ich nazwę. Kopia robocza
nie jest piaskownicą systemową: lokalne komendy i testy wykonują kod projektu
z uprawnieniami dewelopera. Nie dziedziczą zmiennych środowiska z tokenami OAuth.
Kontrole powinny używać testowej bazy; pliki migracji można edytować, lecz Ripple
nie uruchamia automatycznie migracji na bazie aplikacji.

Limity prototypu: 512 plików tekstowych UTF-8, 512 KB na plik, 8 MB źródeł,
64 odpowiedzi modelu, 64 wywołania narzędzi, 10 minut na zadanie i do 120 sekund
na pojedynczą kontrolę. Zawęź `sources`, jeśli repozytorium jest większe.

Manifest i jego kontrole można sprawdzić bez logowania i zużywania subskrypcji:

```sh
cargo run --locked -p ripple-dev -- --project ripple.project.json --check
```

Zmiany dokumentu odświeżają widok od razu. Kod TypeScript trzeba przebudować,
a proces backendu uruchomić ponownie; wynik kontroli z kopii nie instaluje jej
artefaktów w aplikacji. W tym repozytorium zatrzymaj serwer i uruchom `npm run dev`.
Automatyczny restart, typowany klient API, adapter bazy i sesje użytkowników
aplikacji są kolejnymi częściami runtime. OAuth ChatGPT uwierzytelnia dewelopera
korzystającego z modelu — aplikacja potrzebuje własnego logowania użytkowników.

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
- `crates/ripple-core/` — OAuth, magazyn kont, dokument, transport modeli i wykonawca
  zmian w projekcie.
- `crates/ripple-dev/` — lokalny serwer deweloperski.
- `examples/playground/` — punkt wejścia przykładowej strony.
- `ripple.page.json` — przykładowa strona i źródło zmian prototypu.
- `ripple.project.json` — źródła i kontrole projektu fullstack.

## Zakres pierwszej wersji

Strony są drzewami dokumentu Ripple ze stylami inline. Ten prototyp obsługuje
podstawowe elementy HTML. Logika TypeScript i Rust jest edytowana w plikach projektu,
a nie osadzana w JSON-ie strony. Runtime nie ma jeszcze deklaratywnych powiązań
z API ani adaptera bazy danych. Rysunki trafiają do modelu jako wektory oraz schemat PNG
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
