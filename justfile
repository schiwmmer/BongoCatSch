# `just` defaults to `sh` on every platform, but Windows has no POSIX shell and
# Git for Windows only adds `cmd` (never `usr\bin`) to PATH, so every recipe
# failed with "could not find the shell `sh`". `cmd.exe` is always resolvable on
# Windows and propagates native exit codes, so a failing `cargo` still fails the
# recipe; a shell that reports success for a failed child command would let a
# broken build pass. Unix keeps the default `sh -cu`.
set windows-shell := ["cmd.exe", "/c"]

# List the available tasks.
default:
    @just --list

# Run the Development product until explicitly quit.
dev:
    cargo run --locked -p bongocat-app --release -- --run-seconds 0

# Exercise settings close, reopen, and runtime continuity.
dev-smoke:
    cargo run --locked -p bongocat-app --release -- --run-seconds 4 --settings-window-smoke

# Launch the packaged product for a bounded settings-window run and archive the log.
#
# A GUI process has no console, so the log is the only evidence it started at
# all. A stale instance is killed first: LaunchServices drops the flags for an
# already-running copy, and `open --args` is not used here for the same reason.
# The watchdog catches the case where the run guard never fires, which is what a
# silently ignored `--run-seconds` looks like.
gui-smoke seconds="6" app="/Applications/BongoCat.app":
    #!/usr/bin/env sh
    set -eu
    binary="{{app}}/Contents/MacOS/bongocat-app"
    if [ ! -x "$binary" ]; then
        echo "gui-smoke: no executable at $binary (run 'just verify-app' and sync first)" >&2
        exit 1
    fi
    mkdir -p target/smoke
    log="target/smoke/gui-smoke-$(date +%Y%m%d-%H%M%S)-{{seconds}}s.log"
    pkill -f 'Contents/MacOS/bongocat-app' >/dev/null 2>&1 || true
    sleep 1
    "$binary" --run-seconds "{{seconds}}" --settings-window-smoke >"$log" 2>&1 &
    pid=$!
    # The watchdog polls a sentinel instead of being killed, so the recipe never
    # prints a job-termination message for it. It only fires when the run guard
    # silently did not apply, which is the failure mode this recipe exists for.
    done_file="target/smoke/.done.$$"
    timed_out="target/smoke/.timed-out.$$"
    (
        elapsed=0
        while [ ! -f "$done_file" ]; do
            if [ "$elapsed" -ge $(({{seconds}} + 20)) ]; then
                : >"$timed_out"
                kill "$pid" >/dev/null 2>&1 || true
                break
            fi
            sleep 1
            elapsed=$((elapsed + 1))
        done
    ) &
    watchdog=$!
    status=0
    wait "$pid" || status=$?
    : >"$done_file"
    wait "$watchdog" 2>/dev/null || true
    /bin/rm -f "$done_file"
    if [ -f "$timed_out" ]; then
        /bin/rm -f "$timed_out"
        echo "gui-smoke: the run guard never fired; killed after $(({{seconds}} + 20))s" >&2
        echo "gui-smoke: log at $log" >&2
        exit 1
    fi
    if [ "$status" -ne 0 ]; then
        echo "gui-smoke: the product exited $status" >&2
        echo "gui-smoke: log at $log" >&2
        exit "$status"
    fi
    if ! grep -q 'Live2D Cubism SDK Core' "$log"; then
        echo "gui-smoke: the model never loaded" >&2
        echo "gui-smoke: log at $log" >&2
        exit 1
    fi
    echo "gui-smoke: ok ({{seconds}}s)  log: $log"


# Run a deterministic Live2D diagnostic preview.
preview model="standard" seconds="30":
    cargo run --locked -p bongocat-overlay --release -- "{{model}}" "{{seconds}}"

# Print the single product version source resolved by Cargo.
version:
    @cargo run --locked -q -p bongocat-packaging -- --print-version

# Regenerate the checked-in configuration and window-state JSON Schemas.
schema:
    cargo run --locked -p bongocat-config --features schema-generation --bin generate_json_schemas

# Run the workspace tests.
test:
    cargo test --locked --workspace

