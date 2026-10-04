"""Builds site/*.html from pages/*.html (first line: title|description|slug-for-og).
Run: python build.py   — then deploy website/site/ to /var/www/trippin.club/"""
import pathlib, re, time
VER = str(int(time.time()))  # cache-bust css/js behind Cloudflare
root = pathlib.Path(__file__).parent
NAV = [("features.html","Features"),("scenes.html","Scenes"),("whats-new.html","What's new"),("remote.html","Remote app"),("download.html","Download")]
SHELL = """<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{title}</title>
<meta name="description" content="{desc}">
<meta name="theme-color" content="#07060d">
<meta property="og:title" content="{title}">
<meta property="og:description" content="{desc}">
<meta property="og:type" content="website">
<meta property="og:url" content="https://trippin.club/{file}">
<meta property="og:image" content="https://trippin.club/img/hero.jpg">
<meta name="twitter:card" content="summary_large_image">
<link rel="canonical" href="https://trippin.club/{canon}">
<link rel="icon" href="img/logo.png">
<link rel="stylesheet" href="style.css?v={ver}">
</head>
<body>
<nav id="nav">
  <div class="wrap">
    <a class="brand" href="./"><img src="img/logo.png" alt="">Trippin</a>
    <ul>{links}</ul>
    <a class="btn primary" href="download.html">Download</a>
    <button class="menu" aria-label="Menu" onclick="document.getElementById('nav').classList.toggle('open')"><svg viewBox="0 0 24 24"><path d="M4 7h16M4 12h16M4 17h16"/></svg></button>
  </div>
</nav>
{body}
<footer>
  <div class="wrap">
    <span>© <span id="yr">2026</span> Trippin · trippin.club</span>
    <span><a href="remote.html">Remote app</a> · <a href="privacy.html">Privacy</a> · <a href="https://github.com/jimmyeao/Trippin">GitHub</a> · <a href="https://github.com/jimmyeao/Trippin/issues">Support</a></span>
  </div>
</footer>
<script src="site.js?v={ver}"></script>
</body>
</html>
"""
for src in sorted((root/"pages").glob("*.html")):
    first, body = src.read_text(encoding="utf8").split("\n", 1)
    title, desc, _ = first.split("|")
    name = src.name
    links = "".join(f'<li><a href="{h}"{" class=on" if h==name else ""}>{t}</a></li>' for h, t in NAV)
    out = SHELL.format(title=title, desc=desc, file=name, canon="" if name=="index.html" else name, links=links, body=body, ver=VER)
    (root/"site"/name).write_text(out, encoding="utf8")
    print("built", name)
