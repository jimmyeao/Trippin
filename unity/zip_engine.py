import os, zipfile, hashlib

def zipdir(src, out, exec_prefix=None):
    with zipfile.ZipFile(out, 'w', zipfile.ZIP_DEFLATED, compresslevel=9) as z:
        for root, dirs, files in os.walk(src):
            dirs[:] = [d for d in dirs if not d.endswith('_DoNotShip') and d != 'tour']
            for f in files:
                if '_DoNotShip' in f:
                    continue
                p = os.path.join(root, f)
                arc = os.path.relpath(p, src).replace(os.sep, '/')
                zi = zipfile.ZipInfo.from_file(p, arc)
                if exec_prefix and arc.startswith(exec_prefix):
                    zi.external_attr = (0o755 << 16)
                with open(p, 'rb') as fh:
                    z.writestr(zi, fh.read(), zipfile.ZIP_DEFLATED, compresslevel=9)
    h = hashlib.sha256(open(out, 'rb').read()).hexdigest()
    print(f"{out}: {os.path.getsize(out)/1e6:.1f} MB sha256={h}")

os.chdir('TrippinStage')
zipdir('Build', 'TrippinEngine-windows-x64-v13.zip')
zipdir('BuildMac', 'TrippinEngine-macos-v13.zip', exec_prefix='TrippinStage.app/Contents/MacOS/')
