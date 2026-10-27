# tools/gear_textures.py <Assets/Trippin/Textures>: regenerates the unity_decks prints.
# Faceplate and label textures for unity_decks (see SPEC.md). Coordinates in mm,

# x from the left edge, z from the FRONT edge; image rows run back (top) to front

# (bottom), so Unity's uv (x/W, z/D) maps straight on.

import math, random, sys

from PIL import Image, ImageDraw, ImageFilter, ImageFont



OUT = sys.argv[1]

F = "C:/Windows/Fonts/"

def font(px, bold=False):

    return ImageFont.truetype(F + ("arialbd.ttf" if bold else "arial.ttf"), max(6, int(px)))



class Panel:

    def __init__(self, w_mm, d_mm, px_per_mm, base):

        self.s = px_per_mm; self.W = w_mm; self.D = d_mm

        self.img = Image.new("RGB", (int(w_mm * px_per_mm), int(d_mm * px_per_mm)), base)

        self.d = ImageDraw.Draw(self.img)

        self.ao = Image.new("L", self.img.size, 0)       # contact shadows, blurred later

        self.dao = ImageDraw.Draw(self.ao)

    def p(self, x, z):

        return (x * self.s, (self.D - z) * self.s)

    def rect(self, x0, z0, x1, z1, fill=None, outline=None, width=1.0, r=0.0):

        a = self.p(x0, z1); b = self.p(x1, z0)

        self.d.rounded_rectangle([a, b], radius=r * self.s, fill=fill, outline=outline, width=max(1, int(width * self.s)))

    def circle(self, x, z, r, fill=None, outline=None, width=0.5):

        cx, cy = self.p(x, z); rr = r * self.s

        self.d.ellipse([cx - rr, cy - rr, cx + rr, cy + rr], fill=fill, outline=outline, width=max(1, int(width * self.s)))

    def line(self, x0, z0, x1, z1, fill, width=0.3):

        self.d.line([self.p(x0, z0), self.p(x1, z1)], fill=fill, width=max(1, int(width * self.s)))

    def text(self, x, z, s, size_mm, fill=(225, 225, 225), bold=False, anchor="mm"):

        self.d.text(self.p(x, z), s, font=font(size_mm * self.s, bold), fill=fill, anchor=anchor)

    def shadow_circle(self, x, z, r, amt=150):

        cx, cy = self.p(x, z); rr = r * self.s

        self.dao.ellipse([cx - rr, cy - rr, cx + rr, cy + rr], fill=amt)

    def shadow_rect(self, x0, z0, x1, z1, amt=150):

        a = self.p(x0, z1); b = self.p(x1, z0)

        self.dao.rectangle([a, b], fill=amt)

    def finish(self, name, blur_mm=2.5, grain=4):

        ao = self.ao.filter(ImageFilter.GaussianBlur(blur_mm * self.s))

        dark = Image.new("RGB", self.img.size, (0, 0, 0))

        img = Image.composite(dark, self.img, ao.point(lambda v: int(v * 0.8)))

        # Anodised grain.

        random.seed(1)

        noise = Image.effect_noise(img.size, grain * 6).convert("RGB")

        img = Image.blend(img, Image.composite(noise, img, Image.new("L", img.size, 18)), 1.0)

        img.save(f"{OUT}/{name}.png")

        print(name, img.size)



WHITE = (228, 228, 228)

GREY = (120, 122, 128)



# ---------------- turntable top plate ----------------

