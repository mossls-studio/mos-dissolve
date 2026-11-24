"""mos_消散 v1.0.0 外部辅助程序协议、数值与视觉样张自测。

用法：
  python tools/test_helper.py [effectcraft-helper-mos-dissolve.exe 路径]
"""

import array
import json
import math
import os
import queue
import struct
import subprocess
import sys
import tempfile
import threading
import time
import zlib

PROTO = 1
EFFECT = "mos.dissolve"
HERE = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
OUT_DIR = os.path.join(HERE, "tests", "out")


class Helper:
    def __init__(self, exe, env_extra=None):
        env = dict(os.environ)
        if env_extra:
            env.update(env_extra)
        self.proc = subprocess.Popen(
            [exe, "--effectcraft-helper"],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            bufsize=0,
            env=env,
        )
        self.q = queue.Queue()
        threading.Thread(target=self._read, daemon=True).start()

    def _read(self):
        for raw in self.proc.stdout:
            line = raw.decode("utf-8", "replace").strip()
            if not line:
                continue
            if line.startswith("@ec-helper "):
                line = line[len("@ec-helper "):]
            try:
                self.q.put(json.loads(line))
            except ValueError:
                self.q.put({"op": "@@raw", "line": line})

    def send(self, obj):
        self.proc.stdin.write((json.dumps(obj) + "\n").encode("utf-8"))
        self.proc.stdin.flush()

    def recv(self, timeout=15.0):
        return self.q.get(timeout=timeout)

    def close(self):
        try:
            self.send({"op": "shutdown"})
        except OSError:
            pass
        try:
            self.proc.stdin.close()
        except OSError:
            pass
        try:
            self.proc.wait(timeout=5)
        except subprocess.TimeoutExpired:
            self.proc.kill()


def make_params(**overrides):
    # Matches flat indices 0..35 in helper/src/dissolve.rs
    defaults = [
        0.0, 35.0, 18.0, 45.0, 35.0,                    # 0..4: dissolve
        0.0, 0.0, 15.0, 80.0, -1.0, 0.0, 0.0,           # 5..11: mask
        55.0, 3.2, 50.0, 0.0, 1.0,                      # 12..16: particles & pixels
        -15.0, 160.0, -25.0, 45.0, 55.0, 75.0, 45.0,    # 17..23: wind & physics
        0.0, 25.0, 0.0, 1.0, 0.55, 0.2, 1.0, 75.0, 0.0, # 24..32: lifetime & color (27..30 = end_color RGBA)
        1.0, 20.0, 100.0,                               # 33..35: output
    ]
    idx = {
        "mode": 0, "progress": 1, "feather": 2, "lifetime": 3, "life_random": 4,
        "mask_type": 5, "mask_angle": 6, "mask_jitter": 7, "noise_scale": 8,
        "mask_layer": 9, "mask_channel": 10, "mask_invert": 11,
        "particle_density": 12, "particle_size": 13, "size_random": 14,
        "particle_shape": 15, "pixel_block": 16,
        "wind_angle": 17, "wind_strength": 18, "gravity": 19, "scatter": 20,
        "turbulence_amp": 21, "turbulence_scale": 22, "speed_random": 23,
        "fade_out_curve": 24, "size_over_life": 25, "color_fade_enable": 26,
        "end_color_r": 27, "end_color_g": 28, "end_color_b": 29, "end_color_a": 30,
        "color_fade_mix": 31, "additive_glow": 32,
        "seed": 33, "time_evolve": 34, "mix": 35,
    }
    p = list(defaults)
    for k, v in overrides.items():
        if k == "end_color":
            p[27], p[28], p[29], p[30] = [float(c) for c in v]
        else:
            p[idx[k]] = float(v)
    return p


def write_frame(fh, offset, floats):
    arr = array.array("f", floats)
    if sys.byteorder != "little":
        arr.byteswap()
    fh.seek(offset)
    fh.write(arr.tobytes())
    fh.flush()


