"""Summarise a feed recorded with `trippin --dump-feed track.flac feed.jsonl`.

It answers the questions that matter when a Unity show "doesn't move in time"
or "doesn't react" on real music, without rendering anything:

- how often the beat counter jumps (phase corrections / re-locks), and by how
  much (Rx.beatS in Kit.cs smooths these);
- how far the energy clock (clock4[0]) drifts from the beat count (anything
  that dances on it falls off the beat by that many beats);
- the breakdown / drop timeline (drums off, then back) the DropDirector sees;
- the spread of each band level, so a show's reactivity mapping can be sized.

    python tools/feed_report.py feed.jsonl
"""
import json
import sys


def main(path):
    rows = [json.loads(l) for l in open(path) if l.strip()]
    if len(rows) < 120:
        print("feed too short")
        return 1
    dt = 1 / 60
    jumps = []
    for a, b in zip(rows, rows[1:]):
        step = b["beat"] - a["beat"]
        exp = a["bpm"] / 60 * dt
        if abs(step - exp) > 0.15 and abs(step + 4096) > 0.15 and step > -4000:
            jumps.append((b["time"], step - exp))
    print(f"{len(rows)} frames, {len(rows) * dt:.0f} s, bpm {min(r['bpm'] for r in rows):.1f}..{max(r['bpm'] for r in rows):.1f}")
    print(f"beat counter jumps > 0.15 beat in one frame: {len(jumps)}" + (
        "  (first: " + ", ".join(f"{t:.1f}s {d:+.2f}" for t, d in jumps[:6]) + ")" if jumps else ""))
    # Energy clock vs beat: the clock's rate relative to the beat's, per 10 s.
    print("energy clock vs beat (beats gained or lost per 10 s; 0 = it stays on the beat):")
    seg = 600
    for i in range(0, len(rows) - seg, seg):
        a, b = rows[i], rows[i + seg]
        db = (b["beat"] - a["beat"]) % 4096
        dc = (b["clock4"][0] - a["clock4"][0]) % 4096
        print(f"  {a['time']:5.0f}-{b['time']:5.0f} s  beat +{db:5.1f}  clock +{dc:5.1f}  drift {dc - db:+5.1f}")
    # Breakdowns and drops.
    ev, prev = [], True
    for r in rows:
        if r["drums"] != prev:
            ev.append((r["time"], "drums back (drop)" if r["drums"] else "drums out (breakdown)"))
            prev = r["drums"]
    print("drum timeline: " + ("; ".join(f"{t:.0f}s {e}" for t, e in ev) if ev else "drums throughout"))
    for i, name in enumerate(["bass", "mid", "mid-high", "high"]):
        v = sorted(r["lvl4"][i] for r in rows)
        print(f"lvl4[{i}] {name:8s} p10 {v[len(v)//10]:.2f}  p50 {v[len(v)//2]:.2f}  p90 {v[9*len(v)//10]:.2f}")
    kicks = sum(1 for a, b in zip(rows, rows[1:]) if b["hits4"][0] > a["hits4"][0] + 0.3)
    print(f"bass hits: {kicks} ({kicks / (len(rows) * dt) * 60:.0f}/min)")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1]) if len(sys.argv) > 1 else print(__doc__))