def turntable():

    P = Panel(453, 353, 4.5, (24, 24, 26))

    # Platter well (covered by the platter) and its contact shadow.

    P.circle(176, 184, 168, fill=(10, 10, 11))

    P.shadow_circle(176, 184, 171, 200)

    # Tonearm well: a recessed dark disc with a thin bright edge.

    P.circle(375, 265, 54, fill=(30, 30, 32))

    P.circle(375, 265, 52, fill=(12, 12, 13))

    P.shadow_circle(375, 265, 30, 160)

    # Start-stop recess and the speed buttons' recess.

    P.rect(10, 6, 59, 47, fill=(8, 8, 9), r=2)

    P.shadow_rect(12, 8, 57, 45, 170)

    P.rect(59, 8, 111, 22, fill=(8, 8, 9), r=1.5)


    P.text(72.5, 26.5, "33", 3.0, WHITE); P.text(97.5, 26.5, "45", 3.0, WHITE)

    # Power/strobe knob ring.

    P.circle(26, 72, 16, fill=(14, 14, 15)); P.shadow_circle(26, 72, 15, 150)

    for a, lab in ((-50, "OFF"), (40, "ON")):

        r = math.radians(a)

        P.text(26 + 21 * math.sin(r), 72 + 21 * math.cos(r), lab, 2.4, GREY)

    # Target light socket.

    P.circle(262, 22, 9.5, fill=(10, 10, 11)); P.shadow_circle(262, 22, 9, 140)

    # Pitch fader: frame, slot, scale, LED, label.

    P.rect(398, 34, 427, 221, fill=(36, 36, 38), r=2)

    P.rect(401, 37, 424, 218, fill=(14, 14, 15), r=1.5)

    P.rect(410, 44, 415, 211, fill=(2, 2, 2), r=1)

    for k in range(-8, 9):

        z = 127.5 - k * (167 / 16.0)          # -8 at the back, +8 at the front

        long = k % 2 == 0

        P.line(388 if long else 392, z, 397, z, WHITE, 0.35)

        if long:

            lab = ("+" if k > 0 else "-" if k < 0 else "") + str(abs(k))

            P.text(383, z, lab, 3.0, WHITE, anchor="rm")

    P.text(412.5, 26, "pitch adj.", 3.0, WHITE)

    P.text(437, 127.5, "±8%", 2.6, GREY, anchor="lm")

    # Model text, front-right.

    P.text(298, 30, "QUARTZ", 3.4, WHITE, bold=True, anchor="lm")

    P.text(298, 18, "DIRECT DRIVE TURNTABLE SYSTEM", 2.7, WHITE, anchor="lm")

    # 45 adapter recess and hinge marks at the back.

    P.circle(36, 326, 23, fill=(12, 12, 13)); P.shadow_circle(36, 326, 22, 120)

    for hx in (70, 330):

        P.rect(hx - 22, 340, hx + 22, 353, fill=(16, 16, 17))

    # Anti-skate and arm rest shadows.

    P.shadow_circle(412, 232, 9, 140); P.shadow_circle(392, 197, 5, 140)

    P.finish("GearTTPlate")



# ---------------- 4-channel club mixer ----------------