def read_frame(fh, offset, n_floats):
    fh.seek(offset)
    raw = fh.read(n_floats * 4)
    arr = array.array("f")
    arr.frombytes(raw)
    if sys.byteorder != "little":
        arr.byteswap()
    return arr


def save_png(path, w, h, premul_rgba, bg=(0.06, 0.07, 0.10)):
    """Save premultiplied RGBA f32 buffer composited over a dark backdrop as PNG."""
    rows = bytearray()
    for y in range(h):
        rows.append(0)
        row_off = y * w * 4
        for x in range(w):
            o = row_off + x * 4
            r = premul_rgba[o]
            g = premul_rgba[o + 1]
            b = premul_rgba[o + 2]
            a = max(0.0, min(1.0, premul_rgba[o + 3]))
            inv = 1.0 - a
            cr = max(0.0, min(1.0, r + bg[0] * inv))
            cg = max(0.0, min(1.0, g + bg[1] * inv))
            cb = max(0.0, min(1.0, b + bg[2] * inv))
            rows.append(int(round(cr * 255)))
            rows.append(int(round(cg * 255)))
            rows.append(int(round(cb * 255)))
            rows.append(255)

    def chunk(tag, data):
        crc = zlib.crc32(tag + data) & 0xFFFFFFFF
        return struct.pack(">I", len(data)) + tag + data + struct.pack(">I", crc)

    ihdr = struct.pack(">IIBBBBB", w, h, 8, 6, 0, 0, 0)
    png = (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", ihdr)
        + chunk(b"IDAT", zlib.compress(bytes(rows), 6))
        + chunk(b"IEND", b"")
    )
    with open(path, "wb") as f:
        f.write(png)


# 5x7 crisp bitmap font for bold gradient test text "MOS VFX"
FONT_5X7 = {
    "M": ["10001", "11011", "10101", "10001", "10001", "10001", "10001"],
    "O": ["01110", "10001", "10001", "10001", "10001", "10001", "01110"],
    "S": ["01111", "10000", "10000", "01110", "00001", "00001", "11110"],
    " ": ["00000", "00000", "00000", "00000", "00000", "00000", "00000"],
    "V": ["10001", "10001", "10001", "10001", "10001", "01010", "00100"],
    "F": ["11111", "10000", "10000", "11110", "10000", "10000", "10000"],
    "X": ["10001", "10001", "01010", "00100", "01010", "10001", "10001"],
}


def make_text_image(w=640, h=300, text="MOS VFX"):
    """Create a rich colorful gradient text image on transparent background."""
    px = [0.0] * (w * h * 4)
    scale = 12
    char_w = 5 * scale
    char_h = 7 * scale
    gap = 14
    total_w = len(text) * char_w + (len(text) - 1) * gap
    ox = (w - total_w) // 2 - 40  # slightly left so wind blowing right has plenty of room
    oy = (h - char_h) // 2 + 10

    for ci, ch in enumerate(text):
        glyph = FONT_5X7[ch]
        gx0 = ox + ci * (char_w + gap)
        for ry, row in enumerate(glyph):
            for rx, bit in enumerate(row):
                if bit != "1":
                    continue
                x0 = gx0 + rx * scale
                y0 = oy + ry * scale
                for py in range(max(0, y0), min(h, y0 + scale)):
                    for px_i in range(max(0, x0), min(w, x0 + scale)):
                        # Horizontal rainbow-ish cyan -> violet -> coral -> gold gradient
                        t = max(0.0, min(1.0, (px_i - ox) / max(1, total_w)))
                        v = max(0.0, min(1.0, (py - oy) / max(1, char_h)))
                        r = 0.25 + 0.75 * t
                        g = 0.85 * (1.0 - 0.55 * t) + 0.15 * (1.0 - v)
                        b = 1.0 - 0.75 * t
                        o = (py * w + px_i) * 4
                        px[o] = r
                        px[o + 1] = g
                        px[o + 2] = b
                        px[o + 3] = 1.0
    return px


