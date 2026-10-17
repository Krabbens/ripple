import type {
  RippleRuntime,
  DomSurface,
  RipplePage,
  Stroke,
  Point,
  VisualAction,
} from "@ripple/core";
import { styles } from "./styles.js";
import { icon } from "./icons.js";
import { siblingOrder, actionLabel } from "./operations.js";
import { readChangeStream } from "./progress.js";
import type { ChangeResult, ProgressEvent } from "./progress.js";

interface ServerState {
  document: RipplePage | null;
  revision: string;
  account: { email: string | null; subject: string } | null;
  model: string;
  effort: string;
  project: { sources: string[]; checks: string[] } | null;
}
type Mode = "browse" | "select" | "draw";
interface Snapshot {
  page: RipplePage | null;
  actions: VisualAction[];
  strokes: Stroke[];
  selected: string | null;
}
interface Drop {
  parentId: string;
  beforeId: string | null;
}

export interface HarnessOptions {
  endpoint?: string;
  reserveSpace?: boolean;
  onApplied?: (result: ChangeResult) => void;
}
/** Adapters mount this only in development. The host owns its DOM. */
export function mountHarness(
  runtime: RippleRuntime | DomSurface,
  initialRevision: string,
  settings: HarnessOptions = {},
): { destroy(): void } {
  const endpoint = (settings.endpoint ?? "").replace(/\/$/, "");
  const host = document.createElement("ripple-devtools");
  host.style.cssText =
    "position:fixed;inset:0;z-index:2147483647;pointer-events:none";
  const shadow = host.attachShadow({ mode: "open" });
  shadow.innerHTML = `<style>${styles}</style>
    <div class="planned-layer" aria-hidden="true"></div><div class="outline"></div><div class="drop"></div><svg class="draw-layer" aria-hidden="true"></svg>
    <section class="panel" data-ui aria-label="Ripple — tryb deweloperski">
      <div class="top"><div class="brand">${icon("logo")} ripple</div><span class="badge">Edytor</span><button data-collapse aria-label="Zwiń panel" aria-expanded="true">${icon("collapse")}</button></div>
      <p class="intro">Zmieniaj stronę w jej własnym widoku.</p>
      <div class="tools" aria-label="Narzędzia">
        <button data-mode="browse" aria-pressed="false">${icon("browse")}Przeglądaj</button>
        <button data-mode="select" aria-pressed="true">${icon("select")}Zaznacz</button>
        <button data-mode="draw" aria-pressed="false">${icon("draw")}Rysuj</button>
      </div>
      <div class="label">Wybrany element <kbd>Esc</kbd></div>
      <div class="selection"><span data-selection>Wybierz element strony</span><button data-parent title="Zaznacz rodzica" aria-label="Zaznacz rodzica" disabled>${icon("parent")}</button></div>
      <div class="element-actions" role="group" aria-label="Operacje na elemencie">
        <button data-element-action="earlier" aria-label="Przesuń wcześniej" title="Przesuń wcześniej · Alt+↑" disabled>${icon("earlier")}</button>
        <button data-element-action="later" aria-label="Przesuń później" title="Przesuń później · Alt+↓" disabled>${icon("later")}</button>
        <button data-element-action="duplicate" title="Duplikuj · ⌘/Ctrl+D" disabled>${icon("duplicate")}Duplikuj</button>
        <button data-element-action="remove" class="remove-button" title="Usuń · Delete/Backspace" disabled>${icon("remove")}Usuń</button>
      </div>
      <button class="edit-text" data-edit-text disabled>${icon("edit")}Zmień tekst</button>
      <div class="text-editor" hidden>
        <label class="label" for="ripple-element-text">Nowy tekst elementu</label>
        <textarea id="ripple-element-text" maxlength="20000"></textarea>
        <div class="row"><button data-save-text>Dodaj zmianę</button><button data-cancel-text>Anuluj</button></div>
      </div>
      <p class="hint" data-hint>Kliknij element. Przytrzymaj, żeby przenieść.</p>
      <section class="draft" aria-label="Zaplanowane zmiany" hidden>
        <div class="label">Zaplanowane zmiany <span data-plan-count></span></div>
        <ol data-plan></ol>
        <p class="hint">Zapis po wybraniu „Zastosuj zmiany”.</p>
      </section>
      <div class="compose"><label class="label" for="ripple-instruction">Opisz zmianę</label>
        <textarea id="ripple-instruction" aria-label="Opisz zmianę" placeholder="Np. więcej miejsca wokół nagłówka i krótszy tekst przycisku."></textarea>
        <div class="row meta"><span data-count>0 gestów</span><div><button data-undo disabled>Cofnij gest</button> <button data-reset>Wyczyść</button></div></div>
      </div>
      <button class="send" data-send>Zastosuj zmiany ${icon("send")}</button>
      <div class="progress" hidden data-progress>
        <div class="progress-meta"><span data-phase>Przygotowanie</span><span data-elapsed>0 s</span></div>
        <progress aria-label="Postęp zmiany" max="4" value="0"></progress>
        <p class="hint">Mały krok · do 3 plików · maks. 2 odpowiedzi Luny</p>
      </div>
      <div class="model"><span>GPT-6 Luna</span><span>Myślenie: low</span></div>
      <div class="account"><a href="/auth/login" target="_blank" rel="noopener" data-account>Continue with ChatGPT</a></div>
      <p class="source" data-source></p>
      <p class="source" data-project hidden></p>
      <div class="status" role="status" aria-live="polite"></div>
    </section>`;
  document.body.append(host);
  const required = <T extends Element>(selector: string): T => {
    const node = shadow.querySelector<T>(selector);
    if (!node) throw new Error(`Missing harness control ${selector}`);
    return node;
  };
  const outline = required<HTMLDivElement>(".outline");
  const marker = required<HTMLDivElement>(".drop");
  const svg = required<SVGSVGElement>("svg");
  const input = required<HTMLTextAreaElement>("#ripple-instruction");
  const textInput = required<HTMLTextAreaElement>("#ripple-element-text");
  const send = required<HTMLButtonElement>("[data-send]");
  const abort = new AbortController();
  const options = { signal: abort.signal, capture: true };
  const originalMargin = runtime.mount.style.marginRight;
  const originalCursor = runtime.mount.style.cursor;
  const originalUserSelect = runtime.mount.style.userSelect;
  required<HTMLAnchorElement>("[data-account]").href = `${endpoint}/auth/login`;
  let elapsedTimer: ReturnType<typeof setInterval> | undefined;
  const started = { at: 0 };
  function progress(event: ProgressEvent): void {
    required<HTMLElement>("[data-progress]").hidden = false;
    required("[data-phase]").textContent = event.label;
    const bar = required<HTMLProgressElement>("progress");
    bar.max = event.total;
    bar.value = event.step;
    bar.setAttribute("aria-valuetext", event.label);
    required<HTMLElement>("[data-progress]").dataset["active"] = String(
      event.phase === "model",
    );
  }
  required("[data-source]").textContent = runtime.sourceFile;
  function syncCanvasInset(): void {
    const expanded =
      required<HTMLElement>(".panel").dataset["collapsed"] !== "true";
    runtime.mount.style.marginRight =
      settings.reserveSpace !== false && innerWidth >= 980 && expanded
        ? "360px"
        : originalMargin;
  }
  let planVersion = 0;
  let renderedPlanVersion = -1;
  let editing: string | null = null;
  let mode: Mode = "select";
  let selected: string | null = null;
  let actions: VisualAction[] = [];
  let strokes: Stroke[] = [];
  let history: Snapshot[] = [];
  let baseline = runtime.kind === "document" ? runtime.page : null;
  let revision = initialRevision;
  let busy = false;
  let projectMode = false;
  let drag: {
    id: string;
    start: Point;
    active: boolean;
    drop: Drop | null;
  } | null = null;
  let stroke: Stroke | null = null;

  function status(message: string, error = false): void {
    const node = required<HTMLDivElement>(".status");
    node.textContent = message;
    node.dataset["error"] = String(error);
  }
  function remember(): void {
    history.push({
      page: runtime.kind === "document" ? runtime.page : null,
      actions: structuredClone(actions),
      strokes: structuredClone(strokes),
      selected,
    });
    if (history.length > 30) history.shift();
  }
  function addAction(action: VisualAction): void {
    if (actions.length >= 100)
      throw new Error("Wyślij bieżące gesty przed dodaniem kolejnych.");
    actions.push(action);
    planVersion++;
  }
  function parentInPlan(id: string): string | undefined {
    const moved =
      runtime.kind === "dom"
        ? [...actions]
            .reverse()
            .find(
              (action) => action.type === "move" && action.element.id === id,
            )
        : undefined;
    return moved?.type === "move" ? moved.parent.id : runtime.parent(id);
  }
  function pendingRemoval(id: string): boolean {
    const visited = new Set<string>();
    let current: string | undefined = id;
    while (current && !visited.has(current)) {
      visited.add(current);
      if (
        actions.some(
          (action) => action.type === "remove" && action.element.id === current,
        )
      )
        return true;
      current = parentInPlan(current);
    }
    return false;
  }
  function canManipulate(id: string | null): boolean {
    const element = id ? runtime.element(id) : undefined;
    return Boolean(
      id &&
      element &&
      element !== runtime.mount &&
      !["BODY", "HTML"].includes(element.tagName) &&
      runtime.parent(id) &&
      !pendingRemoval(id),
    );
  }
  function canEditText(id: string | null): boolean {
    const element = id ? runtime.element(id) : undefined;
    return (
      canManipulate(id) &&
      Boolean(
        element &&
        element.children.length === 0 &&
        !["IMG", "INPUT", "HR", "BR", "TEXTAREA", "SELECT"].includes(
          element.tagName,
        ) &&
        (element.textContent?.length ?? 0) <= 20_000,
      )
    );
  }
  function placement(
    id: string,
  ): { parentId: string; siblings: string[] } | undefined {
    const parentId = parentInPlan(id);
    const parent = parentId ? runtime.element(parentId) : undefined;
    if (!parent || !parentId) return undefined;
    const initial = Array.from(parent.children)
      .filter(
        (element): element is HTMLElement =>
          element instanceof HTMLElement && runtime.target(element) === element,
      )
      .map((element) => runtime.id(element));
    return {
      parentId,
      siblings:
        runtime.kind === "dom"
          ? siblingOrder(initial, parentId, actions)
          : initial,
    };
  }
  function pendingPlan(): void {
    const list = required<HTMLOListElement>("[data-plan]");
    if (planVersion !== renderedPlanVersion) {
      list.replaceChildren();
      for (const action of actions) {
        const label = actionLabel(action);
        if (!label) continue;
        const item = document.createElement("li");
        item.textContent = label;
        item.dataset["operation"] = action.type;
        list.append(item);
      }
      required<HTMLElement>(".draft").hidden = list.children.length === 0;
      required("[data-plan-count]").textContent = String(list.children.length);
      renderedPlanVersion = planVersion;
    }
    const layer = required<HTMLElement>(".planned-layer");
    layer.replaceChildren();
    if (runtime.kind !== "dom" || mode === "browse") return;
    for (const action of actions) {
      if (action.type !== "remove") continue;
      const element = runtime.element(action.element.id);
      if (!element) continue;
      const rect = element.getBoundingClientRect();
      const mark = document.createElement("div");
      mark.className = "removal-mark";
      Object.assign(mark.style, {
        left: `${rect.x}px`,
        top: `${rect.y}px`,
        width: `${rect.width}px`,
        height: `${rect.height}px`,
      });
      const label = document.createElement("span");
      label.textContent = "Do usunięcia";
      mark.append(label);
      layer.append(mark);
    }
  }
  function refresh(): void {
    const element = selected ? runtime.element(selected) : undefined;
    outline.dataset["removing"] = String(selected && pendingRemoval(selected));
    outline.style.display = element && mode !== "browse" ? "block" : "none";
    if (element) {
      const rect = element.getBoundingClientRect();
      Object.assign(outline.style, {
        left: `${rect.x}px`,
        top: `${rect.y}px`,
        width: `${rect.width}px`,
        height: `${rect.height}px`,
      });
    }
    const lastReference = selected
      ? [...actions]
          .reverse()
          .find(
            (action) => "element" in action && action.element.id === selected,
          )
      : undefined;
    const reference =
      element && selected
        ? runtime.reference(selected)
        : lastReference && "element" in lastReference
          ? lastReference.element
          : undefined;
    required("[data-selection]").textContent = reference
      ? `${reference.tag} #${reference.source.nodeId}${selected && pendingRemoval(selected) ? " · do usunięcia" : ""}`
      : "Wybierz element strony";
    required<HTMLButtonElement>("[data-parent]").disabled =
      busy || !selected || !runtime.parent(selected);
    const editable = !busy && mode === "select" && canManipulate(selected);
    const location = selected ? placement(selected) : undefined;
    const position =
      selected && location ? location.siblings.indexOf(selected) : -1;
    for (const button of shadow.querySelectorAll<HTMLButtonElement>(
      "[data-element-action]",
    )) {
      const operation = button.dataset["elementAction"];
      button.disabled =
        !editable ||
        (operation === "earlier" && position <= 0) ||
        (operation === "later" &&
          (position < 0 || position >= (location?.siblings.length ?? 0) - 1));
    }
    required<HTMLButtonElement>("[data-edit-text]").disabled =
      !editable || !canEditText(selected);
    required<HTMLElement>(".text-editor").hidden = !editing;
    required<HTMLButtonElement>("[data-save-text]").disabled = busy;
    required<HTMLButtonElement>("[data-cancel-text]").disabled = busy;
    required<HTMLButtonElement>("[data-reset]").disabled = busy;
    textInput.disabled = busy;
    pendingPlan();
    required("[data-source]").textContent =
      selected && runtime.element(selected)
        ? runtime.reference(selected).source.file || "Źródło z entrypoints"
        : runtime.sourceFile || "Źródło z entrypoints";
    const count = actions.length;
    const noun =
      count === 1
        ? "gest"
        : count % 10 >= 2 &&
            count % 10 <= 4 &&
            !(count % 100 >= 12 && count % 100 <= 14)
          ? "gesty"
          : "gestów";
    required("[data-count]").textContent = `${count} ${noun}`;
    required<HTMLButtonElement>("[data-undo]").disabled =
      busy || history.length === 0;
    svg.replaceChildren();
    for (const line of strokes) {
      const path = document.createElementNS(
        "http://www.w3.org/2000/svg",
        "polyline",
      );
      path.setAttribute(
        "points",
        line.points
          .map((point) => `${point.x - scrollX},${point.y - scrollY}`)
          .join(" "),
      );
      path.setAttribute("fill", "none");
      path.setAttribute("stroke", line.color);
      path.setAttribute("stroke-width", String(line.width));
      path.setAttribute("stroke-linecap", "round");
      path.setAttribute("stroke-linejoin", "round");
      svg.append(path);
    }
  }
  function select(id: string): void {
    if (selected !== id) {
      remember();
      if (actions.at(-1)?.type === "select") actions.pop();
      addAction({ type: "select", element: runtime.reference(id) });
    }
    if (selected !== id) editing = null;
    selected = id;
    refresh();
  }
  function setMode(next: Mode): void {
    mode = next;
    editing = null;
    drag = null;
    marker.style.display = "none";
    for (const button of shadow.querySelectorAll<HTMLButtonElement>(
      "[data-mode]",
    ))
      button.setAttribute(
        "aria-pressed",
        String(button.dataset["mode"] === mode),
      );
    runtime.mount.style.cursor =
      mode === "draw" ? "crosshair" : mode === "select" ? "default" : "";
    runtime.mount.style.userSelect = mode === "browse" ? "" : "none";
    required("[data-hint]").textContent =
      mode === "draw"
        ? "Rysuj bezpośrednio na stronie. Dodaj opis poniżej."
        : mode === "browse"
          ? "Strona działa normalnie. Esc przełącza na przeglądanie."
          : "Kliknij element. Przytrzymaj, żeby przenieść.";
    refresh();
  }
  function pageElement(target: EventTarget | null): HTMLElement | undefined {
    return runtime.target(target);
  }

  function block(event: Event): void {
    event.preventDefault();
    event.stopImmediatePropagation();
  }
  function inPanel(event: Event): boolean {
    return event.composedPath().includes(host);
  }
  function point(event: PointerEvent): Point {
    return { x: event.clientX + scrollX, y: event.clientY + scrollY };
  }

  window.addEventListener(
    "pointerdown",
    (event) => {
      if (
        busy ||
        inPanel(event) ||
        mode === "browse" ||
        event.button !== 0 ||
        !pageElement(event.target)
      )
        return;
      block(event);
      if (mode === "draw") {
        remember();
        stroke = { points: [point(event)], color: "#1f6c9f", width: 3 };
        strokes.push(stroke);
      } else {
        const element = pageElement(event.target)!;
        const id = runtime.id(element);
        try {
          select(id);
        } catch (error) {
          status(String(error), true);
          return;
        }
        if (pendingRemoval(id)) return;
        drag = { id, start: point(event), active: false, drop: null };
      }
      refresh();
    },
    options,
  );

  window.addEventListener(
    "pointermove",
    (event) => {
      if (stroke) {
        block(event);
        if (stroke.points.length < 2000) stroke.points.push(point(event));
        refresh();
        return;
      }
      if (!drag) return;
      if (
        Math.hypot(
          point(event).x - drag.start.x,
          point(event).y - drag.start.y,
        ) < 6 &&
        !drag.active
      )
        return;
      block(event);
      drag.active = true;
      drag.drop = null;
      marker.style.display = "none";
      const target = pageElement(
        document.elementFromPoint(event.clientX, event.clientY),
      );
      const source = runtime.element(drag.id);
      if (
        !target ||
        !source ||
        source.contains(target) ||
        target.contains(source)
      )
        return;
      const parent = target.parentElement;
      if (
        !parent ||
        !runtime.target(parent) ||
        pendingRemoval(runtime.id(target)) ||
        pendingRemoval(runtime.id(parent))
      )
        return;
      const rect = target.getBoundingClientRect();
      const horizontal =
        getComputedStyle(parent).display === "flex" &&
        getComputedStyle(parent).flexDirection.startsWith("row");
      const before = horizontal
        ? event.clientX < rect.x + rect.width / 2
        : event.clientY < rect.y + rect.height / 2;
      const siblings = Array.from(parent.children) as HTMLElement[];
      const following = siblings[siblings.indexOf(target) + 1];
      drag.drop = {
        parentId: runtime.id(parent),
        beforeId: before
          ? runtime.id(target)
          : following
            ? runtime.id(following)
            : null,
      };
      Object.assign(
        marker.style,
        horizontal
          ? {
              display: "block",
              left: `${before ? rect.left : rect.right}px`,
              top: `${rect.top}px`,
              width: "3px",
              height: `${rect.height}px`,
            }
          : {
              display: "block",
              left: `${rect.left}px`,
              top: `${before ? rect.top : rect.bottom}px`,
              width: `${rect.width}px`,
              height: "3px",
            },
      );
    },
    options,
  );

  function finish(event: PointerEvent): void {
    if (stroke) {
      block(event);
      try {
        addAction({ type: "draw", stroke: structuredClone(stroke) });
      } catch (error) {
        status(String(error), true);
      }
      stroke = null;
    }
    if (drag?.active && drag.drop) {
      block(event);
      const { id, drop } = drag;
      try {
        const action: VisualAction = {
          type: "move",
          element: runtime.reference(id),
          parent: runtime.reference(drop.parentId),
          before: drop.beforeId ? runtime.reference(drop.beforeId) : null,
        };
        queue(action, () => {
          if (runtime.kind === "document")
            runtime.move(id, drop.parentId, drop.beforeId);
        });
        status(
          runtime.kind === "dom"
            ? "Miejsce zaznaczone. Zastosuj zmianę, aby Luna poprawiła kod komponentu."
            : "Nowy układ jest podglądem. Zastosuj go z Luną, aby zapisać stronę.",
        );
      } catch (error) {
        status(
          error instanceof Error
            ? error.message
            : "Nie można przenieść elementu.",
          true,
        );
      }
    }
    drag = null;
    marker.style.display = "none";
    refresh();
  }
  window.addEventListener("pointerup", finish, options);
  window.addEventListener(
    "pointercancel",
    () => {
      drag = null;
      stroke = null;
      marker.style.display = "none";
      refresh();
    },
    options,
  );
  window.addEventListener(
    "click",
    (event) => {
      if (mode !== "browse" && !inPanel(event) && pageElement(event.target))
        block(event);
    },
    options,
  );
  window.addEventListener(
    "dragstart",
    (event) => {
      if (mode !== "browse" && !inPanel(event)) block(event);
    },
    options,
  );
  window.addEventListener("scroll", refresh, options);
  window.addEventListener(
    "resize",
    () => {
      syncCanvasInset();
      refresh();
    },
    options,
  );
  runtime.mount.addEventListener("ripple:render", refresh, {
    signal: abort.signal,
  });
  window.addEventListener(
    "keydown",
    (event) => {
      if (busy || event.repeat) return;
      const typing = event
        .composedPath()
        .some(
          (node) =>
            node instanceof HTMLElement &&
            (node.isContentEditable ||
              ["INPUT", "TEXTAREA", "SELECT"].includes(node.tagName)),
        );
      if (event.key === "Escape") {
        if (editing) closeTextEditor();
        else setMode("browse");
        return;
      }
      if (typing || mode !== "select") return;
      if (
        (event.metaKey || event.ctrlKey) &&
        !event.shiftKey &&
        !event.altKey &&
        event.key.toLowerCase() === "z" &&
        history.length
      ) {
        block(event);
        undo();
        return;
      }
      if (!canManipulate(selected)) return;
      if (
        ["Delete", "Backspace"].includes(event.key) &&
        !event.metaKey &&
        !event.ctrlKey &&
        !event.altKey
      ) {
        block(event);
        manipulate("remove");
      }
      if (
        (event.metaKey || event.ctrlKey) &&
        !event.shiftKey &&
        !event.altKey &&
        event.key.toLowerCase() === "d"
      ) {
        block(event);
        manipulate("duplicate");
      }
      if (
        event.altKey &&
        !event.metaKey &&
        !event.ctrlKey &&
        ["ArrowUp", "ArrowDown"].includes(event.key)
      ) {
        block(event);
        manipulate(event.key === "ArrowUp" ? "earlier" : "later");
      }
    },
    { signal: abort.signal },
  );

  function queue(action: VisualAction, preview: () => void = () => {}): void {
    if (actions.length >= 100)
      throw new Error("Wyślij bieżące zmiany przed dodaniem kolejnych.");
    remember();
    preview();
    addAction(action);
    editing = null;
  }
  function manipulate(operation: string): void {
    if (busy || mode !== "select" || !selected || !canManipulate(selected))
      return;
    try {
      const id = selected;
      const element = runtime.reference(id);
      if (operation === "remove") {
        queue({ type: "remove", element }, () => {
          if (runtime.kind === "document") runtime.remove(id);
        });
      } else {
        const location = placement(id);
        if (!location) return;
        const position = location.siblings.indexOf(id);
        if (
          position < 0 ||
          (operation === "earlier" && position === 0) ||
          (operation === "later" && position === location.siblings.length - 1)
        )
          return;
        const beforeId =
          operation === "earlier"
            ? location.siblings[position - 1]!
            : (location.siblings[
                position + (operation === "duplicate" ? 1 : 2)
              ] ?? null);
        const action = {
          type:
            operation === "duplicate"
              ? ("duplicate" as const)
              : ("move" as const),
          element,
          parent: runtime.reference(location.parentId),
          before: beforeId ? runtime.reference(beforeId) : null,
        };
        queue(action, () => {
          if (runtime.kind !== "document") return;
          if (action.type === "duplicate") runtime.duplicate(id);
          else runtime.move(id, location.parentId, beforeId);
        });
      }
      status("Zmiana dodana do planu. Możesz ją cofnąć lub zastosować z Luną.");
      refresh();
    } catch (error) {
      status(
        error instanceof Error
          ? error.message
          : "Nie udało się zaplanować zmiany.",
        true,
      );
    }
  }
  for (const button of shadow.querySelectorAll<HTMLButtonElement>(
    "[data-element-action]",
  )) {
    button.addEventListener("click", () =>
      manipulate(button.dataset["elementAction"]!),
    );
  }
  required("[data-edit-text]").addEventListener("click", () => {
    if (busy || !selected || !canEditText(selected)) return;
    editing = selected;
    const previous = [...actions]
      .reverse()
      .find(
        (action) => action.type === "setText" && action.element.id === selected,
      );
    textInput.value =
      previous?.type === "setText"
        ? previous.text
        : (runtime.element(selected)!.textContent ?? "");
    refresh();
    textInput.focus();
  });
  function closeTextEditor(): void {
    editing = null;
    refresh();
    required<HTMLButtonElement>("[data-edit-text]").focus();
  }
  required("[data-cancel-text]").addEventListener("click", () => {
    if (!busy) closeTextEditor();
  });
  required("[data-save-text]").addEventListener("click", () => {
    if (busy || !editing || !canEditText(editing)) return;
    try {
      const id = editing;
      queue(
        {
          type: "setText",
          element: runtime.reference(id),
          text: textInput.value,
        },
        () => {
          if (runtime.kind === "document") runtime.setText(id, textInput.value);
        },
      );
      status("Nowy tekst dodany do planu.");
      refresh();
    } catch (error) {
      status(
        error instanceof Error
          ? error.message
          : "Nie udało się zmienić tekstu.",
        true,
      );
    }
  });

  for (const button of shadow.querySelectorAll<HTMLButtonElement>(
    "[data-mode]",
  )) {
    button.addEventListener("click", () => {
      if (!busy) setMode(button.dataset["mode"] as Mode);
    });
  }
  required("[data-parent]").addEventListener("click", () => {
    const parent = selected ? runtime.parent(selected) : undefined;
    if (parent && !busy) select(parent);
  });
  function setCollapsed(collapsed: boolean): void {
    const panel = required<HTMLElement>(".panel");
    panel.dataset["collapsed"] = String(collapsed);
    const button = required<HTMLButtonElement>("[data-collapse]");
    button.innerHTML = icon(collapsed ? "expand" : "collapse");
    button.setAttribute("aria-expanded", String(!collapsed));
    button.setAttribute(
      "aria-label",
      collapsed ? "Rozwiń panel" : "Zwiń panel",
    );
    syncCanvasInset();
    refresh();
  }
  required("[data-collapse]").addEventListener("click", () => {
    setCollapsed(
      required<HTMLElement>(".panel").dataset["collapsed"] !== "true",
    );
  });
  function undo(): void {
    if (busy) return;
    const previous = history.pop();
    if (previous) {
      actions = previous.actions;
      planVersion++;
      strokes = previous.strokes;
      selected = previous.selected;
      if (runtime.kind === "document" && previous.page)
        runtime.replace(previous.page);
      editing = null;
      status("Cofnięto ostatni gest.");
      refresh();
    }
  }
  required("[data-undo]").addEventListener("click", undo);
  required("[data-reset]").addEventListener("click", () => {
    if (busy) return;
    actions = [];
    planVersion++;
    strokes = [];
    history = [];
    selected = null;
    editing = null;
    input.value = "";
    if (runtime.kind === "document" && baseline) runtime.replace(baseline);
    status("");
    refresh();
  });

  function schematic(): string | undefined {
    if (!strokes.length) return undefined;
    const canvas = document.createElement("canvas");
    const scale = Math.min(1, 1440 / innerWidth, 1200 / innerHeight);
    canvas.width = Math.round(innerWidth * scale);
    canvas.height = Math.round(innerHeight * scale);
    const context = canvas.getContext("2d");
    if (!context) return undefined;
    context.scale(scale, scale);
    context.fillStyle = "#fff";
    context.fillRect(0, 0, innerWidth, innerHeight);
    context.font = "11px monospace";
    for (const element of runtime.elements().slice(0, 500)) {
      const rect = element.getBoundingClientRect();
      context.strokeStyle = "#b8c1bb";
      context.strokeRect(rect.x, rect.y, rect.width, rect.height);
      context.fillStyle = "#3e4941";
      context.fillText(
        `${runtime.id(element)} ${element.tagName.toLowerCase()}`,
        rect.x + 4,
        rect.y + 13,
      );
    }
    for (const line of strokes) {
      context.beginPath();
      context.strokeStyle = line.color;
      context.lineWidth = line.width;
      line.points.forEach((p, index) => {
        if (index === 0) context.moveTo(p.x - scrollX, p.y - scrollY);
        else context.lineTo(p.x - scrollX, p.y - scrollY);
      });
      context.stroke();
    }
    return canvas.toDataURL("image/png");
  }

  async function refreshAccount(): Promise<void> {
    try {
      const response = await fetch(`${endpoint}/api/state`, {
        signal: abort.signal,
      });
      if (!response.ok) return;
      const state = (await response.json()) as ServerState;
      projectMode = Boolean(state.project);
      if (!busy && actions.length === 0) revision = state.revision;
      const project = required<HTMLElement>("[data-project]");
      project.hidden = !projectMode;
      project.textContent = state.project
        ? `Mały krok · do 3 plików · ${state.project.checks.join(" · ")}`
        : "";
      input.placeholder = projectMode
        ? "Np. podłącz formularz do API i dodaj zapis w bazie."
        : "Np. więcej miejsca wokół nagłówka i krótszy tekst przycisku.";
      const link = required<HTMLAnchorElement>("[data-account]");
      link.textContent = state.account
        ? (state.account.email ?? "ChatGPT połączony")
        : "Continue with ChatGPT";
      link.title = state.account
        ? "Zaloguj ponownie do wybranego konta ChatGPT"
        : "Połącz własną subskrypcję ChatGPT";
    } catch {
      /* The visible edit request reports connectivity errors. */
    }
  }
  window.addEventListener(
    "focus",
    () => {
      void refreshAccount();
    },
    { signal: abort.signal },
  );
  void refreshAccount();

  send.addEventListener("click", () => {
    void submit();
  });
  async function submit(): Promise<void> {
    if (busy) return;
    if (!input.value.trim() && actions.length === 0) {
      status("Opisz zmianę, zaznacz element albo narysuj uwagę.");
      return;
    }
    busy = true;
    send.disabled = true;
    send.textContent = "Luna pracuje…";
    status(
      projectMode
        ? "Luna pracuje na projekcie. Przed zapisem uruchomi jego kontrole."
        : "Przekazuję stronę i gesty do GPT-6 Luna.",
    );
    started.at = Date.now();
    progress({
      phase: "context",
      label: "Przygotowuję kontekst",
      step: 0,
      total: 4,
    });
    elapsedTimer = setInterval(() => {
      required("[data-elapsed]").textContent =
        `${Math.floor((Date.now() - started.at) / 1000)} s`;
    }, 1000);
    refresh();
    try {
      const intent = {
        version: runtime.kind === "dom" ? 2 : 1,
        baseRevision: revision,
        instruction: input.value,
        page: {
          url: location.href,
          title: document.title,
          width: innerWidth,
          height: innerHeight,
          scrollX,
          scrollY,
        },
        actions,
        ...(runtime.kind === "document" ? { preview: runtime.page } : {}),
        annotation: schematic(),
      };
      const response = await fetch(
        `${endpoint}${runtime.kind === "dom" ? "/api/plugin/change" : "/api/change"}`,
        {
          signal: abort.signal,
          method: "POST",
          headers: {
            "content-type": "application/json",
            "x-ripple-request": "1",
          },
          body: JSON.stringify(intent),
        },
      );
      const result: ChangeResult =
        runtime.kind === "dom"
          ? await readChangeStream(response, progress)
          : ((await response.json()) as ChangeResult);
      if (
        !response.ok ||
        !result.revision ||
        (runtime.kind === "document" && !result.document)
      )
        throw new Error(result.error ?? "Nie udało się zapisać zmiany.");
      revision = result.revision;
      baseline = result.document ?? null;
      actions = [];
      planVersion++;
      strokes = [];
      history = [];
      selected = null;
      editing = null;
      input.value = "";
      if (runtime.kind === "document" && result.document)
        runtime.replace(result.document);
      progress({
        phase: "complete",
        label: "Zapisano · gotowe",
        step: 4,
        total: 4,
      });
      settings.onApplied?.(result);
      status(
        result.changes
          ? `${result.summary ?? "Zmiany zapisane."}\n${result.changes.length ? `Pliki: ${result.changes.map((change) => change.path).join(", ")}.` : "Bez zmian w plikach."} Kontrole: ${result.checks?.map((check) => check.name).join(", ") ?? ""}.${result.rebuildRequired ? "\nPrzebuduj kod i uruchom ponownie aplikację, aby go wczytać." : ""}`
          : `Zmiany zapisane w ${runtime.sourceFile}.`,
      );
    } catch (error) {
      required("[data-phase]").textContent = "Krok zatrzymany";
      status(
        error instanceof Error ? error.message : "Żądanie nie powiodło się.",
        true,
      );
    } finally {
      if (elapsedTimer) clearInterval(elapsedTimer);
      required<HTMLElement>("[data-progress]").dataset["active"] = "false";
      busy = false;
      send.disabled = false;
      send.innerHTML = `Zastosuj zmiany ${icon("send")}`;
      refresh();
    }
  }
  const observer = new MutationObserver(() => {
    if (!busy) refresh();
  });
  observer.observe(runtime.mount, { childList: true, subtree: true });
  setMode("select");
  setCollapsed(innerWidth < 980);
  return {
    destroy() {
      abort.abort();
      observer.disconnect();
      if (elapsedTimer) clearInterval(elapsedTimer);
      host.remove();
      runtime.mount.style.cursor = originalCursor;
      runtime.mount.style.userSelect = originalUserSelect;
      runtime.mount.style.marginRight = originalMargin;
    },
  };
}