def knob_scale(P, x, z, r, ticks=11, centre=False, lab=None, labz=None):

    for i in range(ticks):

        a = math.radians(-135 + 270 * i / (ticks - 1))

        r0, r1 = r + 1.2, r + (3.0 if i in (0, ticks - 1) or (centre and i == ticks // 2) else 2.0)

        P.line(x + r0 * math.sin(a), z + r0 * math.cos(a), x + r1 * math.sin(a), z + r1 * math.cos(a), WHITE, 0.35)

    P.shadow_circle(x, z, r + 0.5, 170)

    if lab:

        P.text(x, labz if labz is not None else z - r - 4.5, lab, 2.6, WHITE)



def mixer():

    P = Panel(333, 414, 4.6, (21, 21, 23))

    cols = [76 + 45 * i for i in range(5)]

    for x in cols:

        P.line(x, 8, x, 406, (58, 58, 62), 0.5)

    P.line(4, 190, 76, 190, (58, 58, 62), 0.5); P.line(256, 160, 329, 160, (58, 58, 62), 0.5)

    P.line(76, 45, 256, 45, (58, 58, 62), 0.5)

    for c in range(4):

        cx = 76 + 45 * c + 22.5

        P.text(cx, 398, str(c + 1), 6.0, WHITE, bold=True)

        # Input selector: a three-way toggle slot.

        P.rect(cx - 9, 384, cx + 9, 390, fill=(8, 8, 9), r=1)

        for i, lab in enumerate(("DIGITAL", "LINE", "PHONO")):

            P.text(cx - 8 + 8 * i, 380.5, lab, 1.6, GREY)

        knob_scale(P, cx, 360, 8, centre=False, lab="TRIM", labz=349)

        for z, lab in ((330, "HI"), (300, "MID"), (270, "LOW")):

            knob_scale(P, cx - 3, z, 8, centre=True)

            P.text(cx - 15, z, lab, 2.4, WHITE, anchor="lm")

        # Channel meter window, 15 segments.

        mx = cx + 14

        P.rect(mx - 2.2, 260, mx + 2.2, 354, fill=(5, 5, 6), r=0.8)

        for i in range(15):

            z0 = 262 + i * 6.1

            P.rect(mx - 1.5, z0, mx + 1.5, z0 + 4.4, fill=(28, 34, 28) if i < 9 else (40, 32, 22) if i < 13 else (44, 22, 22))

        knob_scale(P, cx, 235, 10, ticks=11, centre=True, lab="COLOR", labz=221)

        # Cue button recess.

        P.rect(cx - 9, 199, cx + 9, 211, fill=(6, 6, 7), r=2)

        P.shadow_rect(cx - 8, 200, cx + 8, 210, 160)

        P.text(cx, 195, "CUE", 2.2, WHITE)

        # Channel fader: slot and scale.

        P.rect(cx - 1.6, 64, cx + 1.6, 176, fill=(2, 2, 2), r=1)

        for i in range(11):

            z = 70 + i * 10

            P.line(cx + 5, z, cx + (10 if i % 5 == 0 else 8), z, WHITE, 0.3)

            P.line(cx - 5, z, cx - (10 if i % 5 == 0 else 8), z, WHITE, 0.3)

        P.text(cx + 12, 170, "10", 1.8, GREY, anchor="lm"); P.text(cx + 12, 70, "0", 1.8, GREY, anchor="lm")

        # Crossfader assign A / THRU / B.

        P.rect(cx - 8, 50, cx + 8, 55, fill=(8, 8, 9), r=1)

        P.text(cx - 7, 58, "A", 1.9, GREY); P.text(cx, 58, "THRU", 1.6, GREY); P.text(cx + 7, 58, "B", 1.9, GREY)

    # Crossfader.

    P.rect(166 - 27, 23.4, 166 + 27, 26.6, fill=(2, 2, 2), r=1)

    for i in range(9):

        x = 166 - 24 + 6 * i

        P.line(x, 30, x, 33 if i % 4 == 0 else 32, WHITE, 0.3)

    P.text(132, 25, "A", 3.0, WHITE, bold=True); P.text(200, 25, "B", 3.0, WHITE, bold=True)

    P.text(166, 38, "CROSS FADER", 2.3, WHITE)

    # Left column: mic, SOUND COLOR FX, headphones.

    P.text(38, 398, "MIC", 3.0, WHITE, bold=True)

    knob_scale(P, 22, 375, 7, lab="LEVEL", labz=364); knob_scale(P, 54, 375, 6, centre=True, lab="HI", labz=364)

    knob_scale(P, 54, 345, 6, centre=True, lab="LOW", labz=334); knob_scale(P, 22, 345, 6, lab="BOOTH", labz=334)

    P.text(38, 278, "SOUND COLOR FX", 2.6, WHITE, bold=True)

    for i, lab in enumerate(("SPACE", "DUB ECHO", "SWEEP", "NOISE", "CRUSH", "FILTER")):

        bx = 18 + (i % 3) * 20; bz = 262 - (i // 3) * 16

        P.rect(bx - 7.5, bz - 3.5, bx + 7.5, bz + 3.5, fill=(6, 6, 7), r=1.2)

        P.text(bx, bz - 6.5, lab, 1.7, WHITE)

    knob_scale(P, 38, 212, 8, centre=True, lab="PARAMETER", labz=200)

    P.text(38, 176, "HEADPHONES", 2.6, WHITE, bold=True)

    knob_scale(P, 38, 150, 8, centre=True, lab="MIXING", labz=138)

    P.text(24, 160, "CUE", 1.8, GREY); P.text(52, 160, "MASTER", 1.8, GREY)

    knob_scale(P, 38, 112, 8, lab="LEVEL", labz=100)

    P.rect(30, 66, 46, 82, fill=(8, 8, 9), r=2); P.text(38, 61, "CUE", 2.2, WHITE)

    # Right column: master, meters, BEAT FX.

    P.text(292, 398, "MASTER", 2.8, WHITE, bold=True)

    knob_scale(P, 278, 380, 9, lab="LEVEL", labz=367); knob_scale(P, 312, 380, 7, lab="BOOTH", labz=367)

    for mx in (284, 300):

        P.rect(mx - 2.2, 248, mx + 2.2, 344, fill=(5, 5, 6), r=0.8)

        for i in range(15):

            z0 = 250 + i * 6.2

            P.rect(mx - 1.5, z0, mx + 1.5, z0 + 4.5, fill=(28, 34, 28) if i < 9 else (40, 32, 22) if i < 13 else (44, 22, 22))

    P.text(284, 241, "L", 2.2, WHITE); P.text(300, 241, "R", 2.2, WHITE)

    for i, db in enumerate(("+15", "+10", "+5", "0", "-5", "-10", "-15", "-24")):

        P.text(318, 341 - i * 13, db, 1.7, GREY, anchor="lm")

    P.text(292, 232, "BEAT FX", 2.8, WHITE, bold=True)

    P.rect(266, 196, 318, 222, fill=(4, 4, 5), r=1.5)          # the OLED display

    for i, lab in enumerate(("‹", "BEAT", "›")):

        P.rect(268 + i * 17, 178, 282 + i * 17, 186, fill=(6, 6, 7), r=1.2)

        P.text(275 + i * 17, 173, lab, 1.8, WHITE)

    knob_scale(P, 278, 148, 8, ticks=13, lab="FX SELECT", labz=136)

    knob_scale(P, 312, 148, 7, ticks=5, lab="CH SELECT", labz=136)

    knob_scale(P, 292, 106, 9, centre=True, lab="LEVEL/DEPTH", labz=92)

    P.circle(304, 52, 11, fill=(6, 6, 7)); P.shadow_circle(304, 52, 10, 170)

    P.text(304, 36, "ON/OFF", 2.4, WHITE)

    P.text(272, 58, "TAP", 2.0, WHITE); P.rect(266, 46, 278, 54, fill=(6, 6, 7), r=1.2)

    P.text(166, 10, "4-CHANNEL PROFESSIONAL DJ MIXER", 2.4, GREY)

    P.finish("GearMixPanel")



# The BEAT FX OLED: emissive, white on black.

def fx_screen():

    img = Image.new("RGB", (520, 260), (0, 0, 0)); d = ImageDraw.Draw(img)

    d.text((26, 18), "ECHO", font=font(64, True), fill=(235, 240, 255))

    d.text((360, 26), "3/4", font=font(54, True), fill=(140, 200, 255))

    d.text((26, 120), "CH 3", font=font(38), fill=(170, 175, 190))

    d.text((240, 120), "128.0 BPM", font=font(38), fill=(170, 175, 190))

    d.rectangle([26, 196, 494, 214], outline=(120, 125, 140), width=2)

    d.rectangle([26, 196, 330, 214], fill=(200, 210, 230))

    img.save(f"{OUT}/GearFxScreen.png"); print("GearFxScreen")



# Record labels: generic, readable, several colours.

def labels():

    titles = [("NIGHT SHIFT", "Deep Into The Groove", "(Extended Mix)", (196, 34, 40)),

              ("LOW END RECORDS", "Warehouse Theory", "(Original Mix)", (235, 232, 222)),

              ("SUBMERGE", "Blue Hour", "(Club Dub)", (34, 70, 150)),

              ("ACID HOUSE CO.", "Smiley Faces", "(Peak Time Mix)", (240, 190, 30))]

    for i, (lab, title, mix, col) in enumerate(titles):

        n = 1024; img = Image.new("RGB", (n, n), (6, 6, 6)); d = ImageDraw.Draw(img)

        c = n / 2

        d.ellipse([0, 0, n, n], fill=col)

        ink = (20, 20, 20) if sum(col) > 400 else (240, 240, 240)

        d.ellipse([c - 470, c - 470, c + 470, c + 470], outline=ink, width=4)

        d.text((c, 190), lab, font=font(78, True), fill=ink, anchor="mm")

        d.text((c, 290), "SIDE A", font=font(46), fill=ink, anchor="mm")

        d.text((c, 650), title, font=font(62, True), fill=ink, anchor="mm")

        d.text((c, 730), mix, font=font(48), fill=ink, anchor="mm")

        d.text((210, 512), "33 1/3", font=font(46, True), fill=ink, anchor="mm")

        d.text((210, 568), "RPM", font=font(36), fill=ink, anchor="mm")

        d.text((814, 512), "LE-" + str(1024 + i * 37), font=font(40), fill=ink, anchor="mm")

        d.text((c, 850), "(P) 2026  ALL RIGHTS RESERVED", font=font(28), fill=ink, anchor="mm")

        d.ellipse([c - 22, c - 22, c + 22, c + 22], fill=(5, 5, 5))

        img.save(f"{OUT}/GearLabel{i}.png"); print("GearLabel", i)



# Slipmat: black felt with a thin printed ring.

def slipmat():

    n = 1024; img = Image.new("RGB", (n, n), (16, 16, 17)); d = ImageDraw.Draw(img); c = n / 2

    d.ellipse([c - 500, c - 500, c + 500, c + 500], outline=(200, 200, 200), width=6)

    d.ellipse([c - 486, c - 486, c + 486, c + 486], outline=(200, 200, 200), width=2)

    noise = Image.effect_noise((n, n), 30).convert("RGB")

    img = Image.blend(img, noise, 0.06)

    img.save(f"{OUT}/GearSlipmat.png"); print("GearSlipmat")



# ---------------- media player (CDJ-style), 320 x 406 mm ----------------
def cdj():
    P = Panel(320, 406, 4.6, (17, 17, 19))
    jx, jz = 165, 160
    # Jog well: a dark surround with a fine bright edge and its shadow.
    P.circle(jx, jz, 110, fill=(26, 26, 29)); P.circle(jx, jz, 107, fill=(7, 7, 8))
    P.shadow_circle(jx, jz, 106, 190)
    for a, lab in ((-128, "REV"), (128, "FWD")):
        r = math.radians(a)
        P.text(jx + 118 * math.sin(r), jz + 118 * math.cos(r), lab, 2.6, WHITE)
    
    # PLAY/PAUSE and CUE, bottom-left.
    for z, lab in ((40, "PLAY/PAUSE"), (95, "CUE")):
        P.circle(35, z, 21, fill=(8, 8, 9)); P.shadow_circle(35, z, 19, 170)
        P.text(35, z - 25, lab, 2.4, WHITE)
    # Left column: search, track search, direction, loop.
    for z, lab in ((128, "SEARCH"), (158, "TRACK SEARCH")):
        for x in (22, 47):
            P.rect(x - 9, z - 5, x + 9, z + 5, fill=(7, 7, 8), r=1.5)
        P.text(34.5, z + 9, lab, 2.0, WHITE)
        P.text(22, z, "<<", 2.0, GREY); P.text(47, z, ">>", 2.0, GREY)
    P.rect(26, 188, 42, 214, fill=(7, 7, 8), r=2); P.text(34, 219, "DIRECTION", 2.0, WHITE)
    P.text(20, 182, "REV", 1.7, GREY); P.text(48, 182, "FWD", 1.7, GREY)
    for x, lab in ((24, "IN/CUE"), (48, "OUT")):
        P.circle(x, 262, 9, fill=(7, 7, 8)); P.text(x, 250, lab, 1.8, WHITE)
    P.rect(26, 232, 46, 240, fill=(7, 7, 8), r=1.5); P.text(36, 227, "RELOOP/EXIT", 1.7, WHITE)
    P.text(36, 275, "LOOP", 2.2, WHITE, bold=True)
    # Source buttons left of the display.
    for i, lab in enumerate(("SD", "USB", "LINK", "BROWSE")):
        z = 392 - i * 22
        P.rect(22, z - 5, 44, z + 5, fill=(7, 7, 8), r=1.5); P.text(14, z, lab, 1.6, GREY, anchor="rm")
    # Hot cue pads under the display, A..H.
    for i in range(8):
        x = 78 + i * 26
        P.rect(x - 11, 277, x + 11, 290, fill=(6, 6, 7), r=2)
        P.text(x, 271, "ABCDEFGH"[i], 2.0, WHITE)
    P.text(56, 283.5, "HOT CUE", 1.8, WHITE, anchor="rm")
    # Right column: tempo, sync, jog mode.
    P.rect(290, 30, 305, 182, fill=(26, 26, 29), r=2); P.rect(295, 36, 300, 176, fill=(2, 2, 2), r=1)
    for k in range(-10, 11, 2):
        z = 106 - k * 6.8
        P.line(284, z, 288, z, WHITE, 0.3)
        if k % 10 == 0: P.text(282, z, ("+" if k > 0 else "-" if k < 0 else "") + str(abs(k)), 2.0, WHITE, anchor="rm")
    P.text(297.5, 22, "TEMPO", 2.4, WHITE)
    for z, lab in ((192, "MASTER TEMPO"), (210, "TEMPO ±6/10/16/WIDE")):
        P.rect(290, z - 4, 305, z + 4, fill=(7, 7, 8), r=1.5); P.text(286, z, lab, 1.6, GREY, anchor="rm")
    for x, lab in ((282, "BEAT SYNC"), (305, "MASTER")):
        P.rect(x - 9, 230, x + 9, 240, fill=(7, 7, 8), r=1.5); P.text(x, 245, lab, 1.7, WHITE)
    P.rect(276, 257, 294, 265, fill=(7, 7, 8), r=1.5); P.text(285, 270, "JOG MODE", 1.7, WHITE)
    P.text(285, 252, "VINYL  CDJ", 1.5, GREY)
    knob_scale(P, 306, 284, 6, ticks=9, lab="JOG ADJUST", labz=275)
    # Right of the display: browse knob, back, tag track.
    P.circle(300, 372, 15, fill=(8, 8, 9)); P.shadow_circle(300, 372, 13, 160)
    P.rect(292, 338, 308, 345, fill=(7, 7, 8), r=1.5); P.text(300, 333, "BACK", 1.6, WHITE)
    P.rect(292, 316, 308, 323, fill=(7, 7, 8), r=1.5); P.text(300, 311, "TAG TRACK", 1.5, WHITE)
    # The display pod sits on the plate here; its shadow.
    P.shadow_rect(62, 298, 278, 406, 160)
    P.text(245, 12, "MULTI PLAYER", 3.0, WHITE, bold=True)
    P.finish("GearCdjPlate")

# The player's screen: everything but the waveforms, which the shader draws.
def cdj_screens():
    for i, (title, artist, bpm, key) in enumerate((("Deep Into The Groove (Extended Mix)", "Night Shift", "124.00", "8A"),
                                                   ("Warehouse Theory (Original Mix)", "Low End", "126.00", "5A"))):
        img = Image.new("RGB", (1024, 576), (0, 0, 0)); d = ImageDraw.Draw(img)
        d.rectangle([0, 0, 1023, 54], fill=(18, 22, 34))
        d.text((16, 27), "♪", font=ImageFont.truetype(F + "seguisym.ttf", 34), fill=(120, 180, 255), anchor="lm")
        d.text((56, 27), title, font=font(28, True), fill=(235, 240, 250), anchor="lm")
        d.text((1008, 27), artist, font=font(24), fill=(150, 160, 180), anchor="rm")
        # Bottom info bar: deck number, time, tempo, BPM, key.
        d.rectangle([0, 470, 1023, 575], fill=(10, 12, 18))
        d.rectangle([14, 482, 84, 562], outline=(120, 170, 255), width=3)
        d.text((49, 522), str(i + 1), font=font(56, True), fill=(140, 190, 255), anchor="mm")
        d.text((110, 488), "REMAIN", font=font(18), fill=(130, 140, 160))
        d.text((110, 512), "04:12", font=font(40, True), fill=(235, 240, 250))
        d.text((230, 520), ".3", font=font(26), fill=(200, 210, 230))
        d.text((330, 488), "TEMPO", font=font(18), fill=(130, 140, 160))
        d.text((330, 512), "+0.0%", font=font(36, True), fill=(235, 240, 250))
        d.text((500, 488), "BPM", font=font(18), fill=(130, 140, 160))
        d.text((500, 508), bpm, font=font(44, True), fill=(235, 240, 250))
        d.text((690, 488), "KEY", font=font(18), fill=(130, 140, 160))
        d.text((690, 512), key, font=font(36, True), fill=(120, 230, 160))
        d.text((790, 488), "QUANTIZE", font=font(18), fill=(250, 170, 60))
        d.text((790, 516), "MASTER", font=font(24, True), fill=(250, 170, 60))
        img.save(f"{OUT}/GearCdjScreen{i}.png"); print("GearCdjScreen", i)

def start_button():

    img = Image.new("RGB", (600, 490), (150, 152, 156)); d = ImageDraw.Draw(img)

    d.rectangle([0, 0, 599, 489], outline=(90, 90, 94), width=10)

    d.text((300, 200), "START", font=font(84), fill=(30, 30, 32), anchor="mm")
    d.text((300, 300), "STOP", font=font(84), fill=(30, 30, 32), anchor="mm")

    noise = Image.effect_noise(img.size, 20).convert("RGB")

    Image.blend(img, noise, 0.05).save(f"{OUT}/GearStartBtn.png"); print("GearStartBtn")



turntable(); start_button(); cdj(); cdj_screens(); mixer(); fx_screen(); labels(); slipmat()

