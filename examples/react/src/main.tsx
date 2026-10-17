import { StrictMode, useState } from "react";
import { createRoot } from "react-dom/client";
import "./style.css";
function App() {
  const [count, setCount] = useState(0);
  return (
    <main data-ripple-source="src/main.tsx">
      <nav>
        <strong>ripple</strong>
        <small>React / Vite</small>
      </nav>
      <h1>
        Małe kroki.
        <br />
        Duże zmiany.
      </h1>
      <p>
        Zaznacz komponent, przeciągnij pomysł, dopisz jedno zdanie. Ripple
        zmienia kod Twojej aplikacji.
      </p>
      <section className="cards">
        <article>
          <small>01 / Zaznacz</small>
          <h2>Zacznij od fragmentu.</h2>
          <p>
            Źródłem jest zwykły komponent React. Stan przycisku należy do
            Reacta.
          </p>
          <button onClick={() => setCount(count + 1)}>
            Kliknięcia: {count}
          </button>
        </article>
        <article>
          <small>02 / Zmień</small>
          <h2>Pomysł trafia do kodu.</h2>
          <p>
            Po zapisie Vite odświeża aplikację. Panel pozostaje obok Twojej
            strony.
          </p>
        </article>
      </section>
      <footer>Ripple · plugin deweloperski</footer>
    </main>
  );
}
createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
