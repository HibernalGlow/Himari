#!/usr/bin/env python3
"""Put `sni-gate-tui` in a real PTY, drive it with the mouse, and prove what changed.

Why a script instead of a screenshot: this workstation has no GUI capture, and a
terminal UI is fully observable from the byte stream anyway. The program:

1. opens a PTY at a fixed size and execs the TUI;
2. reconstructs the screen by following cursor-position writes, so assertions are
   about cells rather than substrings of escape noise — and a sequence cut in half
   between two PTY reads is carried over instead of being painted as text;
3. checks the setup sequences in the raw stream, which is direct evidence the app
   asked the terminal for the pointer, something no unit test can show;
4. locates the theme button *in the reconstructed grid*, clicks its middle cell,
   then wheels inside the status card, and requires that cells changed and that the
   block which starts below the fold came into view.

A gesture is only credited when the screen differs afterwards. Sending bytes is not
the same as affecting the UI, and this script refuses to conflate them.

Usage:
    python3 .local/tui_pty_probe.py --self-test
    python3 .local/tui_pty_probe.py target/debug/sni-gate-tui sni-gate.example.toml
"""

import fcntl
import os
import pty
import re
import select
import struct
import sys
import termios
import time

# Overridable so the same probe can be run at a comfortable size and at a cramped
# one: the control band is expected to wrap rather than hide buttons, and that only
# shows up when the window is narrow.
ROWS = int(os.environ.get("TUI_PROBE_ROWS", "32"))
COLS = int(os.environ.get("TUI_PROBE_COLS", "96"))

WIDE_RANGES = [
    (0x1100, 0x115F), (0x2E80, 0x303E), (0x3041, 0x33FF), (0x3400, 0x4DBF),
    (0x4E00, 0x9FFF), (0xA000, 0xA4CF), (0xAC00, 0xD7A3), (0xF900, 0xFAFF),
    (0xFE30, 0xFE4F), (0xFF00, 0xFF60), (0xFFE0, 0xFFE6), (0x1F300, 0x1FAFF),
]


def cell_width(ch):
    """Columns a grapheme occupies, by the same ranges `paint::is_wide` uses.

    The probe has to agree with the app's arithmetic: treat a CJK glyph as one cell
    and every column after it is off by one, so a located label hands back click
    coordinates that miss the button.
    """
    o = ord(ch)
    return 2 if any(lo <= o <= hi for lo, hi in WIDE_RANGES) else 1


def overflow_count(rows):
    """The `+N below` marker's N from a frame, or None when no pane is overflowed."""
    for line in rows:
        m = re.search(r"\+(\d+) below", line)
        if m:
            return int(m.group(1))
    return None


