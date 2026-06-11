# Local Run And Release

Noet is pre-1.0 and currently optimized for local visual checkpoints, not full
installer releases. The product architecture is local-only: Markdown files are
the durable vault, SQLite is a rebuildable local index, and connectors/accounts
are deferred.

## Local Daily Run

Use this path when reviewing UX changes or using Noet directly from the repo.

```bash
cargo run -p noet-gui
```

To keep test data away from a real vault:

```bash
NOET_VAULT=/tmp/noet-review-vault cargo run -p noet-gui
```

Vault resolution order:

1. `NOET_VAULT`, for the current launch only.
2. The saved Settings vault path.
3. The default `~/Documents/NoetVault` path.

The app creates a `notes/` folder inside the vault and seeds a Welcome note when
the vault is empty. The SQLite index is stored in the OS cache directory, not in
the vault, and can be rebuilt.

## Visual Checkpoints

Before asking for interactive review, run:

```bash
cargo test --workspace
git diff --check
```

Then launch with a disposable vault and use the
[Manual Review Checklist](manual-review-checklist.md).

Visual checkpoints should verify the workspace architecture:

- switching primary surfaces does not destroy the current selection
- navigation panes can close without closing the selected work
- context and queue panes can open, close, resize, and hide responsively
- 1:1 Focus works after the People pane is closed
- Markdown task edits write back to the vault

## macOS Local Package

The macOS packaging script builds an Apple Silicon local artifact:

```bash
./scripts/package-macos.sh
```

Outputs:

- `dist/macos/Noet.app`
- `noet-v<version>-local-macos-arm64.dmg`
- `noet-v<version>-local-macos-arm64.tar.gz`

The script ad-hoc signs by default. A Developer ID is optional and should not
block local use or review. If a Developer ID identity is available, pass it with:

```bash
SIGN_IDENTITY="Developer ID Application: Example" ./scripts/package-macos.sh
```

The `.dmg` is a disk image that contains `Noet.app` and an Applications
shortcut. It is not a package installer. Open it, drag `Noet.app` to
Applications, then launch Noet. For an ad-hoc signed local build, macOS may show
the normal unidentified-developer warning; use right-click Open if needed.

## Tagged Releases

Do not tag a GitHub release just because the app builds. Tagged releases are for
stable checkpoints after the release gate in
[Implementation Roadmap](implementation-roadmap.md) passes.

During active UX architecture work, prefer:

1. commit focused implementation slices
2. run automated tests
3. launch the app locally for visual review
4. package only when an app bundle or installer checkpoint is explicitly useful

Windows and Linux installer polish is deferred until the local-only workspace
experience is stable enough to be worth distributing. The tag workflow may still
produce portable artifacts when an intentional release checkpoint is cut.
