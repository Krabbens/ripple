import {
  findNode,
  moveNode,
  parentOf,
  removeNode,
  duplicateNode,
  setNodeText,
  validatePage,
} from "./document.js";
import type { ElementReference, RippleNode, RipplePage } from "./document.js";

export class RippleRuntime {
  readonly kind = "document" as const;
  readonly mount: HTMLElement;
  readonly sourceFile: string;
  #page: RipplePage;
  #elements = new Map<string, HTMLElement>();

  constructor(
    mount: HTMLElement,
    page: RipplePage,
    sourceFile = "ripple.page.json",
  ) {
    this.mount = mount;
    this.sourceFile = sourceFile;
    this.#page = structuredClone(page);
    this.replace(page);
  }

  get page(): RipplePage {
    return structuredClone(this.#page);
  }
  element(id: string): HTMLElement | undefined {
    return this.#elements.get(id);
  }

  id(element: HTMLElement): string {
    return element.dataset["rippleId"]!;
  }
  target(target: EventTarget | null): HTMLElement | undefined {
    const element =
      target instanceof Element
        ? target.closest<HTMLElement>("[data-ripple-id]")
        : null;
    return element && this.mount.contains(element) ? element : undefined;
  }
  parent(id: string): string | undefined {
    return parentOf(this.#page.root, id)?.id;
  }
  elements(): HTMLElement[] {
    return [...this.#elements.values()];
  }

  replace(page: RipplePage): void {
    validatePage(page);
    this.#page = structuredClone(page);
    this.#elements.clear();
    const render = (node: RippleNode): HTMLElement => {
      const element = document.createElement(node.tag);
      element.id = node.id;
      element.dataset["rippleId"] = node.id;
      for (const [key, value] of Object.entries(node.styles ?? {}))
        element.style.setProperty(key, value);
      for (const [key, value] of Object.entries(node.attributes ?? {}))
        element.setAttribute(key, value);
      if (node.text) element.append(document.createTextNode(node.text));
      for (const child of node.children ?? []) element.append(render(child));
      this.#elements.set(node.id, element);
      return element;
    };
    this.mount.replaceChildren(render(this.#page.root));
    document.title = page.title;
    this.mount.dispatchEvent(new CustomEvent("ripple:render"));
  }

  move(id: string, parentId: string, beforeId: string | null): void {
    this.replace(moveNode(this.#page, id, parentId, beforeId));
  }

  remove(id: string): void {
    this.replace(removeNode(this.#page, id));
  }
  duplicate(id: string): void {
    this.replace(duplicateNode(this.#page, id));
  }
  setText(id: string, text: string): void {
    this.replace(setNodeText(this.#page, id, text));
  }

  reference(id: string): ElementReference {
    const element = this.element(id);
    const node = findNode(this.#page.root, id);
    if (!element || !node) throw new Error("Element już nie istnieje.");
    const rect = element.getBoundingClientRect();
    return {
      id,
      tag: node.tag,
      text: (element.textContent ?? "").slice(0, 1000),
      bounds: {
        x: rect.x + scrollX,
        y: rect.y + scrollY,
        width: rect.width,
        height: rect.height,
      },
      source: { file: this.sourceFile, nodeId: id },
    };
  }
}