class Screen:
    """A cell grid updated by cursor-addressed writes."""

    def __init__(self, rows, cols):
        self.rows, self.cols = rows, cols
        self.grid = [[" "] * cols for _ in range(rows)]
        self.y = self.x = 0
        self.modes = set()
        # A PTY delivers whatever fits its buffer, so an escape sequence can arrive in
        # pieces. Carrying the tail over matters: without it the next chunk starts
        # mid-sequence and its digits get painted as text, which once made this probe
        # report a screen the app never drew.
        self.pending = ""

    def feed(self, data):
        self.put(self.pending + data)

    def put(self, data):
        self.pending = ""
        i, n = 0, len(data)
        while i < n:
            ch = data[i]
            if ch == "\x1b":
                consumed = self.escape(data, i)
                if consumed is None:
                    tail = data[i:]
                    # Only carry a tail that could still become a sequence.
                    if len(tail) <= 8192:
                        self.pending = tail
                    return
                i += consumed
                continue
            if ch == "\r":
                self.x = 0
            elif ch == "\n":
                self.y = min(self.y + 1, self.rows - 1)
                self.x = 0
            elif ch == "\x08":
                self.x = max(0, self.x - 1)
            elif ord(ch) >= 0x20:
                width = cell_width(ch)
                if self.y < self.rows and self.x < self.cols:
                    self.grid[self.y][self.x] = ch
                    for extra in range(1, width):
                        if self.x + extra < self.cols:
                            self.grid[self.y][self.x + extra] = ""
                self.x += width
            i += 1

    def escape(self, data, start):
        if start + 1 >= len(data):
            return None
        nxt = data[start + 1]
        if nxt == "[":
            m = re.match(r"\x1b\[([0-9;?]*)([A-Za-z@])", data[start:])
            if not m:
                return None
            self.csi(m.group(1), m.group(2))
            return len(m.group(0))
        if nxt == "]":
            bell = data.find("\x07", start)
            st = data.find("\x1b\\", start)
            found = [p for p in (bell, st) if p != -1]
            if not found:
                return None
            stop = min(found)
            return stop - start + (1 if stop == bell else 2)
        return 2

    def csi(self, params, final):
        if final in "hl":
            if "?" in params:
                self.modes.add((params.replace("?", ""), final))
            return
        nums = [int(p) if p else 0 for p in params.split(";")] if params else []
        if final in "Hf":
            self.y = (nums[0] - 1) if nums and nums[0] else 0
            self.x = (nums[1] - 1) if len(nums) > 1 and nums[1] else 0
        elif final == "A":
            self.y = max(0, self.y - (nums[0] if nums else 1))
        elif final == "B":
            self.y = min(self.rows - 1, self.y + (nums[0] if nums else 1))
        elif final == "C":
            self.x = min(self.cols - 1, self.x + (nums[0] if nums else 1))
        elif final == "D":
            self.x = max(0, self.x - (nums[0] if nums else 1))
        elif final == "G":
            self.x = (nums[0] - 1) if nums and nums[0] else 0
        elif final == "J":
            if not nums or nums[0] == 2:
                self.grid = [[" "] * self.cols for _ in range(self.rows)]
        elif final == "K":
            for c in range(self.x, self.cols):
                self.grid[self.y][c] = " "

    def text_rows(self):
        return ["".join(row).rstrip() for row in self.grid]

    def find(self, needle):
        """(row, col) of the first cell of an ASCII label, or None.

        ASCII only, stated rather than assumed: a CJK label occupies fewer cells than
        it has characters, so a character-index search would be off by one per wide
        glyph and the click would land beside the control.
        """
        if not needle.isascii():
            raise ValueError(f"the probe only locates ASCII labels, got {needle!r}")
        n = len(needle)
        for r, row in enumerate(self.grid):
            for c in range(0, max(1, len(row) - n + 1)):
                if all((row[c + k] or " ") == needle[k] for k in range(n)):
                    return (r, c)
        return None


def set_winsize(fd, rows, cols):
    fcntl.ioctl(fd, termios.TIOCSWINSZ, struct.pack("HHHH", rows, cols, 0, 0))


