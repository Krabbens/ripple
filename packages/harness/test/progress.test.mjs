import { test } from "node:test";
import assert from "node:assert/strict";
import { readChangeStream } from "../dist/progress.js";
function response(events, close = true) {
  const bytes = new TextEncoder().encode(
    events.map((event) => `data: ${JSON.stringify(event)}\r\n\r\n`).join(""),
  );
  return new Response(
    new ReadableStream({
      start(controller) {
        for (const byte of bytes) controller.enqueue(Uint8Array.of(byte));
        if (close) controller.close();
      },
    }),
    { headers: { "content-type": "text/event-stream" } },
  );
}
test("progress arrives before completion, including split UTF-8 and CRLF", async () => {
  const seen = [];
  const result = await readChangeStream(
    response([
      {
        type: "progress",
        progress: { phase: "model", label: "Kończę zmianę", step: 1, total: 4 },
      },
      { type: "complete", result: { revision: "new", summary: "Zapisano" } },
    ]),
    (event) => seen.push(event),
  );
  assert.equal(seen[0].label, "Kończę zmianę");
  assert.equal(result.revision, "new");
});
test("stream errors and interrupted runs cannot report success", async () => {
  await assert.rejects(
    readChangeStream(
      response([{ type: "error", error: "Kontrola nie przeszła" }]),
      () => {},
    ),
    /Kontrola/,
  );
  await assert.rejects(
    readChangeStream(
      response([{ type: "progress", progress: { phase: "model" } }]),
      () => {},
    ),
    /przerwane/,
  );
});
