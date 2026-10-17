import type { ElementReference, VisualAction } from "@ripple/core";

/** Replay source moves against live siblings; repeated clicks can advance several places. */
export function siblingOrder(
  initial: string[],
  parentId: string,
  actions: VisualAction[],
): string[] {
  const order = [...initial];
  for (const action of actions) {
    if (action.type !== "move" && action.type !== "remove") continue;
    if (
      action.type === "move" &&
      action.parent.id === parentId &&
      action.before?.id === action.element.id
    )
      continue;
    const index = order.indexOf(action.element.id);
    if (index !== -1) order.splice(index, 1);
    if (action.type === "move" && action.parent.id === parentId) {
      const before = action.before ? order.indexOf(action.before.id) : -1;
      order.splice(before < 0 ? order.length : before, 0, action.element.id);
    }
  }
  return order;
}
export function elementLabel(element: ElementReference): string {
  const text = element.text.replace(/\s+/g, " ").trim();
  return `${element.tag}${text ? ` · ${text.slice(0, 46)}${text.length > 46 ? "…" : ""}` : ` #${element.source.nodeId}`}`;
}
export function actionLabel(action: VisualAction): string | undefined {
  switch (action.type) {
    case "remove":
      return `Usuń ${elementLabel(action.element)}`;
    case "duplicate":
      return `Duplikuj ${elementLabel(action.element)}`;
    case "setText":
      return `Tekst ${action.element.tag} → „${action.text.replace(/\s+/g, " ").slice(0, 60)}”`;
    case "move":
      return `Przenieś ${elementLabel(action.element)}`;
    default:
      return undefined;
  }
}