def self_test():
    """A positive control for the probe itself.

    If the ANSI reconstruction is wrong, every live assertion would be reading a fake
    screen and a green run would mean nothing.
    """
    screen = Screen(6, 40)
    screen.feed(
        "\x1b[?1049h\x1b[?1000h\x1b[?1003h\x1b[?1006h\x1b[?25l"
        "\x1b[2J"
        "\x1b[1;2H" + "HIMARI" + "\x1b[0m"
        "\x1b[3;30H" + "LIGHT" + "\x1b[0m"
        "\x1b[5;1Htail"
    )
    rows = screen.text_rows()
    failures = []

    def want(ok, label):
        print(("PASS  " if ok else "FAIL  ") + label)
        if not ok:
            failures.append(label)

    want(("1049", "h") in screen.modes, "alt-screen mode parsed")
    want(("1006", "h") in screen.modes, "SGR mouse mode parsed")
    want(rows[0][1:7] == "HIMARI", f"row 1 col 2 wordmark, got {rows[0]!r}")
    want(rows[2][29:34] == "LIGHT", f"row 3 col 30 label, got {rows[2]!r}")
    at = screen.find("LIGHT")
    want(at == (2, 29), f"find() reports 0-based (row, col) = {at}")
    if at:
        row, col = at
        press = f"\x1b[<0;{col + 3};{row + 1}M"
        want(press == "\x1b[<0;32;3M", f"click math lands inside the label: {press!r}")

    # A sequence split across two PTY reads must still parse as one.
    split = Screen(4, 20)
    split.feed("\x1b[1;2HAB")
    split.feed("\x1b[2;")
    split.feed("2HCD")
    want(split.text_rows()[0][1:3] == "AB", f"first write survived, got {split.text_rows()!r}")
    want(split.text_rows()[1][1:3] == "CD", f"the carried tail still positioned, got {split.text_rows()!r}")
    want("2H" not in "".join(split.text_rows()), "no escape debris leaked into the grid")

    # Erase-line clears from the cursor to the end of the row.
    er = Screen(3, 20)
    er.feed("\x1b[1;1HLIGHT\x1b[1;3H\x1b[K")
    want(er.text_rows()[0] == "LI", f"only glyphs before the cursor survive, got {er.text_rows()[0]!r}")

    # Wide glyphs consume two cells each.
    wide = Screen(3, 20)
    wide.feed("\x1b[1;1H" + "日本" + "LIGHT")
    want(wide.find("LIGHT") == (0, 4), f"CJK took two cells each, got {wide.find('LIGHT')}")
    refused = False
    try:
        wide.find("日本")
    except ValueError:
        refused = True
    want(refused, "a non-ASCII needle is refused instead of guessing a column")

    want(overflow_count(["  +18 below — wheel"]) == 18, "overflow marker parsed from a frame")

    print(f"\nSELF-TEST: {'PASS' if not failures else 'FAIL'} ({len(failures)} failed)")
    return 1 if failures else 0


