# Bolt security model

## Assets and adversaries
Protect: file contents in transit, the receiver's filesystem, the user's consent.
Adversaries: another device on the same (possibly hostile, e.g. café) Wi-Fi that
can sniff, replay, spoof mDNS and connect to open ports; a malicious paired
device; a malicious file name/offer.

## Controls
| Threat | Control | Test |
|---|---|---|
| Eavesdropping | QUIC/TLS 1.3 only, ECDSA P-256 certs, ALPN pinned | transport |
| MITM / spoofed mDNS | peer identity = hash of its certificate; senders pin the expected id; pairing binds ids with a QR-delivered secret | `wrong_identity_pin_aborts_before_sending_anything` |
| Pairing guess | 128-bit single-use token, 5-minute TTL, locked after 3 bad proofs, never sent on the wire (keyed proof), plus user-visible 6-digit code on both screens | `pairing_*`, `pairing::tests` |
| QR leaks secrets | QR holds device id, address, short-lived token only — no private key, no long-lived secret | `PairingUri` |
| Unknown sender | every offer from an unpaired device requires explicit accept; unanswered offers time out as declined; auto-accept applies only to paired devices and only if enabled | `trusted_sender_skips_prompt_only_when_enabled`, `unanswered_offer_times_out_and_is_declined` |
| Path traversal / reserved names | validated at arrival *and* re-checked at finalisation | `hostile_offers_are_rejected_without_touching_disk`, `fsx::tests` |
| Symlink escape | every existing directory component is checked with `symlink_metadata`; canonical parent must stay under the root; sender never follows symlinks | `fsx::tests::symlinked_directory_cannot_redirect_writes` (Unix), `scan` |
| Overwrite | unique-name reservation with `create_new`; originals are never replaced | `finalize_never_overwrites`, `concurrent_transfers_are_isolated` |
| Partial/corrupt data published | `.part` file + whole-file BLAKE3 + fsync, then rename; mismatch deletes the partial | `wrong_hash_…`, `corrupted_partial_…` |
| Memory exhaustion | frames ≤16 MiB, ≤50 000 files, one chunk buffer per transfer, QUIC windows capped, ≤16 concurrent connections, bounded stream count | `framing::tests` |
| Surplus data after FILE_START | stream is parsed strictly; extra bytes fail the transfer | `data_beyond_announced_length_…` |
| Code execution | received files are never executed or auto-opened | by design |
| Telemetry | none; logs redact keys (`Identity: Debug` prints `<redacted>`) | `identity::tests` |

## Trust model details
TLS proves "the peer holds the private key for certificate C". Authorisation is a
separate, explicit step (pairing). The TLS verifier therefore accepts any
self-signed certificate (as SSH accepts unknown host keys) but still verifies the
handshake signature, then the application compares the certificate hash with the
pinned/trusted id. A device that merely connects can do nothing except submit an
offer that a human must accept, or a pairing proof that needs the QR token.

## Known limitations (be honest)
- **Private-key storage**: written to the app data dir (owner-only perms on Unix;
  per-user profile ACL on Windows). Planned: DPAPI on Windows, Android Keystore
  wrapping on Android. Not yet implemented.
- **Pre-auth connection slots**: anyone on the LAN can open up to 16 QUIC
  connections that linger briefly; there is no per-IP rate limit yet.
- **Unpaired offers** show the sender's *claimed* name; the device id is
  authentic but unfamiliar. UIs must show the short id and "unknown device".
- **No TCP fallback**: networks that block UDP cannot be used (explicit failure).
- mDNS advertises device name, kind and id to the local network by design.
- Not externally audited.
