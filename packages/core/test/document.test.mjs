import assert from "node:assert/strict";
import { test } from "node:test";
import { moveNode, validatePage } from "../dist/index.js";

const page = () => ({
  version: 1,
  title: "Test",
  root: {
    id: "root",
    tag: "main",
    children: [
      {
        id: "a",
        tag: "section",
        children: [{ id: "child", tag: "p", text: "Hello" }],
      },
      { id: "b", tag: "section" },
      { id: "c", tag: "section" },
    ],
  },
});

test("moves between siblings without mutating the original page", () => {
  const original = page();
  const next = moveNode(original, "a", "root", "c");
  assert.deepEqual(
    next.root.children.map((n) => n.id),
    ["b", "a", "c"],
  );
  assert.deepEqual(
    original.root.children.map((n) => n.id),
    ["a", "b", "c"],
  );
});

test("moves across containers and rejects cycles and invalid destinations", () => {
  const next = moveNode(page(), "child", "b", null);
  assert.equal(next.root.children[0].children.length, 0);
  assert.equal(next.root.children[1].children[0].id, "child");
  assert.throws(() => moveNode(page(), "a", "child", null));
  assert.throws(() => moveNode(page(), "root", "b", null));
  assert.throws(() => moveNode(page(), "a", "root", "missing"));
});

test("rejects duplicate IDs, scripts, executable links, and event attributes", () => {
  for (const root of [
    { id: "a", tag: "div", children: [{ id: "a", tag: "p" }] },
    { id: "a", tag: "script" },
    { id: "a", tag: "a", attributes: { href: "javascript:alert(1)" } },
    { id: "a", tag: "button", attributes: { onclick: "alert(1)" } },
  ])
    assert.throws(() => validatePage({ version: 1, title: "Test", root }));
});

test("removes a complete subtree, keeps originals and protects the root", async () => {
  const { removeNode } = await import("../dist/index.js");
  const original = page();
  const next = removeNode(original, "a");
  assert.deepEqual(
    next.root.children.map((node) => node.id),
    ["b", "c"],
  );
  assert.equal(original.root.children[0].children[0].id, "child");
  assert.throws(() => removeNode(original, "root"));
  assert.throws(() => removeNode(original, "missing"));
});
test("duplicates subtrees with fresh IDs and refuses the root", async () => {
  const { duplicateNode } = await import("../dist/index.js");
  const original = page();
  original.root.children[1].id = "ripple-copy-1";
  const next = duplicateNode(original, "a");
  const copy = next.root.children[1];
  assert.equal(copy.children[0].text, "Hello");
  assert.notEqual(copy.id, "a");
  assert.notEqual(copy.children[0].id, "child");
  assert.equal(original.root.children.length, 3);
  validatePage(duplicateNode(next, copy.id));
  assert.throws(() => duplicateNode(original, "root"));
});
test("text replacement preserves attributes and refuses to flatten child markup", async () => {
  const { setNodeText } = await import("../dist/index.js");
  const original = page();
  original.root.children[0].children[0].attributes = { title: "Keep" };
  const next = setNodeText(original, "child", "Updated");
  assert.equal(next.root.children[0].children[0].text, "Updated");
  assert.equal(next.root.children[0].children[0].attributes.title, "Keep");
  assert.equal(original.root.children[0].children[0].text, "Hello");
  assert.throws(() => setNodeText(original, "a", "Would flatten"));
  assert.throws(() => setNodeText(original, "child", "x".repeat(20_001)));
});
