import { test } from "node:test";
import assert from "node:assert/strict";
import { Window } from "happy-dom";
import { DomSurface, RippleRuntime } from "@ripple/core";
import { mountHarness } from "../dist/index.js";

// An isolated DOM model: no real browser, navigation, accounts or network requests.
function fixture(t, { legacy = false, failure = false } = {}) {
  const window = new Window({
    url: "https://fixture.invalid",
    width: 1200,
    height: 900,
  });
  const document = window.document;
  document.body.innerHTML =
    '<div id="app"><section id="items" data-ripple-source="src/app.tsx"><h1 id="alpha">Alpha</h1><p id="beta">Beta</p><p id="gamma">Gamma</p><p id="delta">Delta</p></section></div>';
  const names = [
    "window",
    "document",
    "HTMLElement",
    "Element",
    "MutationObserver",
    "CustomEvent",
    "AbortController",
    "innerWidth",
    "innerHeight",
    "scrollX",
    "scrollY",
    "location",
  ];
  const descriptors = new Map(
    [...names, "fetch", "getComputedStyle"].map((name) => [
      name,
      Object.getOwnPropertyDescriptor(globalThis, name),
    ]),
  );
  for (const name of names)
    Object.defineProperty(globalThis, name, {
      value: name === "window" ? window : window[name],
      configurable: true,
      writable: true,
    });
  globalThis.getComputedStyle = window.getComputedStyle.bind(window);
  const sent = [];
  const applied = [];
  let pending;
  globalThis.fetch = async (url, options) => {
    if (url.endsWith("/api/state"))
      return Response.json({
        revision: "initial",
        account: null,
        project: { sources: ["src"], checks: ["test"] },
        model: "gpt-6-luna",
        effort: "low",
      });
    sent.push(JSON.parse(options.body));
    return new Promise((resolve) => {
      pending = resolve;
    });
  };
  const mount = document.getElementById("app");
  const runtime = legacy
    ? new RippleRuntime(
        mount,
        {
          version: 1,
          title: "Fixture",
          root: {
            id: "items",
            tag: "section",
            children: [
              { id: "alpha", tag: "h1", text: "Alpha" },
              { id: "beta", tag: "p", text: "Beta" },
              { id: "gamma", tag: "p", text: "Gamma" },
              { id: "delta", tag: "p", text: "Delta" },
            ],
          },
        },
        "ripple.page.json",
      )
    : new DomSurface(mount, "src/app.tsx");
  const harness = mountHarness(runtime, "initial", {
    onApplied: (result) => applied.push(result),
  });
  const shadow = document.querySelector("ripple-devtools").shadowRoot;
  const control = (selector) => {
    const element = shadow.querySelector(selector);
    assert.ok(element, selector);
    return element;
  };
  const choose = (id) => {
    const element = document.getElementById(id);
    element.dispatchEvent(
      new window.PointerEvent("pointerdown", {
        bubbles: true,
        composed: true,
        button: 0,
      }),
    );
    element.dispatchEvent(
      new window.PointerEvent("pointerup", {
        bubbles: true,
        composed: true,
        button: 0,
      }),
    );
  };
  const key = (key, options = {}, target = window) => {
    const event = new window.KeyboardEvent("keydown", {
      key,
      bubbles: true,
      composed: true,
      cancelable: true,
      ...options,
    });
    target.dispatchEvent(event);
    return event;
  };
  const plan = () =>
    [...control("[data-plan]").children].map(
      (element) => element.dataset.operation,
    );
  const tick = async () => {
    await new Promise((resolve) => setImmediate(resolve));
  };
  const finish = async (fail = failure) => {
    const event = fail
      ? { type: "error", error: "Kontrola nie przeszła" }
      : {
          type: "complete",
          result: {
            revision: "next",
            ...(legacy ? { document: sent.at(-1).preview } : {}),
            summary: "Zapisano",
            changes: [{ path: "src/app.tsx", kind: "modified" }],
            checks: [{ name: "test", success: true }],
          },
        };
    const body = legacy
      ? JSON.stringify(event.result ?? { error: event.error })
      : `data: ${JSON.stringify(event)}\n\n`;
    pending(
      new Response(body, {
        headers: {
          "content-type": legacy ? "application/json" : "text/event-stream",
        },
      }),
    );
    await tick();
  };
  t.after(async () => {
    harness.destroy();
    await window.happyDOM.close();
    for (const [name, descriptor] of descriptors) {
      if (descriptor) Object.defineProperty(globalThis, name, descriptor);
      else delete globalThis[name];
    }
  });
  return {
    window,
    document,
    mount,
    runtime,
    harness,
    control,
    choose,
    key,
    plan,
    sent,
    applied,
    finish,
    tick,
  };
}

