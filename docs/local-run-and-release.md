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

For live accessibility or Computer Use inspection on macOS, build and open the
review app bundle. It has a unique bundle id so it does not collide with an
installed Noet app:

```bash
scripts/build-visual-review-app.sh
open "target/noet-visual-review/Noet Visual Review.app"
```

For GUI workflow debugging, enable a local JSONL trace:

```bash
NOET_DISABLE_IPC=1 NOET_DISABLE_TRAY=1 NOET_UI_TRACE=/tmp/noet-ui-trace.jsonl NOET_VAULT=/tmp/noet-review-vault cargo run -p noet-gui
```

Add `NOET_UI_TRACE_CONTENT=1` only when visible note/search excerpts are needed
to explain what the reviewer saw.

`NOET_DISABLE_IPC=1` keeps a review launch from forwarding to an already running
installed Noet instance, which makes live visual inspection target the build
under review. `NOET_DISABLE_TRAY=1` removes macOS tray/global-hotkey setup from
the visual pass so accessibility inspection can target the main window directly.
The review app bundle applies those flags automatically and runs with
`NOET_AI_RUNTIME=preview` so no local model is loaded during visual checks.

Visual checkpoints should verify the workspace architecture:

- switching primary surfaces does not destroy the current selection
- navigation panes can close without closing the selected work
- context and queue panes can open, close, resize, and hide responsively
- 1:1 Focus works after the People pane is closed
- Markdown task edits write back to the vault
- trace logs identify the activated callback, app command, command outcome,
  status/error text, and pane state for any behavior under investigation

## Local AI Release Gate

Normal CI stays deterministic and must not load local models:

```bash
cargo test --workspace
git diff --check
```

Normal GUI builds include the inline `mistral.rs` runtime. The compile gate
should verify the normal embedded-library path:

```bash
cargo check -p noet-gui
```

The deterministic release gate is wrapped by:

```bash
scripts/release-smoke.sh
```

By default the script runs formatting, workspace tests, the normal embedded
`mistral.rs` GUI compile check, an auto-detected Metal compile check when the
Xcode Metal compiler is present, and whitespace checks. It does not load local
AI models or build installers unless explicitly requested.

The Apple Silicon Metal/Accelerate check requires the full Xcode Metal compiler
(`xcrun -f metal`), not only the default command-line tools. If the compiler is
missing, the release smoke script safely continues with the embedded CPU
runtime. To require Metal and fail when it is missing:

```bash
NOET_CHECK_METAL=1 scripts/release-smoke.sh
```

Model-backed smokes are ignored by default because they load local models. Run
them only on a prepared machine after checking memory pressure:

```bash
memory_pressure
cargo test -p noet-gui headless_ui_local_model_ai_smoke -- --ignored --nocapture
cargo test -p noet-gui headless_ui_local_model_cancel_smoke -- --ignored --nocapture
cargo test -p noet-gui headless_ui_local_embedding_refresh_smoke -- --ignored --nocapture
```

The same model-backed checks can be run through the release smoke script:

```bash
NOET_RUN_LOCAL_MODEL_SMOKES=1 scripts/release-smoke.sh
```

When Metal is installed, these smokes exercise the embedded Noet runtime with
`mistralrs-inline-metal`, which is the path to use for local benchmark numbers
on Apple Silicon.

Expected local model cache inputs for the current smokes:

- chat profile: `ministral-8b-instruct-2410-gguf-q4-k-m`
- embedding profile: `embedding-gemma-300m`
- model root: `~/.cache/huggingface/hub`

Do not run the model-backed smokes when `memory_pressure` reports less free
memory than the configured Noet AI threshold. The app also performs its own
preflight before loading local models.

Release evidence should record:

- the `memory_pressure` free percentage before model loading
- `cargo check -p noet-gui`
- the Metal compile check result, either skipped because `xcrun -f metal` was
  absent or run with `cargo check -p noet-gui --features mistralrs-inline-metal`
- both ignored local model smoke commands, if the model cache is available
- confirmation that semantic index files remain in the disposable cache/index
  directory and not in the Markdown vault

## macOS Local Package

The macOS packaging script builds an Apple Silicon local artifact:

```bash
./scripts/package-macos.sh
```

It embeds the `mistral.rs` runtime by default and auto-detects Metal
acceleration when `xcrun -f metal` succeeds. To require Metal and fail when it
is missing:

```bash
NOET_MACOS_ENABLE_METAL=1 ./scripts/package-macos.sh
```

To force the embedded CPU runtime even when Metal is installed:

```bash
NOET_MACOS_ENABLE_METAL=0 ./scripts/package-macos.sh
```

The packaging step can be attached to the release smoke script when a local app
bundle/DMG checkpoint is needed:

```bash
NOET_RUN_MACOS_PACKAGE=1 scripts/release-smoke.sh
```

Outputs:

- `dist/macos/Noet.app`
- `noet-v<version>-local-macos-arm64.dmg`
- `noet-v<version>-local-macos-arm64.tar.gz`

The app icon source lives at `assets/app-icon/noet-icon.svg`. The generated
macOS icon file lives at `assets/app-icon/Noet.icns` and is copied into
`Noet.app/Contents/Resources`. If the `.icns` file is missing, the packaging
script regenerates it with:

```bash
./scripts/generate-macos-icon.sh
```

The icon generator uses macOS `qlmanage`, `sips`, and `iconutil`, so it only runs
on macOS.

The script ad-hoc signs by default. A Developer ID is optional and should not
block local use or review. If a Developer ID identity is available, pass it with:

```bash
SIGN_IDENTITY="Developer ID Application: Example" ./scripts/package-macos.sh
```

The release workflow calls the same script with:

- `NOET_MACOS_VERSION`, usually the tag without the leading `v`
- `NOET_MACOS_ARTIFACT_LABEL`, usually the full tag such as `v0.6.0`
- `NOET_MACOS_BUNDLE_VERSION`, usually the bundle version without `-local`

Local defaults keep the explicit `-local` suffix so ad-hoc artifacts are easy to
distinguish from tagged releases.

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
