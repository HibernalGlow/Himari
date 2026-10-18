#!/bin/sh
# Independent, library-free check of the claim a user actually cares about:
#   "the fake SNI I typed into <data_dir>/packs/*.json is the name the far side receives."
#
# Deliberately NOT the same gauge as the Rust guard: the sink here is a raw socket that dumps the
# ClientHello bytes, and the gate is the real sni-gate process started by hand with the config the
# bridge's own renderer produced from the pack file. Nothing in this script links himari_bridge.
#
# Read-only outside its own scratch directory; binds 127.0.0.1 only; kills only what it started.
set -u

RUST_DIR=/Users/glow/Base/Code/Prometheus/himari-app/flutter/rust
GATE_BIN=${GATE_BIN:-/Users/glow/Base/Code/Prometheus/Himari/target/debug/sni-gate}
WORK=${WORK:-$(dirname "$0")}
SINK_PY="${SINK_PY:-$(dirname "$0")/sink.py}"
PROBE="$RUST_DIR/target/debug/himari_packs_probe"
DATA="$WORK/data"
HOST_ASKED=${HOST_ASKED:-packtest.invalid}
HOST_PROMISED=${HOST_PROMISED:-spoofed.test}

say() { echo "[wire] $*"; }
cleanup() { [ -n "${GATE_PID:-}" ] && kill "$GATE_PID" 2>/dev/null; [ -n "${SINK_PID:-}" ] && kill "$SINK_PID" 2>/dev/null; }
trap cleanup EXIT INT TERM
die() { echo "FAIL: $*"; exit 1; }

[ -x "$PROBE" ] || die "no built probe at $PROBE"
[ -x "$GATE_BIN" ] || die "no gate binary at $GATE_BIN"

rm -rf "$DATA"; mkdir -p "$DATA/packs"

PORTS=$(python3 - <<'PY'
import socket
a = socket.socket(); a.bind(("127.0.0.1", 0)); p1 = a.getsockname()[1]; a.close()
b = socket.socket(); b.bind(("127.0.0.1", 0)); p2 = b.getsockname()[1]; b.close()
print(p1, p2)
PY
) || die "could not pick free ports"
SINK_PORT=$(echo "$PORTS" | cut -d' ' -f1)
LISTEN_PORT=$(echo "$PORTS" | cut -d' ' -f2)
say "sink (pretend origin) : 127.0.0.1:$SINK_PORT"
say "gate listener        : 127.0.0.1:$LISTEN_PORT"
say "gate binary          : $GATE_BIN"

# 1. the user's file: this is the only input the whole check has.
printf '[{"name":"manual-wire","kind":"tls","host":"%s","upstream":"127.0.0.1:%s","override_sni":"%s","include_subdomains":false}]\n' \
    "$HOST_ASKED" "$SINK_PORT" "$HOST_PROMISED" > "$DATA/packs/wire.json" || die "writing pack failed"
say "pack file            : $DATA/packs/wire.json"

# 2. the bridge renders the gate config from that file (same merge Turn On uses).
"$PROBE" --data-dir "$DATA" --mode render --listen "127.0.0.1:$LISTEN_PORT" > "$WORK/render.out" 2>&1
RENDER_RC=$?
[ "$RENDER_RC" = 0 ] || die "render probe rc=$RENDER_RC"
grep -q "render ok: true" "$WORK/render.out" || die "renderer refused: $(sed -n '1,6p' "$WORK/render.out")"
python3 - "$WORK/render.out" "$WORK/gate.toml" "$DATA" <<'PY' || die "could not extract the rendered toml"
import sys
text = open(sys.argv[1]).read()
body = text.split("toml-begin\n", 1)[1].split("\ntoml-end", 1)[0]
# The probe renders an empty plan, so its CA/store paths come out blank and a real gate then tries
# to write a file named "". Only the route table is under test here, so those two paths are pointed
# at the scratch data dir and the gate mints its own CA into it, exactly as Turn On does.
data = sys.argv[3].rstrip("/")
body = body.replace('cert_path = ""', f'cert_path = "{data}/himari-ca.crt"')
body = body.replace('key_path = ""', f'key_path = "{data}/himari-ca.key"')
body = body.replace('dir = ""', f'dir = "{data}/store"')
open(sys.argv[2], "w").write(body + "\n")
PY
mkdir -p "$DATA/store"
grep -q "override_sni = \"$HOST_PROMISED\"" "$WORK/gate.toml" || die "the rendered config does not carry the pack's override"
say "step 2 ok: rendered $(wc -l < "$WORK/gate.toml" | tr -d ' ') lines of gate config carrying override_sni=$HOST_PROMISED"

