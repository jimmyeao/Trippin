"""Build a `.trippinpack` (an effects pack, see src/packs.rs) from a pack folder.

    python tools/pack_build.py packs/night-city [--out dist/]

The pack folder holds pack.json, scenes/*.wgsl, videos/*.tvid (packed by
tools/video_pack.py), scene_tags.json and scene_energy.json. Writes
`<id>-<version>.trippinpack` (a zip, stored: the clips are JPEG already) and
prints its size and sha256 for the catalog in src/packs.rs.
"""
import argparse, hashlib, json, os, sys, zipfile

ALLOWED = ('pack.json', 'scene_tags.json', 'scene_energy.json')


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('pack')
    ap.add_argument('--out', default='dist')
    a = ap.parse_args()
    m = json.load(open(os.path.join(a.pack, 'pack.json'), encoding='utf-8-sig'))
    scenes = sorted(f for f in os.listdir(os.path.join(a.pack, 'scenes')) if f.endswith('.wgsl'))
    vdir = os.path.join(a.pack, 'videos')
    videos = sorted(f for f in os.listdir(vdir) if f.endswith('.tvid')) if os.path.isdir(vdir) else []
    # A video scene without its clip would never rotate: refuse.
    for s in scenes:
        if s.startswith('video_') and s[6:-5] + '.tvid' not in videos:
            sys.exit(f'{s}: no videos/{s[6:-5]}.tvid')
    os.makedirs(a.out, exist_ok=True)
    out = os.path.join(a.out, f"{m['id']}-{m['version']}.trippinpack")
    with zipfile.ZipFile(out, 'w', zipfile.ZIP_STORED) as z:
        for f in ALLOWED:
            p = os.path.join(a.pack, f)
            if os.path.exists(p):
                z.write(p, f)
        for f in scenes:
            z.write(os.path.join(a.pack, 'scenes', f), 'scenes/' + f)
        for f in videos:
            z.write(os.path.join(vdir, f), 'videos/' + f)
            # The clip's own hits (tools/video_beats.py), when it has a rhythm.
            beats = os.path.join(vdir, f[:-5] + '.beats.json')
            if os.path.exists(beats):
                z.write(beats, 'videos/' + f[:-5] + '.beats.json')
    h = hashlib.sha256()
    with open(out, 'rb') as f:
        for b in iter(lambda: f.read(1 << 20), b''):
            h.update(b)
    print(f'{out}: {len(scenes)} scenes, {os.path.getsize(out) >> 20} MB, sha256 {h.hexdigest()}')


if __name__ == '__main__':
    main()
