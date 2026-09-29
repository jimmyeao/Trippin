"""Generate the neon_coaster track constants (shaders/scenes/neon_coaster.wgsl).

The track is a closed Fourier curve C(theta) (figure-eight with an over/under
crossing, hills and a lift hill). The ride's timing is physical: a chain lift
at constant slow speed, then v = sqrt(v_crest^2 + 2 g (h_top - h)), so the car
crawls over crests and flies through dips. The shader has no state, so the
map  phi (lap fraction, from the tempo clock)  ->  theta  is precomputed here
and fitted as a Fourier series, as is the bank angle. Towers are cleared from
city cells the track passes through (a bitmask).

Run:  python tools/coaster_track.py   (prints WGSL to paste between the
      GENERATED markers in the scene; --write patches the file in place)
"""
import math
import sys
from pathlib import Path

import numpy as np

TAU = 2 * math.pi

# --- Track shape (x, y, z as Fourier terms of theta) -----------------------
# x: figure-eight lobes; z: the crossing; y: hills, with the lift crest
# near theta ~ 0.25*TAU and the over/under crossing heights far apart.
X = [(1, 0.0, 330.0), (3, 0.0, 45.0)]            # (k, cos coef, sin coef)
Z = [(2, 0.0, 190.0), (1, 60.0, 0.0), (3, 25.0, 0.0)]
Y0 = 95.0
Y = [(1, -20.0, 55.0), (2, 22.0, 0.0), (3, 0.0, 14.0), (5, 8.0, 0.0)]

G = 9.81 * 3.0          # a punchier "gravity" — this is a music video, not a sim
V_CREST = 12.0
LIFT_SPAN = 0.14        # theta/TAU span of the chain lift (constant slow speed)
V_LIFT = 14.0

N = 20000
CELL = 40.0
GRID = 24               # cells per side of the bitmask, centred on the origin
CLEAR = 40.0            # clear towers within this distance of the track (xz)


def fourier(terms, th, deriv=0):
    out = np.zeros_like(th)
    for k, c, s in terms:
        if deriv == 0:
            out += c * np.cos(k * th) + s * np.sin(k * th)
        else:
            out += k * (-c * np.sin(k * th) + s * np.cos(k * th))
    return out


