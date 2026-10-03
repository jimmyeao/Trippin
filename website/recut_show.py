"""Re-cuts a Trippin timeline's scene track from the song's own structure.

    python recut_show.py "<in.json>" "<out.json>"

Keeps every non-scene cue, drops text cues and the half-beat A/B stutter, and lays new
scene cuts on phrase boundaries: slow in breakdowns, accelerating through builds
(8,4,2,1,1 beats), a fresh hero scene on every drop, one-bar cuts through the peaks.
"""
import json, sys, random, pathlib

ROOT = pathlib.Path(__file__).resolve().parent.parent
SCENES = {p.stem for p in (ROOT / "shaders/scenes").glob("*.wgsl")}

HERO = """stage_rig laser_show neon_coaster rooftop_city glass_monoliths megastructure storm_front neon_alley
warehouse_haze laser_cavern infinity_room chrome_ferro gyro_core kifs_cathedral fire_mandala spiral_galaxy
crystal_cave canyon_run subway_rush prism_field rave_hall arch_run sunburst beam_sweep lattice bass_blocks
gyroid_drift torus_dance cathedral light_trails sparks vortex""".split()
GROOVE = """city_rain bokeh_lights nebula eq_skyline ink fluid dot_wave bubble_room wire_terrain synthwave polar_bloom
helix plasma comets ribbons orbiters stardrive moire echo_tunnel cosmic_nest julia_portal kaleido voronoi_pulse
warp_grid sunset_waves caustics glow_surf firefly_forest deep_blue orbit_night chrome_bloom led_wall tunnel
desert_highway salt_flats""".split()
CALM = """aurora_veil aurora_wave deep_blue glacier_cave rain_window orbit_night salt_flats cathedral firefly_forest
glow_surf bokeh_lights nebula ocean caustics sunset_waves infinity_room chrome_bloom light_trails clouds""".split()


class Pool:
    """Hands out scenes without repeating until the pool is used up."""
    def __init__(self, names, rng, recent):
        self.names = [n for n in names if n in SCENES]
        self.rng, self.recent, self.q = rng, recent, []

    def take(self):
        if not self.q:
            self.q = self.names[:]
            self.rng.shuffle(self.q)
        for i, n in enumerate(self.q):
            if n not in self.recent:
                self.q.pop(i)
                break
        else:
            n = self.q.pop(0)
        self.recent.append(n)
        del self.recent[:-14]
        return n


def phrase_energy(c, n_phr):
    ov, beat = c["overview"], 60 / c["bpm"]
    def e(b0, b1):
        t0, t1 = c["first_beat"] + b0 * beat, c["first_beat"] + b1 * beat
        a = int(t0 / c["duration_s"] * len(ov)); b = max(a + 1, int(t1 / c["duration_s"] * len(ov)))
        return sum(ov[a:b]) / len(ov[a:b])
    v = [e(i * 16, (i + 1) * 16) for i in range(n_phr)]
    m = max(v)
    return [x / m for x in v]


def main(src, dst):
    j = json.load(open(src, encoding="utf-8-sig"))
    c = j["clips"][0]
    total = round(c["duration_s"] * c["bpm"] / 60)
    n_phr = total // 16 + 1
    v = phrase_energy(c, n_phr)
    rng, recent = random.Random(125), []
    hero, groove, calm = (Pool(p, rng, recent) for p in (HERO, GROOVE, CALM))

    cuts = []  # (beat, length, scene)
    for i in range(n_phr):
        b0 = i * 16
        if b0 >= total - 4:
            break
        left = min(16, total - b0)
        cur, nxt = v[i], v[i + 1] if i + 1 < n_phr else 0.5
        prev_hi = i > 0 and v[i - 1] >= 0.8
        if i == n_phr - 1 or left < 16:
            plan = [(left, calm)]
        elif cur < 0.40:
            plan = [(8, calm), (8, calm)]
        elif nxt - cur >= 0.12 and nxt >= 0.8:           # hard build into a drop
            plan = [(8, groove), (4, groove), (2, hero), (1, hero), (1, hero)]
        elif cur >= 0.9:                                   # peak: one bar per scene
            plan = [(4, hero)] * 4
        elif cur >= 0.8:                                   # drop / high
            plan = [(4, hero)] * 4 if not prev_hi else [(8, hero), (8, hero)]
        elif cur >= 0.6:
            plan = [(8, groove), (8, groove)]
        else:
            plan = [(8, calm), (8, calm)]
        b = b0
        for length, pool in plan:
            cuts.append((b, length, pool.take()))
            b += length
    # the very last phrase fades out on a calm scene
    keep = []
    for x in j["cues"]:
        k = next(iter(x["kind"])) if isinstance(x["kind"], dict) else x["kind"]
        if k in ("Scene", "Text"):
            continue
        keep.append(x)
    for b, length, name in cuts:
        keep.append({"clip": 0, "beat": float(b), "beats": float(length), "kind": {"Scene": name}})
    keep.sort(key=lambda x: (x["beat"], 0 if isinstance(x["kind"], dict) and "Scene" in x["kind"] else 1))
    j["cues"] = keep
    j["name"] = j["name"] + " (clean cut)"
    json.dump(j, open(dst, "w", encoding="utf-8"))
    print(len(cuts), "scene cuts,", len(keep), "cues total")
    for b, l, n in cuts[:14]: print(f"  beat {b:>5.0f} x{l:<2} {n}")


if __name__ == "__main__":
    main(*sys.argv[1:3])
