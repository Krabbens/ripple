import { Ripple } from "@ripple/plugin/next";
export default function Page() {
  return (
    <>
      <main data-ripple-source="app/page.tsx">
        <nav>
          <strong>ripple</strong>
          <small>Next.js</small>
        </nav>
        <h1>
          Małe kroki.
          <br />
          Duże zmiany.
        </h1>
        <p>
          Komponenty serwerowe i klienckie zostają w Next. Ripple dodaje
          narzędzia do pracy na Twoim kodzie.
        </p>
        <section className="cards">
          <article>
            <small>01 / Zaznacz</small>
            <h2>Zacznij od fragmentu.</h2>
            <p>
              Panel montuje się po hydracji. Widzi żywy DOM oraz ścieżkę
              źródłową elementu.
            </p>
          </article>
          <article>
            <small>02 / Zmień</small>
            <h2>Pomysł trafia do kodu.</h2>
            <p>
              Wybierz mały krok. Po kontrolach zapisujemy poprawkę, a Next
              odświeża widok.
            </p>
          </article>
        </section>
        <footer>Ripple · plugin deweloperski</footer>
      </main>
      {process.env.NODE_ENV !== "production" && (
        <Ripple sourceFile="app/page.tsx" />
      )}
    </>
  );
}
