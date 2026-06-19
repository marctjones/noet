# M5 Release Evidence

Date: 2026-06-19
Validated code commit: `aab21d6`
Branch: `main`
Tracker: #57, #74

## Automated Release Gate

Command:

```sh
CARGO_BUILD_JOBS=2 scripts/release-smoke.sh
```

Result: passed.

Coverage included:

- `cargo fmt --all -- --check`
- `CARGO_BUILD_JOBS=2 cargo test --workspace`
- `CARGO_BUILD_JOBS=2 cargo check -p noet-gui`
- `git diff --check`
- Inline `mistral.rs` GUI build coverage through the default embedded runtime
  path.

Local model-backed smokes were intentionally not run in this pass. The default
release gate does not load local models; model-backed validation remains an
explicit opt-in gate so normal release QA does not create avoidable memory
pressure.

## Visual GUI Review

Command:

```sh
CARGO_BUILD_JOBS=2 scripts/build-visual-review-app.sh
open 'target/noet-visual-review/Noet Visual Review.app'
```

Result: visual review app built and launched.

Review environment:

- Vault: `target/noet-demo-vault`
- Trace: `/tmp/noet-ui-review.jsonl`
- AI runtime: `preview`
- IPC/tray: disabled
- Trace content logging: enabled

Observed current workflow:

- The app launched into the Notes workspace with the current 1:1 note selected.
- The primary editor remained the dominant surface for note taking.
- Current-note todos appeared beside the active note.
- Accessibility metadata exposed the search field, workspace tabs, pane toggles,
  todo rows, checkboxes, editor, and note actions.
- UI trace confirmed startup, workspace setup, disabled IPC/tray setup, indexing,
  and the selected current note.

Tracked follow-up:

- #75: improve current-note todo rail readability and full todo drill-in. The
  live review showed that long todo titles still truncate in the narrow todo
  rail, which makes some actions harder to scan while editing.

## macOS Package Gate

Command:

```sh
CARGO_BUILD_JOBS=2 scripts/package-macos.sh
```

Result: the release app compiled, bundled, and signed successfully. The final
DMG creation step failed inside the sandbox with `hdiutil: create failed -
Device not configured`, so the already-built bundle was packaged with `hdiutil`
outside the sandbox and then verified.

Package evidence:

- App bundle: `dist/macos/Noet.app`
- Bundle version: `0.7.0-local`
- Bundle identifier: `cl.skpt.noet`
- Signing verification: `codesign --verify --deep --strict --verbose=2
  dist/macos/Noet.app` passed.
- DMG: `noet-v0.7.0-local-macos-arm64.dmg` (`36M`)
- DMG verification: `hdiutil verify
  noet-v0.7.0-local-macos-arm64.dmg` passed.
- Tarball: `noet-v0.7.0-local-macos-arm64.tar.gz` (`30M`)

The release package build compiled the embedded `mistralrs` and `metal` crates,
confirming the packaged macOS GUI uses the inline native local-AI runtime path
instead of a CLI bridge.

## M5 Disposition

M5 release readiness is complete. Remaining issues found during review are
tracked outside M5 because they are product polish rather than release blockers.
