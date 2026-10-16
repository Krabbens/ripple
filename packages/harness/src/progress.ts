export interface ProgressEvent {
  phase: string;
  label: string;
  step: number;
  total: number;
}
export interface ChangeResult {
  document?: import("@ripple/core").RipplePage;
  revision?: string;
  error?: string;
  summary?: string;
  changes?: { path: string; kind: string }[];
  checks?: { name: string; success: boolean }[];
  rebuildRequired?: boolean;
}

/** Incremental UTF-8/SSE decoding also handles boundaries inside Polish characters. */
export async function readChangeStream(
  response: Response,
  onProgress: (event: ProgressEvent) => void,
): Promise<ChangeResult> {
  if (
    !response.ok ||
    !response.headers.get("content-type")?.includes("text/event-stream")
  ) {
    const result = (await response.json()) as ChangeResult;
    throw new Error(result.error ?? "Nie udało się uruchomić zmiany.");
  }
  if (!response.body) throw new Error("Brak strumienia postępu.");
  const reader = response.body.getReader();
  const decoder = new TextDecoder();
  let buffer = "";
  try {
    while (true) {
      const { value, done } = await reader.read();
      buffer += decoder.decode(value, { stream: !done }).replace(/\r/g, "");
      if (buffer.length > 3_000_000)
        throw new Error("Strumień odpowiedzi jest za duży.");
      let boundary: number;
      while ((boundary = buffer.indexOf("\n\n")) >= 0) {
        const block = buffer.slice(0, boundary);
        buffer = buffer.slice(boundary + 2);
        const data = block
          .split("\n")
          .filter((line) => line.startsWith("data:"))
          .map((line) => line.slice(5).trimStart())
          .join("\n");
        if (!data) continue;
        const event = JSON.parse(data) as {
          type: string;
          progress: ProgressEvent;
          result: ChangeResult;
          error: string;
        };
        if (event.type === "progress") onProgress(event.progress);
        if (event.type === "error") throw new Error(event.error);
        if (event.type === "complete") return event.result;
      }
      if (done)
        throw new Error(
          "Połączenie przerwane. Sprawdź stan projektu przed ponowieniem.",
        );
    }
  } finally {
    await reader.cancel();
    reader.releaseLock();
  }
}