# Build the browser build of the pomodoro page.
#
# The page reuses the same state machine the desktop runs, compiled to
# `wasm32-unknown-unknown` and driven through a small `extern "C"` bridge, so the
# numbers on the web build and the numbers in the app cannot drift apart.
#
# `getrandom_backend=unsupported` is required by the way `atomic-write-file`
# pulls in `rand`: the browser build never writes a file, so the backend only has
# to exist, not to work. `--target-dir` keeps the wasm artifacts out of the
# native `target/`, whose profile settings are tuned for the app.
web:
    #!/usr/bin/env sh
    set -eu
    RUSTFLAGS='--cfg getrandom_backend="unsupported"' \
        cargo build --locked -p bongocat-pomodoro-web \
        --target wasm32-unknown-unknown --release --target-dir target-web
    cp target-web/wasm32-unknown-unknown/release/bongocat_pomodoro_web.wasm web/
    echo "web: web/bongocat_pomodoro_web.wasm ($(wc -c < web/bongocat_pomodoro_web.wasm) bytes)"

# Serve the browser build.
#
# A `file://` page cannot fetch the module, so the page has to come off http.
web-serve: web
    python3 -m http.server 8099 --bind 127.0.0.1 --directory web

# Run all default workspace quality gates.
check:
    # Windows support was removed in 77c92050; this gate keeps it gone. The
    # deliberate `pc-windows-msvc` rejection in the cubism bindgen is not a
    # `cfg(windows)`/`target_os` conditional or a dependency, so it is not
    # matched by either grep.
    @if grep -rIn --include='*.rs' -e 'cfg(windows)' -e 'target_os = "windows"' -e 'target_os="windows"' crates tools spikes 2>/dev/null; then echo 'error: Windows conditional found (Windows support was removed in 77c92050)' >&2; exit 1; fi
    @if grep -rIni --include='Cargo.toml' -e 'windows' -e 'winapi' crates tools spikes 2>/dev/null; then echo 'error: Windows dependency found (Windows support was removed in 77c92050)' >&2; exit 1; fi
    cargo fmt --all -- --check
    cargo clippy --locked --workspace --all-targets --all-features --exclude bongocat-app -- -D warnings
    cargo clippy --locked -p bongocat-app --all-targets --features storage-test-injection -- -D warnings
    cargo clippy --locked -p bongocat-app --all-targets --features production -- -D warnings
    cargo test --locked --workspace
    cargo check --locked --workspace --release

# `--target`, `--environment` and `--formats` are forwarded to the packaging
# entry point, which is the single source of truth for targets and artifacts:
#
#   just build
#   just build --target x86_64-apple-darwin
#   just build --environment development --formats app
#
# Build the Production product and package the release artifacts.
build *args:
    cargo run --locked -p bongocat-packaging -- {{args}}

# Build the Production app bundle for installing on this machine to verify.
#
# The environment is pinned on purpose: `--environment development` builds a
# fine bundle, but the two environments keep separate data directories
# (`com.ayangweb.bongo-cat/development|production`), and a development build
# reads an empty one — every stored level, count, model and task disappears
# from the surface while the production files sit untouched beside it. A
# verification build has to read the user's real data, so it has to be a
# production build. See the 2026-10-15 pomodoro round, where a development
# build made a real interaction history look deleted.
verify-app:
    cargo run --locked -p bongocat-packaging -- --environment production --formats app

# Reclaim the debug build cache (typically 16G of `target/debug`), keeping the
# release build that `just build`/`verify-app` use. Cargo never garbage-collects
# artifacts for dependency versions that dropped out of `Cargo.lock`, so the
# debug cache only grows until something removes it. Run this periodically or
# before a release; the next `cargo check`/`test` rebuilds from source.
clean-dev:
    cargo clean --profile dev

# Reclaim the two caches that grow without bound and cost little to keep: the
# incremental cache (per-crate hashes that only speed up the next edit, 1-5G) and
# the cross-target tree the packager writes (`target/aarch64-apple-darwin`, a
# second copy of binaries that are already built, 1.5G). This is the small valve
# to `clean-dev`'s full one.
#
# The paths are removed directly rather than through `cargo clean --target
# <triple>`: this machine's host triple *is* aarch64-apple-darwin, so asking cargo
# to clean that target matches every artifact in `target/` and wipes the lot —
# including the release build the next `verify-app` would have reused.
prune:
    /bin/rm -rf target/debug/incremental
    /bin/rm -rf target/aarch64-apple-darwin
    @du -sh target 2>/dev/null || true

