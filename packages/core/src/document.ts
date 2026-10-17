export interface RippleNode {
  id: string;
  tag: string;
  text?: string;
  styles?: Record<string, string>;
  attributes?: Record<string, string>;
  children?: RippleNode[];
}

export interface RipplePage {
  version: 1;
  title: string;
  root: RippleNode;
}

export interface Point {
  x: number;
  y: number;
}
export interface Rect extends Point {
  width: number;
  height: number;
}
export interface ElementReference {
  id: string;
  tag: string;
  text: string;
  bounds: Rect;
  source: { file: string; nodeId: string };
}
export interface Stroke {
  points: Point[];
  color: string;
  width: number;
}
export type VisualAction =
  | { type: "select"; element: ElementReference }
  | {
      type: "move";
      element: ElementReference;
      parent: ElementReference;
      before: ElementReference | null;
    }
  | { type: "remove"; element: ElementReference }
  | {
      type: "duplicate";
      element: ElementReference;
      parent: ElementReference;
      before: ElementReference | null;
    }
  | { type: "setText"; element: ElementReference; text: string }
  | { type: "draw"; stroke: Stroke };

export interface VisualIntent {
  version: 1;
  baseRevision: string;
  instruction: string;
  page: {
    url: string;
    title: string;
    width: number;
    height: number;
    scrollX: number;
    scrollY: number;
  };
  actions: VisualAction[];
  preview: RipplePage;
  /** PNG schematic of element bounds and drawing strokes; not a screenshot. */
  annotation?: string;
}

export const tags = new Set([
  "div",
  "section",
  "header",
  "nav",
  "main",
  "footer",
  "article",
  "aside",
  "h1",
  "h2",
  "h3",
  "h4",
  "h5",
  "h6",
  "p",
  "a",
  "button",
  "span",
  "ul",
  "ol",
  "li",
  "img",
  "input",
  "label",
  "form",
  "textarea",
  "strong",
  "em",
  "small",
  "hr",
  "br",
]);
export const attributes = new Set([
  "href",
  "src",
  "alt",
  "type",
  "placeholder",
  "title",
  "aria-label",
  "role",
  "name",
  "value",
]);

export function validatePage(page: RipplePage): void {
  if (
    page.version !== 1 ||
    typeof page.title !== "string" ||
    page.title.length > 300
  )
    throw new Error("Nieprawidłowy dokument Ripple.");
  const ids = new Set<string>();
  function visit(node: RippleNode, depth: number): void {
    if (depth > 32 || ids.size >= 500)
      throw new Error("Strona przekracza limit prototypu.");
    if (
      typeof node.id !== "string" ||
      !/^[\w-]{1,80}$/.test(node.id) ||
      ids.has(node.id)
    )
      throw new Error("Elementy muszą mieć unikalne ID.");
    ids.add(node.id);
    if (!tags.has(node.tag)) throw new Error("Nieobsługiwany znacznik HTML.");
    if (
      ["img", "input", "hr", "br"].includes(node.tag) &&
      (node.text || node.children?.length)
    )
      throw new Error("Ten element nie może mieć zawartości.");
    if (
      node.text !== undefined &&
      (typeof node.text !== "string" || node.text.length > 20_000)
    )
      throw new Error("Nieprawidłowa treść elementu.");
    const styles = Object.entries(node.styles ?? {});
    const attrs = Object.entries(node.attributes ?? {});
    if (styles.length > 80 || attrs.length > 20)
      throw new Error("Za dużo właściwości elementu.");
    for (const [key, value] of styles) {
      if (
        !/^[a-z-]+$/.test(key) ||
        typeof value !== "string" ||
        value.length > 2_000
      )
        throw new Error("Nieprawidłowa właściwość CSS.");
    }
    for (const [key, value] of attrs) {
      if (
        !attributes.has(key) ||
        typeof value !== "string" ||
        value.length > 2_000
      )
        throw new Error("Nieobsługiwany atrybut.");
      if (
        (key === "href" || key === "src") &&
        (/[\s\x00-\x1f\x7f]/.test(value) ||
          (value.includes(":") && !/^https?:\/\//.test(value)))
      )
        throw new Error("Nieobsługiwany URL.");
    }
    for (const child of node.children ?? []) visit(child, depth + 1);
  }
  visit(page.root, 0);
}

export function findNode(root: RippleNode, id: string): RippleNode | undefined {
  if (root.id === id) return root;
  for (const child of root.children ?? []) {
    const found = findNode(child, id);
    if (found) return found;
  }
  return undefined;
}

export function parentOf(root: RippleNode, id: string): RippleNode | undefined {
  if (root.children?.some((child) => child.id === id)) return root;
  for (const child of root.children ?? []) {
    const found = parentOf(child, id);
    if (found) return found;
  }
  return undefined;
}

/** Pure tree operation shared by the runtime and its undo history. */
export function moveNode(
  page: RipplePage,
  id: string,
  parentId: string,
  beforeId: string | null,
): RipplePage {
  validatePage(page);
  const next = structuredClone(page);
  const node = findNode(next.root, id);
  const from = parentOf(next.root, id);
  const to = findNode(next.root, parentId);
  if (!node || !from?.children || !to || findNode(node, parentId))
    throw new Error("Nie można przenieść elementu w to miejsce.");
  if (["img", "input", "hr", "br"].includes(to.tag))
    throw new Error("Ten element nie przyjmuje dzieci.");
  if (beforeId === id && from === to) return next;
  if (beforeId !== null && !to.children?.some((child) => child.id === beforeId))
    throw new Error("Nieprawidłowe miejsce wstawienia.");
  from.children.splice(from.children.indexOf(node), 1);
  to.children ??= [];
  const index =
    beforeId === null
      ? to.children.length
      : to.children.findIndex((child) => child.id === beforeId);
  to.children.splice(index, 0, node);
  validatePage(next);
  return next;
}

/** Remove a subtree without allowing the document root to disappear. */
export function removeNode(page: RipplePage, id: string): RipplePage {
  validatePage(page);
  const next = structuredClone(page);
  const parent = parentOf(next.root, id);
  if (!parent?.children) throw new Error("Nie można usunąć korzenia strony.");
  parent.children = parent.children.filter((child) => child.id !== id);
  validatePage(next);
  return next;
}

/** Clone a subtree with fresh IDs; keep the original and its descendants unchanged. */
export function duplicateNode(page: RipplePage, id: string): RipplePage {
  validatePage(page);
  const next = structuredClone(page);
  const parent = parentOf(next.root, id);
  const node = findNode(next.root, id);
  if (!parent?.children || !node)
    throw new Error("Nie można powielić korzenia strony.");
  const copy = structuredClone(node);
  let sequence = 0;
  const assign = (node: RippleNode): void => {
    do {
      node.id = `ripple-copy-${++sequence}`;
    } while (findNode(next.root, node.id));
    for (const child of node.children ?? []) assign(child);
  };
  assign(copy);
  parent.children.splice(parent.children.indexOf(node) + 1, 0, copy);
  validatePage(next);
  return next;
}

export function setNodeText(
  page: RipplePage,
  id: string,
  text: string,
): RipplePage {
  validatePage(page);
  const next = structuredClone(page);
  const node = findNode(next.root, id);
  if (
    !node ||
    node.children?.length ||
    ["img", "input", "hr", "br"].includes(node.tag)
  )
    throw new Error("Wybierz element z samym tekstem.");
  node.text = text;
  validatePage(next);
  return next;
}
