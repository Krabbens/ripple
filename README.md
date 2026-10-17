# Ripple

Plugin deweloperski do Astro, React/Vite i Next.js. Zaznaczasz element na swojej
stronie, przeciągasz go albo rysujesz uwagę. GPT-6 Luna dostaje kontekst i poprawia
kod źródłowy. Lokalny proces Rust sprawdza poprawkę i zapisuje ją w projekcie.

Ripple korzysta z Twojej subskrypcji ChatGPT przez bezpośredni OAuth. Model:
`gpt-6-luna`, `reasoning.effort: low`. Tokeny pozostają w procesie Rust.

## Uruchomienie repozytorium

Potrzebne są Rust stable przez `rustup` oraz Node.js 24+ z npm.

```sh
npm ci
npm run dev
```

Otwórz [127.0.0.1:4310](http://127.0.0.1:4310). Playground jest zwykłą stroną HTML
z osadzonym pluginem. Zachowuje design prototypu, ale jego źródłem jest
`examples/plugin/index.html`.

1. Kliknij element w trybie **Zaznacz**. Przycisk rodzica rozszerza zaznaczenie.
2. Przeciągnij go przed lub za inny element. Znacznik określa miejsce wstawienia.
3. Użyj **Rysuj**, aby dodać szkic, i opisz jedną zmianę.
4. Połącz subskrypcję przez **Continue with ChatGPT**.
5. Wybierz **Zastosuj zmiany**. Pasek pokazuje rzeczywiste etapy, obok biegnie czas.

Przeciągnięcie zapisuje zamiar zmiany w źródle. Plugin nie przestawia DOM-u Reacta
ani nie przejmuje stanu komponentów. Po zapisie widok odświeża mechanizm HMR
aplikacji. **Cofnij gest** i **Wyczyść** dotyczą niewysłanych gestów.

## Szybkie operacje na elementach

Po zaznaczeniu elementu panel udostępnia **Usuń**, **Duplikuj**, przesunięcie
wcześniej lub później oraz **Zmień tekst**. Edycja tekstu działa dla elementu bez
zagnieżdżonych znaczników; dla złożonego fragmentu zaznacz jego tekstowe dziecko.

Usunięcie oznacza fragment na czerwono. Lista **Zaplanowane zmiany** pokazuje
operacje, które zostaną zapisane po wybraniu **Zastosuj zmiany**. Dodatkowa
instrukcja nie jest potrzebna. Możesz nadal dodać opis lub szkic.

| Skrót w trybie zaznaczania | Operacja                    |
| -------------------------- | --------------------------- |
| `Delete` / `Backspace`     | Usuń element                |
| `⌘/Ctrl+D`                 | Duplikuj                    |
| `Alt+↑` / `Alt+↓`          | Przesuń wcześniej / później |
| `⌘/Ctrl+Z`                 | Cofnij ostatni gest         |

Skróty nie przechwytują pisania w polach tekstowych. **Cofnij gest** przywraca
poprzedni plan, a **Wyczyść** odrzuca cały niezapisany zestaw. Kontenery dokumentu
(`body`, `html` i punkt montowania pluginu) są chronione przed usunięciem.

Plugin oznacza plan w swojej warstwie, zachowując DOM i stan aplikacji do czasu
zapisu źródeł. Runtime prototypu JSON ma osobny lokalny podgląd tych operacji.

## Przykłady frameworków

Zbuduj paczki przez `npm run build`. Uruchom jeden serwer Ripple i odpowiedni
frontend w drugim terminalu:

| Framework  | Serwer Ripple                                                             | Frontend            | Adres                   |
| ---------- | ------------------------------------------------------------------------- | ------------------- | ----------------------- |
| React/Vite | `cargo run -p ripple-dev -- --project examples/react/ripple.project.json` | `npm run dev:react` | `http://127.0.0.1:5173` |
| Astro      | `cargo run -p ripple-dev -- --project examples/astro/ripple.project.json` | `npm run dev:astro` | `http://127.0.0.1:4321` |
| Next.js    | `cargo run -p ripple-dev -- --project examples/next/ripple.project.json`  | `npm run dev:next`  | `http://127.0.0.1:3000` |

Zatrzymaj domyślny serwer przed uruchomieniem przykładu. Jeden proces korzysta
z magazynu kont, więc kolejne logowanie nie jest potrzebne. Manifest powinien
należeć do aplikacji, której pliki chcesz edytować.

## Integracja

Paczki są na razie lokalnymi workspace'ami, nie wydaniem w rejestrze npm.
`@ripple/plugin` udostępnia wspólny panel i adaptery. Do innego repozytorium można
przenieść zbudowane paczki jako lokalne archiwa:

```sh
npm run build
mkdir -p /tmp/ripple-packages
npm pack --workspace @ripple/core --workspace @ripple/harness --workspace @ripple/plugin --pack-destination /tmp/ripple-packages
# W aplikacji:
npm install --save-dev /tmp/ripple-packages/ripple-*.tgz
```

### Astro

```js
// astro.config.mjs
import { defineConfig } from "astro/config";
import ripple from "@ripple/plugin/astro";

export default defineConfig({
  integrations: [ripple()],
});
```

Integracja wstrzykuje panel tylko dla `astro dev`. Obsługuje też zmianę strony
przez Astro ClientRouter.

### React / Vite

```ts
// vite.config.ts
import { defineConfig } from "vite";
import ripple from "@ripple/plugin/vite";

export default defineConfig({ plugins: [ripple()] });
```

Można też umieścić komponent `Ripple` z `@ripple/plugin/react` obok aplikacji.
Nie używaj obu adapterów jednocześnie. Panel montuje się po hydracji i sprząta
nasłuchy przy odmontowaniu, również w React StrictMode.

### Next.js App Router

```tsx
// app/layout.tsx, wewnątrz body, obok children
import { Ripple } from "@ripple/plugin/next";

// ...
{
  process.env.NODE_ENV !== "production" && <Ripple />;
}
```

`Ripple` jest komponentem klienckim, podczas SSR zwraca `null`. W produkcji nie
montuje panelu. Przy lokalnych paczkach workspace użyj `transpilePackages` jak
w `examples/next/next.config.mjs`.

### Wskazanie źródła

Dodaj ścieżkę względem katalogu manifestu do kontenera komponentu:

```tsx
<section data-ripple-source="src/components/Contact.tsx">
  <h2>Porozmawiajmy</h2>
</section>
```

Dzieci dziedziczą najbliższy znacznik. Bez niego plugin korzysta z opcji
`sourceFile` adaptera albo `entrypoints` manifestu. To jawne powiązanie DOM-u
ze źródłem; automatyczne mapowanie wszystkich komponentów przez kompilator
nie jest jeszcze zaimplementowane. `data-ripple-ignore` wyłącza fragment
z zaznaczania.

Adaptery przyjmują `endpoint` (domyślnie `http://127.0.0.1:4310`), `sourceFile`
i `reserveSpace`. Panel domyślnie nakłada się na aplikację; `reserveSpace: true`
odsuwa widok na szerokim ekranie. Na małych ekranach startuje zwinięty.
Jawne `mountRipple()` z `@ripple/plugin` nadaje się do własnego wejścia dev.

## Projekt fullstack i małe kroki

`ripple.project.json` określa pliki, które Luna może zmieniać, adresy lokalnego
frontendu i komendy sprawdzające projekt. Przykład:

```json
{
  "version": 1,
  "entrypoints": ["src/app/page.tsx"],
  "sources": [
    "src",
    "server",
    "migrations",
    "package.json",
    "tsconfig.json",
    "Cargo.toml",
    "Cargo.lock"
  ],
  "devOrigins": ["http://127.0.0.1:3000"],
  "checks": [
    {
      "name": "TypeScript",
      "command": ["npm", "run", "typecheck"],
      "timeoutSeconds": 60
    },
    {
      "name": "Rust",
      "command": ["cargo", "test", "--locked"],
      "timeoutSeconds": 120
    }
  ],
  "instructions": "Frontend Next, backend Rust. Zachowaj istniejące kontrakty API."
}
```

Ścieżki są względne wobec katalogu manifestu. Dodaj wszystkie źródła i konfiguracje
potrzebne do kontroli; zainstaluj zależności przed uruchomieniem Ripple. Na macOS
i Linux wykonawca wykorzystuje zależności z lokalnego i nadrzędnych `node_modules`, przekierowuje
lokalne paczki workspace do kopii i współdzieli cache Cargo w `target/`.

Nowa ścieżka pluginu ma konkretne limity:

- Do 3 plików źródłowych i 64 KB kontekstu w kroku.
- Jedna odpowiedź Luny; druga tylko wtedy, gdy model potrzebuje dodatkowego pliku.
- Do 12 poprawek tekstu. Każdy zastępowany fragment musi wystąpić dokładnie raz.
- 120 sekund na odpowiedzi modelu; osobny limit 1–120 sekund na każdą kontrolę.

Luna otrzymuje źródła od razu. Nie wykonuje osobnej rundy na każdy odczyt, zapis
czy kontrolę. Po nieudanych kontrolach krok się kończy, bez długiej pętli napraw.
Możesz zmienić instrukcję i wysłać mniejsze zadanie. API, backend Rust i migracje
SQL mieszczą się w tym samym mechanizmie, jeśli należą do `sources`.

Poprawki trafiają do kopii roboczej. Serwer zapisuje je po przejściu wszystkich
kontroli na końcowej wersji źródeł. Zewnętrzna zmiana pliku lub manifestu w trakcie
pracy odrzuca zapis. Błędy modelu i kontroli pozostawiają oryginały bez zmian.
Przy utracie połączenia w czasie zapisu sprawdź pliki przed ponowieniem.

Konfigurację można sprawdzić bez OAuth i użycia modelu:

```sh
cargo run -p ripple-dev -- --project ripple.project.json --check
```

Serwer słucha na `127.0.0.1`. `devOrigins` dopuszcza dokładne lokalne originy HTTP;
żądania zmiany wymagają originu i nagłówka `X-Ripple-Request`. Postęp wraca przez
SSE: kontekst, model, kontrole, zapis, zakończenie. Pasek pokazuje etap, nie
przewiduje procentu wygenerowanego kodu.

Źródła mają limit 512 plików UTF-8, 512 KB na plik i 8 MB łącznie. Wykonawca
pomija ukryte pliki, klucze, bazy danych i wygenerowane katalogi; odrzuca symlinki
oraz ścieżki poza `sources`. Komendy kontrolne są zamrożone z manifestu. Kopia
robocza nie jest piaskownicą systemową: testy wykonują kod projektu z uprawnieniami
dewelopera, bez tokenów OAuth w środowisku. Używaj testowej bazy. Ripple edytuje
pliki migracji, ale nie uruchamia migracji na bazie aplikacji.

HMR dotyczy frontendu. Restart backendu, wdrożenie, adapter bazy i logowanie
użytkowników Twojej aplikacji pozostają po stronie jej frameworka i narzędzi.
OAuth ChatGPT służy deweloperowi do korzystania z modelu.

## Logowanie i model

Ripple nie wymaga Codex CLI, SDK, app-server ani klucza API. Własny klient Rust
stosuje opisany przez OpenAI przepływ [ChatGPT plan usage](https://developers.openai.com/siwc/token-sharing-open-source/sign-in)
i [bezpośredni Responses API](https://developers.openai.com/siwc/token-sharing-open-source/models-and-inference):
PKCE, dynamiczna rejestracja klienta, lokalny callback, `store: false`, `stream: true`.
Poprawki korzystają ze [structured outputs](https://developers.openai.com/api/docs/guides/structured-outputs).

Identyfikator hosta UUIDv4 i konta są przechowywane w chronionym
`~/.config/ripple/accounts.json`. `XDG_CONFIG_HOME` lub `RIPPLE_HOME` zmieniają
lokalizację. Proces weryfikuje podpis, issuer, audience, nonce i ważność ID tokenu.
Katalog modeli konta jest buforowany przez 5 minut. Jeśli Luna nie jest dostępna,
Ripple zwraca błąd i nie podstawia innego modelu.

## Kod i sprawdzanie

- `packages/plugin/`: adaptery Astro, React, Next, Vite i jawne wejście przeglądarkowe.
- `packages/harness/`: panel w Shadow DOM, gesty i strumień postępu.
- `packages/core/`: obserwacja DOM-u oraz kompatybilny runtime dokumentów.
- `crates/ripple-core/`: OAuth, transport modelu, małe poprawki i kontrole źródeł.
- `crates/ripple-dev/`: lokalny serwer i most do aplikacji.
- `examples/`: zwykłe HTML oraz aplikacje React/Vite, Astro i Next.

UI stosuje wariant `minimalist-ui` z [taste-skill](https://github.com/Leonxlnx/taste-skill).
Przypięta kopia i licencja są w `.agents/skills/minimalist-ui/`.

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
npm run format:check
npm run typecheck
npm test
npm run build:examples
```

GitHub Actions wykonuje te kontrole. Testy automatyczne nie używają prawdziwego
konta ani subskrypcji. Lokalne próby z OAuth wymagają połączonego konta.

Starszy prototyp z `ripple.page.json` i własnym rendererem pozostaje dostępny przez
`npm run dev -- --page ripple.page.json`. Manifest z polem `page` uruchamia starszą
edycję projektu z narzędziami. Limity małych kroków i SSE dotyczą pluginu.
Nowe integracje nie wymagają dokumentu JSON.