# Report whether `target/` has drifted away from the steady state that
# `incremental = false` produces (~8 GiB), and which of the two slow growth
# sources is responsible. Read-only: nothing is deleted, so it is safe at any
# time — unlike `prune`, which costs a full rebuild of the cross-target tree.
#
# The thresholds come from measurement, not guesswork: after the incremental
# cache was removed, 528 dependency crates held 1500 variants (mean 2.84, none
# above 8), and `permission-flow` — the Swift FFI bridge, the one build script
# that writes a large per-configuration output — settled at 4 copies. Variants
# above 8 mean artifacts for a dependency version or feature set that no longer
# exists, which cargo keeps until something removes it. `bongocat_*` are exempt
# from that check because their multiplicity is legitimate: the check recipes
# compile them under several `--features` combinations.
[doc('Report `target/` drift and which reclaim recipe it needs')]
target-health:
    #!/bin/sh
    set -u

    if [ ! -d target ]; then
        echo "target/ 尚未生成，无需维护。"
        exit 0
    fi

    total_mb=$(du -sm target 2>/dev/null | cut -f1)
    printf 'target 总计: %s MiB（稳态约 8192）\n\n' "${total_mb:-0}"

    echo '主要子目录:'
    for p in target/debug/deps target/debug/build target/debug/incremental target/release target/aarch64-apple-darwin; do
        if [ -d "$p" ]; then
            printf '  %-36s %6s MiB\n' "$p" "$(du -sm "$p" 2>/dev/null | cut -f1)"
        fi
    done

    variants() {
        ls target/debug/deps 2>/dev/null \
            | sed -nE 's/^lib(.+)-[0-9a-f]+\.rlib$/\1/p' \
            | sort | uniq -c | sort -rn
    }

    echo ''
    echo '依赖 crate 变体数 top5（稳态 1~5；>8 说明有孤儿产物）:'
    variants | head -5 | awk '{printf "  %-34s %s 个\n", $2, $1}'

    pf_count=$(ls -d target/debug/build/permission-flow-* 2>/dev/null | wc -l | tr -d ' ')
    echo ''
    printf 'Swift 桥产物份数（permission-flow-*）: %s（稳态 4~6）\n' "$pf_count"

    stale=0
    flag() {
        if [ "$stale" -eq 0 ]; then
            echo ''
            echo '诊断:'
        fi
        echo "  ⚠ $1"
        stale=1
    }

    if [ "${total_mb:-0}" -gt 10240 ]; then
        flag "target 超过 10 GiB（${total_mb} MiB）"
    fi
    if [ "$pf_count" -gt 6 ]; then
        flag "permission-flow 产物堆积（$pf_count 份）"
    fi
    orphan=$(variants | awk '$2 !~ /^bongocat_/ && $1 > 8 {print $2 " " $1 " 份"; exit}')
    if [ -n "$orphan" ]; then
        flag "依赖孤儿变体: $orphan"
    fi

    echo ''
    if [ "$stale" -eq 0 ]; then
        echo '✓ 健康，无需清理。'
    elif [ "$pf_count" -gt 6 ] || [ -n "$orphan" ]; then
        # Orphan variants and permission-flow copies live in `target/debug`,
        # which `prune` never touches — it only reclaims the incremental and
        # cross-target trees. `clean-dev` is the valve for that cache.
        echo '建议: just clean-dev（孤儿变体/权限流堆积在 target/debug，just prune 清不掉）'
    else
        echo '建议: just prune（仅增量/跨目标膨胀）'
    fi

# Generate the Minisign key pair that signs update payloads (one-time, offline).
keygen file:
    cargo run --locked -p bongocat-packaging -- --generate-signing-key {{file}}

# Each `just build` writes one manifest fragment per target, but the updater requests a
# single shared manifest, so a multi-target release merges the fragments once before
# publishing:
#
#   just manifest target/package target/package/macos-aarch64.json ...
#   just release-manifest target/package NOTES.md target/package/*.json
#
# Merge the per-target manifest fragments into the shared release manifest.
manifest directory *fragments:
    cargo run --locked -p bongocat-packaging -- --merge-manifests {{directory}} {{fragments}}

# Merge the fragments and announce the release changelog read from <notes>, which the
# update window shows. The release pipeline uses this one; `manifest` is the same merge
# without notes.
release-manifest directory notes *fragments:
    cargo run --locked -p bongocat-packaging -- --merge-manifests {{directory}} --release-notes {{notes}} {{fragments}}

# Compose this version's release notes from CHANGELOG.md and CHANGELOG.zh-CN.md into
# <file>, followed by the download links, model gallery and sponsors this tool knows.
# The release pipeline feeds that one file to both `release-manifest` and the
# GitHub release body, so the release page and the update window cannot disagree:
#
#   just release-notes release-notes.md
#
# The lookup is keyed on the product version `just version` prints, so it fails when the
# version was bumped without a matching changelog entry. Only the changelog entry is
# versioned; the trailing block is generated, so the wording, the gallery and the
# sponsor list are constants in `crates/bongocat-packaging`, not changelog text.
release-notes file:
    cargo run --locked -p bongocat-packaging -- --extract-release-notes {{file}}