# 3. start the sink and the real gate process.
python3 "$SINK_PY" "$SINK_PORT" > "$WORK/sink.log" 2>&1 &
SINK_PID=$!
sleep 1
kill -0 "$SINK_PID" || die "sink did not stay up: $(sed -n '1,5p' "$WORK/sink.log")"

"$GATE_BIN" --config "$WORK/gate.toml" > "$WORK/gate.log" 2>&1 &
GATE_PID=$!
say "gate pid=$GATE_PID sink pid=$SINK_PID"

python3 - "$LISTEN_PORT" <<'PY' || die "the gate never listened on 127.0.0.1:$LISTEN_PORT"
import socket, sys, time
port = int(sys.argv[1])
for _ in range(60):
    try:
        s = socket.create_connection(("127.0.0.1", port), 0.5); s.close(); sys.exit(0)
    except OSError:
        time.sleep(0.1)
sys.exit(1)
PY

send_hello() {
    # $1 = host:port to dial, $2 = SNI to ask for. The client never speaks to the sink directly
    # except in the control run below, which is what makes the control a control.
    printf 'GET / HTTP/1.1\r\nHost: %s\r\nConnection: close\r\n\r\n' "$2" \
        | openssl s_client -connect "$1" -servername "$2" -brief > "$WORK/client.$3.log" 2>&1
    return $?
}

# 4. CONTROL: dial the sink directly with the same name. If the sink cannot see an UNSPOOFED
#    hello here, every later "it saw the other name" would be worthless.
send_hello "127.0.0.1:$SINK_PORT" "$HOST_ASKED" control
say "control client rc=$?"

# 5. THE MEASUREMENT: same client, same requested name, but through the gate.
send_hello "127.0.0.1:$LISTEN_PORT" "$HOST_ASKED" through
say "through-gate client rc=$? (a verification warning is expected: this client does not trust the gate's CA)"

sleep 2
echo "--- sink.log ---"
sed -n '1,40p' "$WORK/sink.log"
echo "--- gate.log (first 20) ---"
sed -n '1,20p' "$WORK/gate.log"

kill "$GATE_PID" 2>/dev/null; kill "$SINK_PID" 2>/dev/null
wait "$GATE_PID" 2>/dev/null; wait "$SINK_PID" 2>/dev/null

# 6. verdict, read out of the sink's own lines rather than from a green tick. The two hellos are
#    judged as an ordered pair: connection #1 is the direct control, connection #2 came through the
#    gate. Grepping the log anywhere would let the control alone satisfy the check, so the
#    comparison is positional and a run whose two names coincide is a failure.
python3 - "$WORK/sink.log" "$HOST_ASKED" "$HOST_PROMISED" <<'PY'
import re, sys
names = re.findall(r"SNI received: ([^\s!]+)", open(sys.argv[1]).read())
asked, promised = sys.argv[2], sys.argv[3]
print(f"[wire] sink recorded, in order: {names}")
if len(names) < 2:
    print(f"FAIL: expected 2 hellos (control + through-gate), got {len(names)}"); sys.exit(1)
if names[0] != asked:
    print(f"FAIL: the control did not show the gauge working (saw {names[0]!r}, wanted {asked!r})"); sys.exit(1)
if names[1] == names[0]:
    print("FAIL: no difference - the gate sent the same name the client asked for"); sys.exit(1)
if names[1] != promised:
    print(f"FAIL: the far side saw {names[1]!r}, but the pack promised {promised!r}"); sys.exit(1)
print(f"[wire] RESULT: client asked {asked}, control hello {names[0]}, through-gate hello {names[1]}")
print("[wire]        the pack's override_sni is the name that left the machine.")
sys.exit(0)
PY
VERDICT_RC=$?
echo "[wire] verdict rc=$VERDICT_RC"
exit $VERDICT_RC
