"""Record one 1.14d session for replay-diff (specs/tools/replay-diff.md §2):
record_state.py unchanged (every option, the state-1 snapshots, pokes,
sends, the frame-anchored input) plus the C->S tap of c2s_tap.py, so one
run of the game gives both the input stream per server frame and the
state after every tick.

    record_replay.py [record_state.py options]     (Windows Python / Wine)
    record_replay.py --selftest                     (any OS)

Our own code. Nothing here is derived from Blizzard code.
"""

import os
import sys

sys.dont_write_bytecode = True
HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "..", "trace-recorder"))
sys.path.insert(0, HERE)
import c2s_tap  # noqa: E402
import record_state  # noqa: E402

TOOL_SUFFIX = " + replay-diff c2s_tap 0.1.0"


def patch():
    """record_state.main() builds its recorder with make_recorder(rt): wrap it
    so the tap attaches first (the input, poke and send layers wrap it)."""
    base_make = record_state.make_recorder

    def make(rt):
        base = base_make(rt)

        class ReplayRecorder(base):
            def __init__(self, *a, **k):
                super().__init__(*a, **k)
                c2s_tap.attach(self, rt)

            def kill(self):
                # run()'s finally kills the game, then writes the footer notes
                if not getattr(self, "_tap_noted", False):
                    self._tap_noted = True
                    self.notes.append(f"c2s tap: {self.c2s_tap.count} message(s)")
                return super().kill()

        return ReplayRecorder

    record_state.make_recorder = make
    record_state.TOOL = record_state.TOOL + TOOL_SUFFIX


def main():
    if "--selftest" in sys.argv[1:]:
        c2s_tap.selftest()
        return
    patch()
    record_state.main()


if __name__ == "__main__":
    main()
