# Read-only appearance snapshots (LYCO fork)

This fork adds an additive Rust API and a Windows/Linux diagnostic CLI. It
retains the upstream MIT OR Apache-2.0 licensing and backend fallback chains.
It changes neither OS preferences nor browser profiles. It is not a network,
browser fingerprint, service eligibility or anti-abuse acceptance test.

```rust
use uda_core::appearance::AppearanceManager;

// Use the platform's existing appearance manager.
let report = manager.snapshot();
```

`AppearanceManager::snapshot()` is object-safe and has a default implementation,
so existing managers need no new methods. It reports `schema_version: 1`,
appearance capability flags, theme and RGBA accent. Every field has its own
`available`, `unknown`, `unsupported` or `error` status. An unknown theme is never
guessed to be light or automatic; `auto` is a distinct available preference.
RGBA includes alpha unchanged.

Capabilities describe advertised backend support, not guaranteed current reads.
An advertised field can still fail or be unsupported in the current session.
An absent capability skips that read. If querying capabilities fails, the report
preserves the error and tries the two read-only methods independently. No field
is reported as healthy simply because another succeeds. Raw backend errors are
mapped to bounded categories (`detection`, `command`, `io`, `internal`), excluding
their text, which can contain local paths or session details.

## CLI

```text
uda-cli appearance --json
uda-cli --json
uda-cli --help
```

The default command prints the same read-only report in human-readable form.
There are no wallpaper-write, notification, wake-lock or power commands. Help
and invalid arguments are handled before constructing a backend. Invalid input
exits 2 without echoing arbitrary arguments. Exit 0 means a report was produced;
it does not certify that every field is available. Consumers must inspect each
status, validate the schema version, and impose their own child-process timeout.
The existing synchronous Linux backends can wait on IPC or command execution;
this change does not add an in-process deadline or a background event watcher.

Snapshots are sequential observations, not atomic, and do not read account
credentials, browser data, IP information, locale, timezone, or wallpaper paths.
The ABI in `include/uda.h` is unchanged; C/Python/Node FFI consumers do not acquire
this Rust snapshot method automatically. LYCO's existing pinned appearance helper
is also unchanged until this fork passes its own gate and an explicit integration
change passes the browser's product gate.

## Verification

All builds, formatting and tests run in GitHub Actions only. The preparation
workflow generates the updated lockfile and formatted sources for review; it is
not an acceptance gate. The main CI retains its lint, cross-compilation, Linux
D-Bus mock, Windows backend and FFI gates. New checks cover independent errors,
missing capabilities, unknown/auto semantics, error redaction and JSON schema.

Linux integration isolates PATH and the session bus, checks the exact allowed
read-only fallback commands and an unavailable-desktop result. Windows integration
compares with independent read-only registry queries and checks those observed
values did not change. Neither fixture tests all desktop environments or proves
anything about Claude detection. Current-run evidence is required before marking
the SDK change accepted; no test is claimed passed merely because it exists.
