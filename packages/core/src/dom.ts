import type { ElementReference } from "./document.js";

/** Observe the host framework's DOM without rendering or reparenting it. */
export class DomSurface {
  readonly kind = "dom" as const;
  readonly mount: HTMLElement;
  readonly sourceFile: string;
  #ids = new WeakMap<HTMLElement, string>();
  #elements = new Map<string, HTMLElement>();
  #next = 0;
  constructor(mount = document.body, sourceFile = "") {
    this.mount = mount;
    this.sourceFile = sourceFile;
  }
  target(target: EventTarget | null): HTMLElement | undefined {
    let element = target instanceof Element ? target : null;
    while (element && !(element instanceof HTMLElement))
      element = element.parentElement;
    return element instanceof HTMLElement &&
      this.mount.contains(element) &&
      !element.closest(
        "ripple-devtools, astro-dev-toolbar, nextjs-portal, script, style, link, meta, [data-ripple-ignore]",
      )
      ? element
      : undefined;
  }
  id(element: HTMLElement): string {
    let id = this.#ids.get(element);
    if (!id) {
      id = `dom-${++this.#next}`;
      this.#ids.set(element, id);
      this.#elements.set(id, element);
    }
    return id;
  }
  element(id: string): HTMLElement | undefined {
    const element = this.#elements.get(id);
    if (element && this.mount.contains(element)) return element;
    this.#elements.delete(id);
    return undefined;
  }
  parent(id: string): string | undefined {
    const element = this.target(this.element(id)?.parentElement ?? null);
    return element ? this.id(element) : undefined;
  }
  elements(): HTMLElement[] {
    return Array.from(this.mount.querySelectorAll<HTMLElement>("*")).filter(
      (element) => this.target(element) === element,
    );
  }
  reference(id: string): ElementReference {
    const element = this.element(id);
    if (!element)
      throw new Error("Element już nie istnieje. Zaznacz go ponownie.");
    const rect = element.getBoundingClientRect();
    const source = element.closest<HTMLElement>("[data-ripple-source]");
    return {
      id,
      tag: element.tagName.toLowerCase(),
      text: (element.textContent ?? "").slice(0, 1000),
      bounds: {
        x: rect.x + scrollX,
        y: rect.y + scrollY,
        width: rect.width,
        height: rect.height,
      },
      source: {
        file: source?.dataset["rippleSource"] ?? this.sourceFile,
        nodeId: element.id || id,
      },
    };
  }
}