def make_radial_mask(w=640, h=300):
    """Radial gradient custom mask: dark in center (dissolves first), bright at edges (dissolves last)."""
    px = [0.0] * (w * h * 4)
    cx = w * 0.44
    cy = h * 0.52
    max_r = math.hypot(w * 0.45, h * 0.45)
    for y in range(h):
        for x in range(w):
            d = math.hypot(x - cx, y - cy) / max_r
            val = max(0.0, min(1.0, d))
            o = (y * w + x) * 4
            px[o] = val
            px[o + 1] = val
            px[o + 2] = val
            px[o + 3] = 1.0
    return px


def render_case(helper, fh, w, h, src_px, params, mask_px=None, time_s=1.0):
    offset_main = 0
    offset_mask = w * h * 4 * 4
    write_frame(fh, offset_main, src_px)
    if mask_px is not None:
        write_frame(fh, offset_mask, mask_px)

    t0 = time.perf_counter()
    helper.send({
        "op": "render",
        "effect": EFFECT,
        "instance": 1,
        "time": time_s,
        "frame": {"offset": offset_main, "width": w, "height": h, "origin": [0.0, 0.0], "scale": 1.0},
        "layer_size": [float(w), float(h)],
        "flat": params,
    })

    while True:
        msg = helper.recv(timeout=20.0)
        op = msg.get("op")
        if op == "layer_input":
            assert msg["param"] == "mask_layer", f"unexpected param {msg}"
            if mask_px is not None:
                helper.send({
                    "op": "image",
                    "image": {"offset": offset_mask, "width": w, "height": h, "origin": [0.0, 0.0], "scale": 1.0},
                })
            else:
                helper.send({"op": "image", "image": None})
        elif op == "done":
            break
        elif op == "error":
            raise RuntimeError(f"helper error: {msg}")
        else:
            raise RuntimeError(f"unexpected message: {msg}")

    dt_ms = (time.perf_counter() - t0) * 1000.0
    out = read_frame(fh, offset_main, w * h * 4)
    return out, dt_ms


def max_alpha(buf):
    return max(buf[i] for i in range(3, len(buf), 4))


def max_diff(a, b):
    return max(abs(x - y) for x, y in zip(a, b))