test("delete is reversible, marks the element and leaves the host DOM untouched", async (t) => {
  const f = fixture(t);
  const original = f.document.getElementById("alpha");
  f.choose("alpha");
  assert.equal(f.key("Delete").defaultPrevented, true);
  assert.deepEqual(f.plan(), ["remove"]);
  assert.equal(f.document.getElementById("alpha"), original);
  assert.equal(f.control(".removal-mark span").textContent, "Do usunięcia");
  assert.equal(f.control('[data-element-action="remove"]').disabled, true);
  f.key("Delete", { repeat: true });
  assert.deepEqual(f.plan(), ["remove"]);
  f.key("z", { metaKey: true });
  assert.deepEqual(f.plan(), []);
  assert.equal(f.control(".planned-layer").children.length, 0);
  assert.equal(f.key("Backspace").defaultPrevented, true);
  f.control("[data-send]").click();
  await f.tick();
  assert.equal(f.sent[0].version, 2);
  assert.equal(f.sent[0].instruction, "");
  assert.equal(f.sent[0].actions.at(-1).type, "remove");
  assert.equal(f.sent[0].actions.at(-1).element.source.file, "src/app.tsx");
  assert.equal(f.control('[data-element-action="duplicate"]').disabled, true);
  await f.finish();
  assert.equal(f.applied.length, 1);
  assert.deepEqual(f.plan(), []);
  assert.equal(f.control(".planned-layer").children.length, 0);
});

test("successive reorder buttons and duplicate shortcuts carry precise placement to the bridge", async (t) => {
  const f = fixture(t);
  f.choose("alpha");
  f.control('[data-element-action="later"]').click();
  f.control('[data-element-action="later"]').click();
  f.control('[data-element-action="earlier"]').click();
  assert.equal(f.key("d", { ctrlKey: true }).defaultPrevented, true);
  assert.deepEqual(f.plan(), ["move", "move", "move", "duplicate"]);
  assert.deepEqual(
    [...f.document.getElementById("items").children].map((el) => el.id),
    ["alpha", "beta", "gamma", "delta"],
  );
  f.control("[data-send]").click();
  await f.tick();
  const actions = f.sent[0].actions.filter(
    (action) => action.type !== "select",
  );
  assert.deepEqual(
    actions.map((action) => action.before.source.nodeId),
    ["gamma", "delta", "gamma", "gamma"],
  );
  assert.ok(actions.every((action) => action.parent.source.nodeId === "items"));
  await f.finish();
});

test("text editing never flattens child markup and shortcuts respect text controls", async (t) => {
  const f = fixture(t);
  f.choose("items");
  assert.equal(f.control("[data-edit-text]").disabled, true);
  f.choose("alpha");
  f.control("[data-edit-text]").click();
  const field = f.control("#ripple-element-text");
  assert.equal(field.value, "Alpha");
  assert.equal(f.key("Delete", {}, field).defaultPrevented, false);
  assert.deepEqual(f.plan(), []);
  field.value = "Nowy tekst";
  f.control("[data-save-text]").click();
  assert.equal(f.document.getElementById("alpha").textContent, "Alpha");
  assert.deepEqual(f.plan(), ["setText"]);
  const instruction = f.control("#ripple-instruction");
  f.key("z", { metaKey: true }, instruction);
  assert.deepEqual(f.plan(), ["setText"]);
  f.control("[data-edit-text]").click();
  assert.equal(field.value, "Nowy tekst");
  f.control("[data-cancel-text]").click();
  assert.equal(
    f.control("[data-edit-text]"),
    f.control(".panel").getRootNode().activeElement,
  );
  f.control("[data-send]").click();
  await f.tick();
  assert.equal(f.sent[0].actions.at(-1).text, "Nowy tekst");
  await f.finish();
});

test("failed checks keep the deletion plan; reset, protected roots and cleanup stay usable", async (t) => {
  const f = fixture(t);
  const original = f.document.getElementById("alpha");
  f.choose("app");
  assert.equal(f.control('[data-element-action="remove"]').disabled, true);
  assert.equal(f.control('[data-element-action="duplicate"]').disabled, true);
  f.choose("alpha");
  f.control('[data-element-action="remove"]').click();
  f.control("[data-send]").click();
  await f.tick();
  await f.finish(true);
  assert.deepEqual(f.plan(), ["remove"]);
  assert.match(f.control(".status").textContent, /Kontrola nie przeszła/);
  assert.equal(f.document.getElementById("alpha"), original);
  f.control("[data-reset]").click();
  assert.deepEqual(f.plan(), []);
  assert.equal(f.control(".planned-layer").children.length, 0);
  f.harness.destroy();
  assert.equal(f.document.querySelector("ripple-devtools"), null);
  assert.equal(f.key("Delete").defaultPrevented, false);
  assert.equal(f.document.getElementById("alpha"), original);
});

test("the compatible JSON preview can duplicate, remove and undo subtrees", (t) => {
  const f = fixture(t, { legacy: true });
  f.choose("alpha");
  f.control('[data-element-action="duplicate"]').click();
  assert.equal(f.runtime.page.root.children.length, 5);
  f.control('[data-element-action="remove"]').click();
  assert.equal(f.runtime.page.root.children.length, 4);
  assert.equal(f.document.getElementById("alpha"), null);
  f.control("[data-undo]").click();
  assert.ok(f.document.getElementById("alpha"));
  f.control("[data-undo]").click();
  assert.equal(f.runtime.page.root.children.length, 4);
  assert.deepEqual(f.plan(), []);
});
