import { test } from "node:test";
import assert from "node:assert/strict";
import { siblingOrder } from "../dist/operations.js";
const move = (id, parent, before) => ({
  type: "move",
  element: { id },
  parent: { id: parent },
  before: before ? { id: before } : null,
});
test("replays repeated moves, cross-parent moves and removals without mutating live siblings", () => {
  const initial = ["a", "b", "c", "d"];
  const actions = [
    move("a", "list", "c"),
    move("a", "list", "d"),
    move("outside", "list", "a"),
    { type: "remove", element: { id: "b" } },
    move("c", "other", null),
  ];
  assert.deepEqual(siblingOrder(initial, "list", actions), [
    "outside",
    "a",
    "d",
  ]);
  assert.deepEqual(initial, ["a", "b", "c", "d"]);
  assert.deepEqual(siblingOrder(["e"], "other", actions), ["e", "c"]);
  assert.deepEqual(
    siblingOrder(initial, "list", [move("a", "list", "a")]),
    initial,
  );
});
