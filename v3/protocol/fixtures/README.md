# Protocol fixtures

One JSON file per message shape. These are the cross-language contract of
[PROTOCOL.md](../../docs/spec/PROTOCOL.md) §10: the Rust core produces them from its real types, and
the future Flutter shell decodes the same files — so a wire change that only one side makes cannot
pass unnoticed.

## File format

Every file is one object:

```json
{
  "case": "request.hello",
  "kind": "request",
  "message": { "v": 1, "id": "1", "cmd": "hello", "args": { "protocol": 1, "client": "0.1.0" } }
}
```

- `case` — the fixture's name; it is also the file stem (`request.hello.json`). A command with more
  than one documented argument shape has one case per shape (`request.board.test.move`,
  `request.ocr.scan.count`, …).
- `kind` — `request`, `response` or `event`, so the file decodes without guessing.
- `message` — the wire object exactly as it is sent, produced by serialising the real `seal-proto`
  type (`Request`, `Response`, `Event`). `v` is always `1`.

Fixtures are pretty-printed, UTF-8, LF line endings and end with one newline; the test asserts that
and [`.gitattributes`](.gitattributes) forces `eol=lf` on checkout, because this repository is used on
Windows with `core.autocrlf=true`.

## Producer: the Rust test

`v3/core/crates/seal-proto/tests/fixtures.rs` is the only writer. A normal test run is
**compare-only**: every fixture is re-serialised from the types and compared byte for byte, so a type
change fails with the first differing line. The same run decodes every fixture back into its type
(and checks the envelope invariants: exactly one of `result`/`error`, an event has no `id`, …), and
checks that the set covers every command, error code and event the spec defines — a new protocol
shape without a fixture fails rather than shipping untested.

Regenerate after an **intended** wire change, from `v3/core`:

```powershell
$env:UPDATE_FIXTURES = "1"
.\cargo.cmd test -p seal-proto --offline
Remove-Item Env:UPDATE_FIXTURES
```

That writes every fixture from `cases()` and removes `.json` files no case names any more, so the
directory stays exact. Review the diff, and make the protocol change in one commit with
`PROTOCOL.md` and the Dart decode side. To add a shape, add a case in `fixtures.rs`, regenerate, and
commit both.

## Consumer: the Dart test

The shell's side of the same contract is decode-only and never writes these files. For each file it:

1. reads `case`/`kind`/`message`;
2. decodes `message` into the envelope type for `kind` (`Request`, `Response` or `Event`);
3. decodes the command-specific `args`/`result`/`event` payload into the shell's typed model, failing
   if a field is missing or has the wrong type.

That last step is why the payloads are full examples and not `{}`: a fixture whose `message` the
shell cannot decode is a broken contract, and the Rust test already guarantees the payload came from
the real types rather than from a hand-tended file.

Nothing here is real player data — every payload is synthetic. Never copy values from
`v2/config/local.yaml` into a fixture.