def main():
    th = np.linspace(0.0, TAU, N, endpoint=False)
    x, y, z = fourier(X, th), Y0 + fourier(Y, th), fourier(Z, th)
    dx, dy, dz = fourier(X, th, 1), fourier(Y, th, 1), fourier(Z, th, 1)
    speed_th = np.sqrt(dx * dx + dy * dy + dz * dz)       # |dC/dtheta|
    u = th / TAU
    # The chain lift climbs to the summit (the track's highest point).
    u_top = u[np.argmax(y)]
    lo = (u_top - LIFT_SPAN) % 1.0
    in_lift = ((u - lo) % 1.0) < LIFT_SPAN
    h_top = y.max()
    v = np.sqrt(np.maximum(V_CREST**2 + 2 * G * (h_top - y), 1.0))
    v = np.where(in_lift, V_LIFT, v)
    # Blend the lift's edges so the speed has no step (Fourier-friendly).
    W = 1500
    v = np.convolve(np.concatenate([v[-W:], v, v[:W]]), np.ones(2 * W + 1) / (2 * W + 1), "same")[W:-W]
    dt = speed_th * (TAU / N) / v
    t = np.concatenate([[0.0], np.cumsum(dt)[:-1]])
    lap = dt.sum()
    phi = t / lap

    # Fit the *inverse* map: lap fraction as a function of track position,
    # phi(theta) = theta/TAU + low harmonics. Few smooth terms means the ride
    # speed is smooth by construction (fitting theta(phi) directly rang
    # around the chain-lift edges — 50+ speed wobbles per lap). The shader
    # inverts it with a few Newton steps.
    resid = phi - th / TAU
    K = 8
    warp = []
    for k in range(1, K + 1):
        a = 2 * np.mean(resid * np.cos(k * th))
        b = 2 * np.mean(resid * np.sin(k * th))
        warp.append((k, a, b))
    a0 = np.mean(resid)
    dphi = 1 / TAU + sum(k * (-a * np.sin(k * th) + b * np.cos(k * th)) for k, a, b in warp)
    fit = th / TAU + a0 + sum(a * np.cos(k * th) + b * np.sin(k * th) for k, a, b in warp)
    err = np.abs(fit - phi).max()
    spd = speed_th / dphi
    ext = int((np.diff(np.sign(np.diff(spd))) != 0).sum())
    print(f"// lap length {np.sum(speed_th) * TAU / N:.0f} m, lap time {lap:.1f} s (physical), "
          f"phi fit err {err:.4f}, speed {spd.min() / spd.mean():.2f}..{spd.max() / spd.mean():.2f} x mean, "
          f"{ext} speed extrema/lap", file=sys.stderr)
    assert dphi.min() > 0.0, "fitted phi(theta) not monotonic"

    # Bank: roll into turns from the horizontal curvature and speed.
    ddx = np.gradient(dx, th)
    ddz = np.gradient(dz, th)
    kappa = (dx * ddz - dz * ddx) / np.maximum((dx * dx + dz * dz) ** 1.5, 1e-6)
    roll = np.arctan(v * v * kappa / G) * 0.6
    roll = np.clip(roll, -0.6, 0.6)
    bank = []
    for k in range(1, 9):
        a = 2 * np.mean(roll * np.cos(k * th))
        b = 2 * np.mean(roll * np.sin(k * th))
        bank.append((k, a, b))
    b0 = np.mean(roll)

    # Tower clearance bitmask over GRID x GRID cells.
    pts = np.stack([x, z], axis=1)[::10]
    rows = []
    for j in range(GRID):
        bits = 0
        for i in range(GRID):
            cx = (i - GRID / 2 + 0.5) * CELL
            cz = (j - GRID / 2 + 0.5) * CELL
            d = np.sqrt(((pts - [cx, cz]) ** 2).sum(1)).min()
            if d < CLEAR:
                bits |= 1 << i
        rows.append(bits)

    def terms(name, ts):
        return f"const {name}: array<vec3<f32>, {len(ts)}> = array<vec3<f32>, {len(ts)}>(\n" + \
            ",\n".join(f"    vec3<f32>({k:.1f}, {c:.5f}, {s:.5f})" for k, c, s in ts) + "\n);\n"

    out = "// ---- GENERATED by tools/coaster_track.py — do not edit by hand ----\n"
    out += terms("TX", X) + terms("TY", Y) + terms("TZ", Z)
    out += f"const TY0: f32 = {Y0:.2f};\n"
    out += terms("WARP", warp) + f"const WARP0: f32 = {a0:.5f};\n"
    out += terms("BANK", bank) + f"const BANK0: f32 = {b0:.5f};\n"
    out += f"const CLEAR_GRID: i32 = {GRID};\nconst CLEAR_CELL: f32 = {CELL:.1f};\n"
    out += f"const CLEAR_ROWS: array<u32, {GRID}> = array<u32, {GRID}>(\n    " + \
        ", ".join(f"{r}u" for r in rows) + "\n);\n"
    out += "// ---- END GENERATED ----\n"

    if "--write" in sys.argv:
        p = Path(__file__).resolve().parent.parent / "shaders/scenes/neon_coaster.wgsl"
        s = p.read_text(encoding="utf-8")
        a = s.index("// ---- GENERATED")
        b = s.index("// ---- END GENERATED ----\n") + len("// ---- END GENERATED ----\n")
        p.write_text(s[:a] + out + s[b:], encoding="utf-8", newline="\n")
        print(f"patched {p}", file=sys.stderr)
    else:
        print(out)

    # Height profile summary.
    print(f"// height {y.min():.0f}..{y.max():.0f} m, speed {v.min():.0f}..{v.max():.0f} m/s", file=sys.stderr)
    # Over/under check at the figure-eight crossing(s).
    close = []
    for i in range(0, N, 20):
        d = np.sqrt((x - x[i]) ** 2 + (z - z[i]) ** 2)
        far = np.abs(((th - th[i]) + math.pi) % TAU - math.pi) > 0.6
        j = np.argmin(np.where(far, d, 1e9))
        if d[j] < 6.0:
            close.append(abs(y[i] - y[j]))
    if close:
        print(f"// crossing height separation min {min(close):.1f} m", file=sys.stderr)


if __name__ == "__main__":
    main()