def main():
    if "--self-test" in sys.argv:
        return self_test()

    binary = sys.argv[1] if len(sys.argv) > 1 else "target/debug/sni-gate-tui"
    config = sys.argv[2] if len(sys.argv) > 2 else "sni-gate.example.toml"
    for path, label in ((binary, "binary"), (config, "config")):
        if not os.path.exists(path):
            print(f"FAIL: no {label} at {path}")
            return 2

    master, slave = pty.openpty()
    set_winsize(slave, ROWS, COLS)
    pid = os.fork()
    if pid == 0:
        os.setsid()
        fcntl.ioctl(slave, termios.TIOCSCTTY, 1)
        os.dup2(slave, 0)
        os.dup2(slave, 1)
        os.dup2(slave, 2)
        os.close(master)
        os.close(slave)
        os.execv(os.path.abspath(binary), [os.path.abspath(binary), "--config", config])
        os._exit(127)
    os.close(slave)

    screen = Screen(ROWS, COLS)
    raw = []

    def drain(seconds):
        end = time.time() + seconds
        while time.time() < end:
            r, _, _ = select.select([master], [], [], 0.1)
            if not r:
                continue
            try:
                chunk = os.read(master, 65536)
            except OSError:
                return
            if not chunk:
                return
            text = chunk.decode("utf-8", "replace")
            raw.append(text)
            screen.feed(text)

    def click(x, y, button=0):
        # 1-based, as the SGR protocol requires.
        #
        # The `<` stays on the release too. crossterm 0.29 dispatches CSI on its
        # first parameter byte (`b'<' => parse_csi_sgr_mouse`), so a release written
        # the old rxvt way (`ESC[0;x;ym`) falls through to the key-code parser and
        # never becomes `MouseEventKind::Up` — the UI would sit in the pressed state
        # forever and no control would ever activate.
        os.write(master, f"\x1b[<{button};{x};{y}M".encode())
        time.sleep(0.03)
        os.write(master, f"\x1b[<{button};{x};{y}m".encode())

    drain(1.5)
    before = screen.text_rows()

    # Click the theme button, positioned from the painted grid. Deliberately not
    # START: that would spawn a real gateway process. The click path through
    # `activate()` is identical either way, so this proves the pointer with no side
    # effects beyond the UI's own state.
    target = screen.find("LIGHT") or screen.find("DARK")
    if target:
        row, col = target
        click(col + 3, row + 1)
    drain(0.8)
    at_click = screen.text_rows()

    # Wheel inside the status card body. crossterm encodes ScrollDown as button 65.
    # Enough notches to cross the whole overflow: the point of the assertion is that
    # the block below the fold is reachable *by wheeling*, so it has to be all the way.
    for _ in range(24):
        os.write(master, b"\x1b[<65;10;14M")
        time.sleep(0.02)
    drain(0.8)
    at_wheel = screen.text_rows()

    os.write(master, b"q")
    drain(1.5)

    forced = False
    try:
        os.kill(pid, 15)
    except ProcessLookupError:
        pass
    for _ in range(40):
        done, _status = os.waitpid(pid, os.WNOHANG)
        if done:
            break
        time.sleep(0.1)
    else:
        forced = True
        os.kill(pid, 9)
        os.waitpid(pid, 0)
    os.close(master)

    stream = "".join(raw)
    joined_click = "\n".join(at_click)
    joined_wheel = "\n".join(at_wheel)
    failures = []

    def require(ok, label):
        print(("PASS  " if ok else "FAIL  ") + label)
        if not ok:
            failures.append(label)

    require("\x1b[?1049h" in stream, "entered the alternate screen")
    require("\x1b[?1000h" in stream or "\x1b[?1003h" in stream, "asked the terminal for mouse reporting")
    require("\x1b[?1006h" in stream, "asked for SGR (x;y) mouse coordinates")
    require("\x1b[?25l" in stream, "hid the cursor")
    require("\x1b[?1049l" in stream, "left the alternate screen again on exit")
    require(not forced, "exited on `q` without needing SIGKILL")

    require("HIMARI" in joined_click, "wordmark rendered")
    require("START" in joined_click and "TURN OFF" in joined_click, "action buttons rendered")
    require("STATUS" in joined_click and "LOGS" in joined_click, "tab strip rendered")
    require("GATEWAY" in joined_click, "status card rendered")
    # Nothing may be dropped at either tested size: a control that is not painted is
    # unreachable by pointer and by keyboard focus alike.
    have_actions = all(x in joined_click for x in ("START", "TURN OFF", "QUIT"))
    have_theme = "DARK" in joined_click or "LIGHT" in joined_click
    named = "action(s) hidden for lack of room" in joined_click
    require(have_actions or named, "actions are painted, or the ones that are not are named")
    require(have_theme or named, "the theme button is painted, or named as dropped")
    # A window this wide is expected to fit everything; narrower ones may honestly
    # report a drop instead, which is why the assertion above allows the note.
    if COLS >= 96:
        require(have_actions and have_theme, f"at {COLS} columns nothing is dropped")
    require("sni-gate.example.toml" in joined_click, "the config it was given is on screen")
    require("missing" in joined_click or "present" in joined_click, "the CA reading is stated as a stat either way")
    require("8;2;" not in joined_click, "no truncated-escape debris in the reconstructed frame")

    require(target is not None, f"the theme button was located in the painted grid ({target})")
    diff_click = [(b, a) for b, a in zip(before, at_click) if b != a]
    require(bool(diff_click), "the click changed cells")
    require(any("theme:" in a.lower() for _b, a in diff_click), "the message band reported the theme switch")

    diff_wheel = [(c, w) for c, w in zip(at_click, at_wheel) if c != w]
    require(bool(diff_wheel), "the wheel changed cells")
    require("NOT MEASURED" in joined_wheel, "wheeling brought the not-measured block into view")
    was = overflow_count(at_click)
    now = overflow_count(at_wheel)
    require(was is not None, f"the overflow marker was on screen before wheeling (got {was})")
    # At the very end there is nothing below, so the marker disappearing is the
    # correct outcome — requiring a smaller number would fail a working UI.
    require(
        was is not None and (now is None or now < was),
        f"the overflow count fell away as the view reached the end ({was} -> {now})",
    )

    for name, rows in (("initial", before), ("after click", at_click), ("after wheel", at_wheel)):
        print(f"\n--- {name} frame ---")
        print("\n".join(rows[:ROWS]))

    print(f"\nRESULT: {'PASS' if not failures else 'FAIL'} ({len(failures)} failed)")
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
