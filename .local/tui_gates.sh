#!/usr/bin/env bash
# One button that verifies and launches the Himari terminal UI.
#
# It exists because the agent-side build queue was blocked for hours by another
# lane's flutter tests: the properties below are only claims until something runs
# them, and you are allowed to run them on your own schedule.
#
#   bash .local/tui_gates.sh            # respects the build lock, then runs all steps
#   bash .local/tui_gates.sh --force    # skip the lock and the memory guard
#   bash .local/tui_gates.sh --dry-run  # only measure conditions and print the plan
#
# Every step prints its own exit code; nothing here infers success from silence.

set -u
cd "$(dirname "$0")/.." || exit 2

LOCK_DIR=/Users/glow/Base/.build-lock
STALE_SECONDS=900
RESULTS=""
FAILED=0

note() {
    printf '%s\n' "$*"
}

record() {
    # record <name> <rc> <evidence>
    RESULTS="${RESULTS}$1|rc=$2|$3
"
    if [ "$2" != "0" ]; then
        FAILED=$((FAILED + 1))
    fi
}

conditions() {
    FT=$(pgrep flutter_tester 2>/dev/null | wc -l | tr -d ' ')
    RC=$(pgrep rustc 2>/dev/null | wc -l | tr -d ' ')
    SWAP=$(sysctl -n vm.swapusage)
    LOAD=$(uptime | sed 's/.*load averages*://')
    note "flutter_tester=$FT rustc=$RC"
    note "swap: $SWAP"
    note "load:$LOAD"
}

take_lock() {
    if [ "$FORCE" = 1 ]; then
        note "lock: skipped (--force)"
        HAVE_LOCK=0
        return 0
    fi
    for i in $(seq 1 12); do
        if mkdir "$LOCK_DIR" 2>/dev/null; then
            printf '%s %s\n' himari-tui "$(date +%s)" >"$LOCK_DIR/owner"
            HAVE_LOCK=1
            note "lock: acquired on attempt $i"
            return 0
        fi
        holder=$(cat "$LOCK_DIR/owner" 2>/dev/null || echo "?")
        age=$(( $(date +%s) - $(awk '{print $2}' <<<"$holder" 2>/dev/null || echo 0) ))
        if [ "$age" -gt "$STALE_SECONDS" ]; then
            note "lock: stale (holder=$holder age=${age}s > ${STALE_SECONDS}s) — taking it"
            rm -f "$LOCK_DIR/owner"
            rmdir "$LOCK_DIR" 2>/dev/null
            continue
        fi
        note "lock: busy, holder=$holder age=${age}s (attempt $i)"
        sleep 25
    done
    HAVE_LOCK=0
    note "lock: never acquired"
    return 1
}

release_lock() {
    if [ "${HAVE_LOCK:-0}" = 1 ]; then
        rm -f "$LOCK_DIR/owner"
        rmdir "$LOCK_DIR" 2>/dev/null
        note "lock: released"
    fi
}

FORCE=0
DRY=0
for arg in "$@"; do
    case "$arg" in
        --force) FORCE=1 ;;
        --dry-run) DRY=1 ;;
        *) note "unknown argument: $arg"; exit 2 ;;
    esac
done

note "== conditions =="
conditions

STEPS=(
    "tui gates|cargo test --features tui --no-fail-fast --test tui_surfaces --test tui_visual_language --test tui_mouse|test result lines"
    "build tui bin|cargo build --features tui --bin sni-gate-tui|Finished, or the first error block"
    "default build untouched|cargo check --locked|error count must be zero without the feature"
    "format gate|cargo fmt --all -- --check|any file it names"
    "clippy gate|cargo clippy --features tui --all-targets --locked -- -D warnings|warning count"
    "probe self-test|python3 .local/tui_pty_probe.py --self-test|SELF-TEST: PASS (0 failed)"
    "live pty click|python3 .local/tui_pty_probe.py target/debug/sni-gate-tui sni-gate.example.toml|RESULT: PASS + the reconstructed frame"
)

if [ "$DRY" = 1 ]; then
    note ""
    note "== plan (--dry-run, nothing executed) =="
    for step in "${STEPS[@]}"; do
        IFS='|' read -r name cmd evidence <<<"$step"
        printf '  - %s\n      cmd: %s\n      look for: %s\n' "$name" "$cmd" "$evidence"
    done
    exit 0
fi

if [ "$FORCE" != 1 ]; then
    ft_now=$(pgrep flutter_tester 2>/dev/null | wc -l | tr -d ' ')
    if [ "${ft_now:-0}" != "0" ]; then
        note ""
        note "refusing to start: $ft_now flutter_tester processes are running."
        note "That is the shape that took the IDE down twice today. Wait for them, or"
        note "pass --force if you are a human who has decided to take that risk."
        exit 3
    fi
fi

if [ "$FORCE" != 1 ] && ! take_lock; then
    note ""
    note "Did not get the lock. Re-run later, or use --force."
    exit 4
fi

export https_proxy="${https_proxy:-http://127.0.0.1:7890}"
export http_proxy="${http_proxy:-http://127.0.0.1:7890}"

LOG_DIR=.local/gates
mkdir -p "$LOG_DIR"

run_step() {
    local name="$1" cmd="$2"
    local log="$LOG_DIR/$(echo "$name" | tr ' ' '_').log"
    note ""
    note "==== $name ===="
    note "  $cmd"
    # shellcheck disable=SC2086
    bash -c "$cmd" >"$log" 2>&1
    local rc=$?
    note "  rc=$rc   log=$log"
    # Print the lines that carry the verdict, never a filtered 'success' guess.
    grep -E "test result|^error|^warning: unused|Finished|FAILED|panicked|assertion" "$log" | head -12
    record "$name" "$rc" "$(grep -cE '^error' "$log") error lines"
}

for step in "${STEPS[@]}"; do
    IFS='|' read -r name cmd evidence <<<"$step"
    run_step "$name" "$cmd"
done

release_lock

note ""
note "== summary =="
printf '%s' "$RESULTS"
note "failed steps: $FAILED"
if [ "$FAILED" = 0 ]; then
    note ""
    note "All gates green. To look at it yourself:"
    note "  target/debug/sni-gate-tui --config sni-gate.example.toml"
    note "Mouse works immediately (press/release on the same control, wheel over a pane,"
    note "drag to scroll, right/middle click, double-click). Quit with q or the QUIT button."
fi
exit $([ "$FAILED" = 0 ] && echo 0 || echo 1)
