# Bolt protocol v1

Transport: QUIC, TLS 1.3, ALPN `bolt/1`, mutual certificate authentication
(see security.md). Default UDP port 47600 (falls back to an ephemeral port, which
is what mDNS advertises).

Each session uses **one client-initiated bidirectional stream**.

## Framing
Control message = `u32` big-endian length ‖ UTF-8 JSON. Max 16 MiB (bounds
allocation). JSON object has `"type"` in SCREAMING_SNAKE_CASE. After `FILE_START`
the sender writes exactly `length` raw bytes — no framing, no per-chunk header.

## Messages
| Type | Direction | Notes |
|---|---|---|
| `HELLO` | both, first | `protocol`, `device_name` (display only), `device_kind`, `max_chunk_bytes`. Mismatched major → close. Also serves as CAPABILITIES in v1. |
| `PAIR_REQUEST` | initiator → responder | `proof` = keyed BLAKE3(token; initiator_id ‖ responder_id). Token never sent. |
| `PAIR_RESPONSE` | responder → initiator | `approved`, `reason`. Sent only after the responder's user compares the 6-digit code and approves. |
| `TRANSFER_OFFER` | sender → receiver | `transfer_id` (128-bit random hex), files `{index, path, size, file_id}`. |
| `TRANSFER_ACCEPT` | receiver → sender | `resume: [{index, offset, complete}]` |
| `TRANSFER_REJECT` | receiver → sender | `reason` (declined, busy, no space, invalid) |
| `FILE_START` | sender → receiver | `{index, offset, length}`; followed by `length` raw bytes |
| `FILE_COMPLETE` | sender → receiver | `blake3` of the whole file (including resumed prefix) |
| `FILE_ACK` | receiver → sender | `ok`; the file is on disk, fsynced and verified when `ok` |
| `TRANSFER_COMPLETE` | both | sender announces end; receiver echoes after cleanup |
| `TRANSFER_ERROR` | either | `code` ∈ protocol, io, disk_full, integrity, internal |
| `LINK_TEST` | initiator → responder | network speed test instead of an offer: `bytes` (≤ 1 GiB) and `direction` ∈ up, down. After HELLO, in place of `TRANSFER_OFFER`. `up`: initiator then sends `bytes` raw bytes. `down`: responder sends `bytes` raw bytes and finishes. |
| `LINK_TEST_RESULT` | responder → initiator | for `up`: bytes received and the responder's elapsed microseconds |

Not separate messages in v1: `HEARTBEAT` (QUIC keep-alive every 4 s, idle timeout
15 s), `FILE_PROGRESS` (progress is local to each side, derived from bytes moved),
`TRANSFER_START`/`FILE_METADATA` (folded into OFFER/FILE_START), `TRANSFER_CANCEL`
(cancellation is a QUIC `CONNECTION_CLOSE` with application code 1).

## Close codes
`0` done/normal · `1` cancelled by user · `2` protocol violation.

## Sequence
```
sender                                   receiver
  HELLO  ───────────────────────────────▶
         ◀─────────────────────────────── HELLO
  TRANSFER_OFFER ───────────────────────▶   (validate; user prompt unless trusted+auto)
         ◀───────────────  TRANSFER_ACCEPT{resume}  | TRANSFER_REJECT
  for each non-complete file:
    FILE_START{index,offset,length} ────▶
    <length raw bytes> ─────────────────▶   (stream to .part, hash incrementally)
    FILE_COMPLETE{blake3} ──────────────▶   (fsync, compare, rename, no overwrite)
         ◀──────────────────── FILE_ACK{ok}
  TRANSFER_COMPLETE ────────────────────▶
         ◀──────────────────── TRANSFER_COMPLETE
```

## Offer validation (receiver, before any prompt or disk access)
- `transfer_id` 32 hex chars; ≥1 and ≤ 50 000 files; indexes dense from 0.
- Each path: `/`-separated, no empty/`.`/`..` components, no `\ : * ? " < > |` or
  control chars, no Windows reserved names (CON, NUL, COM1…), no trailing dot/space,
  ≤255 bytes per component, ≤1024 total. No case-insensitive duplicates.
- `FILE_START` must match the accepted plan exactly; surplus or missing bytes fail the transfer.

## Resume
Partial data lives in `<download>/.bolt-partial/<key>.{part,json,done}` where
`key = BLAKE3(sender_id, path, size, file_id)`. A partial is reused only if the
authenticated sender id, path, size and `file_id` (size+mtime of the source) all
match the manifest. The receiver re-hashes its own prefix, the sender re-hashes
the same prefix locally, and the final whole-file hash decides success — a
corrupted partial is detected and discarded, never published. Files finished in an
earlier attempt of the same batch are marked `.done` and skipped on retry.
Partials older than 7 days are purged at startup.

## Versioning
`HELLO.protocol` is the major version. Additive changes (new optional fields,
new message types ignored by older peers only inside new majors) bump ALPN.
