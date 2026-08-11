# Subprocess / WASI stdio

One JSON line request → one JSON line reply (pipes or WASI stdin/stdout). See also [wasi.md](wasi.md).

**Request**

```json
{
  "schema": "kcell.envelope.v1",
  "correlationId": "corr-1",
  "timeoutMs": 3000,
  "capability": "echo-sub",
  "payload": { "ping": true }
}
```

**Reply** — same `schema` / `correlationId` / `capability`, plus `payload`.

Rules: Host closes stdin after one line; Cell prints one line and exits `0`; timeout kills the process.

Demo: `kcell worker`. Manifest uses `runtime.kind: subprocess` with `entrypoint` / `artifact`.