def main():
    exe = sys.argv[1] if len(sys.argv) > 1 else os.path.join(HERE, "dist", "effectcraft-helper-mos-dissolve.exe")
    assert os.path.exists(exe), f"helper exe not found: {exe}"
    os.makedirs(OUT_DIR, exist_ok=True)

    w, h = 640, 300
    src_px = make_text_image(w, h)
    mask_px = make_radial_mask(w, h)
    save_png(os.path.join(OUT_DIR, "00-source.png"), w, h, src_px)
    save_png(os.path.join(OUT_DIR, "00-custom-mask.png"), w, h, mask_px)

    with tempfile.NamedTemporaryFile(prefix="mos-dissolve-test-", suffix=".bin", delete=False) as tf:
        frames_path = tf.name
        tf.truncate(w * h * 4 * 4 * 2)

    try:
        # 1. Check bilingual manifest (EN)
        h_en = Helper(exe, env_extra={"MOS_LANG": "en"})
        h_en.send({"op": "hello", "version": PROTO, "frames": frames_path})
        r_hello = h_en.recv()
        assert r_hello["op"] == "hello" and r_hello["name"] == "mos_dissolve", r_hello
        h_en.send({"op": "describe"})
        r_desc = h_en.recv()
        eff_en = r_desc["effects"][0]
        assert eff_en["padding"] is True
        assert eff_en["manifest"]["id"] == EFFECT
        assert eff_en["manifest"]["category"] == "MOS VFX"
        assert eff_en["manifest"]["name"] == "mos_dissolve"
        h_en.close()
        print("[OK] EN bilingual manifest verified")

        # 2. Main ZH session
        helper = Helper(exe, env_extra={"MOS_LANG": "zh"})
        helper.send({"op": "hello", "version": PROTO, "frames": frames_path})
        r_hello = helper.recv()
        assert r_hello["op"] == "hello" and r_hello["name"] == "mos_消散", r_hello
        helper.send({"op": "describe"})
        r_desc = helper.recv()
        eff_zh = r_desc["effects"][0]
        assert eff_zh["manifest"]["category"] == "MOS 视效"
        assert eff_zh["manifest"]["name"] == "mos_消散"
        print(f"[OK] ZH manifest verified ({len(eff_zh['manifest']['params'])} params)")

        # 3. Check Padding
        helper.send({
            "op": "padding",
            "effect": EFFECT,
            "instance": 1,
            "time": 0.0,
            "scale": 1.0,
            "layer_size": [float(w), float(h)],
            "flat": make_params(wind_strength=200.0),
        })
        r_pad = helper.recv()
        assert r_pad["op"] == "padding" and r_pad["pixels"] > 100, r_pad
        print(f"[OK] Padding verified: {r_pad['pixels']} px")

        with open(frames_path, "r+b") as fh:
            cases = [
                ("01-particle-p0.png", make_params(mode=0, progress=0.0), None),
                ("02-particle-linear-p45.png", make_params(mode=0, progress=45.0, mask_type=0), None),
                ("03-particle-noise-p50.png", make_params(
                    mode=0, progress=50.0, mask_type=1, noise_scale=55.0,
                    color_fade_enable=1.0, end_color=[1.0, 0.45, 0.1, 1.0],
                    color_fade_mix=85.0, additive_glow=45.0
                ), None),
                ("04-particle-custom-p50.png", make_params(
                    mode=0, progress=50.0, mask_type=2, mask_layer=2, mask_channel=0
                ), mask_px),
                ("05-particle-streak-p45.png", make_params(
                    mode=0, progress=45.0, mask_type=0, particle_shape=3,
                    wind_strength=240.0, turbulence_amp=85.0, particle_size=2.6
                ), None),
                ("06-particle-p100.png", make_params(mode=0, progress=100.0, mask_type=0), None),
                ("07-pixel-sep-linear-p45.png", make_params(
                    mode=1, progress=45.0, mask_type=0, pixel_block=1,
                    wind_strength=170.0, turbulence_amp=55.0
                ), None),
                ("08-pixel-sep-block4-p45.png", make_params(
                    mode=1, progress=45.0, mask_type=1, noise_scale=60.0, pixel_block=4,
                    wind_strength=150.0, scatter=60.0
                ), None),
                ("09-pixel-sep-custom-p50.png", make_params(
                    mode=1, progress=50.0, mask_type=2, mask_layer=2, pixel_block=2
                ), mask_px),
                ("10-pixel-sep-p100.png", make_params(mode=1, progress=100.0, mask_type=0, pixel_block=1), None),
            ]

            for fname, params, mpx in cases:
                out, ms = render_case(helper, fh, w, h, src_px, params, mask_px=mpx)
                save_png(os.path.join(OUT_DIR, fname), w, h, out)
                ma = max_alpha(out)
                if "p0" in fname:
                    d = max_diff(src_px, out)
                    assert d < 1e-5, f"{fname} differed from source by {d}"
                if "p100" in fname:
                    assert ma < 1e-4, f"{fname} did not dissolve completely (max_alpha={ma})"
                print(f"[OK] {fname:<30} max_alpha={ma:.3f}  time={ms:.1f} ms")

        helper.close()
        print("\nALL TESTS PASSED!")
    finally:
        try:
            os.remove(frames_path)
        except OSError:
            pass


if __name__ == "__main__":
    main()
