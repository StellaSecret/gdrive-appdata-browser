"""
Faithful re-implementation of icon-generator.html's canvas drawing routine,
translated call-by-call to Pillow + numpy since no headless browser or
node-canvas was available in this environment to execute the original JS.
"""

import numpy as np
from PIL import Image, ImageDraw, ImageFont, ImageFilter

W, H = 512, 512


def hex_to_rgb(h):
    h = h.lstrip("#")
    return tuple(int(h[i : i + 2], 16) for i in (0, 2, 4))


def lerp_color(c0, c1, t):
    return tuple(c0[i] + (c1[i] - c0[i]) * t for i in range(3))


def rounded_rect_mask(size, radius):
    mask = Image.new("L", size, 0)
    d = ImageDraw.Draw(mask)
    d.rounded_rectangle([0, 0, size[0] - 1, size[1] - 1], radius=radius, fill=255)
    return mask


def diagonal_gradient(size, c0_hex, c1_hex):
    """Mirrors ctx.createLinearGradient(0, 0, W, H)."""
    w, h = size
    c0 = np.array(hex_to_rgb(c0_hex), dtype=np.float64)
    c1 = np.array(hex_to_rgb(c1_hex), dtype=np.float64)
    xs = np.linspace(0, 1, w)
    ys = np.linspace(0, 1, h)
    t = (xs[None, :] + ys[:, None]) / 2.0
    t = np.clip(t, 0, 1)
    arr = c0[None, None, :] + (c1 - c0)[None, None, :] * t[:, :, None]
    return Image.fromarray(arr.astype(np.uint8), mode="RGB")


def radial_gradient_rgba(size, cx, cy, r0, r1, color_rgb, a0, a1):
    """Mirrors ctx.createRadialGradient(cx, cy, r0, cx, cy, r1) with an
    rgba color stop at 0 and a transparent stop at 1."""
    w, h = size
    yy, xx = np.mgrid[0:h, 0:w]
    dist = np.sqrt((xx - cx) ** 2 + (yy - cy) ** 2)
    t = np.clip((dist - r0) / max(r1 - r0, 1e-6), 0, 1)
    alpha = (a0 + (a1 - a0) * t) * 255
    alpha = np.clip(alpha, 0, 255)
    out = np.zeros((h, w, 4), dtype=np.uint8)
    out[:, :, 0] = color_rgb[0]
    out[:, :, 1] = color_rgb[1]
    out[:, :, 2] = color_rgb[2]
    out[:, :, 3] = alpha.astype(np.uint8)
    return Image.fromarray(out, mode="RGBA")


def quadratic_bezier_points(p0, p1, p2, steps=20):
    pts = []
    for i in range(steps + 1):
        t = i / steps
        x = (1 - t) ** 2 * p0[0] + 2 * (1 - t) * t * p1[0] + t**2 * p2[0]
        y = (1 - t) ** 2 * p0[1] + 2 * (1 - t) * t * p1[1] + t**2 * p2[1]
        pts.append((x, y))
    return pts


def build_curve_path(pts):
    """Mirrors the moveTo + repeated quadraticCurveTo-to-midpoint pattern
    used for both the DEP curve and its fill outline in the original JS."""
    path = [pts[0]]
    for i in range(len(pts) - 1):
        mx = (pts[i][0] + pts[i + 1][0]) / 2
        my = (pts[i][1] + pts[i + 1][1]) / 2
        path.extend(quadratic_bezier_points(path[-1], pts[i], (mx, my))[1:])
    path.append(pts[-1])
    return path


def draw_dashed_line(draw, points, dash=6, gap=8, **kwargs):
    for i in range(len(points) - 1):
        x0, y0 = points[i]
        x1, y1 = points[i + 1]
        seg_len = ((x1 - x0) ** 2 + (y1 - y0) ** 2) ** 0.5
        if seg_len == 0:
            continue
        dx, dy = (x1 - x0) / seg_len, (y1 - y0) / seg_len
        pos = 0.0
        draw_on = True
        while pos < seg_len:
            step = dash if draw_on else gap
            end = min(pos + step, seg_len)
            if draw_on:
                draw.line(
                    [(x0 + dx * pos, y0 + dy * pos), (x0 + dx * end, y0 + dy * end)],
                    **kwargs,
                )
            pos = end
            draw_on = not draw_on


def main():
    canvas = Image.new("RGBA", (W, H), (0, 0, 0, 0))

    # 1. Background: rounded rect, diagonal gradient #0d1b2a -> #0a1628
    bg = diagonal_gradient((W, H), "0d1b2a", "0a1628").convert("RGBA")
    bg_mask = rounded_rect_mask((W, H), 80)
    canvas.paste(bg, (0, 0), bg_mask)

    # 2. Halo behind the curve, radial gradient centered (256, 220)
    halo = radial_gradient_rgba((W, H), 256, 220, 20, 240, (79, 156, 249), 0.15, 0.0)
    canvas = Image.alpha_composite(canvas, halo)

    # 3. DEP curve points (same control points as the original)
    pts = [
        (80, 310),
        (120, 300),
        (160, 240),
        (195, 145),  # peak
        (225, 175),
        (255, 210),
        (290, 230),
        (330, 248),
        (380, 258),
        (430, 262),
    ]
    curve_path = build_curve_path(pts)

    # Fill under the curve: linear gradient rgba(79,156,249,.35 -> .02),
    # vertical from y=140 to y=330, clipped to the area under the curve.
    fill_poly = curve_path + [(430, 330), (80, 330)]
    fill_mask = Image.new("L", (W, H), 0)
    ImageDraw.Draw(fill_mask).polygon(fill_poly, fill=255)

    grad_col = np.array([79, 156, 249], dtype=np.uint8)
    fill_layer = np.zeros((H, W, 4), dtype=np.uint8)
    fill_layer[:, :, 0] = grad_col[0]
    fill_layer[:, :, 1] = grad_col[1]
    fill_layer[:, :, 2] = grad_col[2]
    ys = np.clip((np.arange(H) - 140) / (330 - 140), 0, 1)
    alpha_row = ((0.35 + (0.02 - 0.35) * ys) * 255).astype(np.uint8)
    fill_layer[:, :, 3] = alpha_row[:, None]
    fill_img = Image.fromarray(fill_layer, mode="RGBA")
    canvas = Image.alpha_composite(canvas, Image.composite(fill_img, Image.new("RGBA", (W, H), (0, 0, 0, 0)), fill_mask))

    # 4. Curve stroke, solid #4f9cf9, width 5, rounded joins/caps
    stroke_layer = Image.new("RGBA", (W, H), (0, 0, 0, 0))
    sd = ImageDraw.Draw(stroke_layer)
    sd.line(curve_path, fill=(79, 156, 249, 255), width=5, joint="curve")
    r = 2  # round caps/joins approximation
    for p in (curve_path[0], curve_path[-1]):
        sd.ellipse([p[0] - r, p[1] - r, p[0] + r, p[1] + r], fill=(79, 156, 249, 255))
    canvas = Image.alpha_composite(canvas, stroke_layer)

    # 5. Peak point with halo + white ring
    peak_x, peak_y = 195, 145
    peak_halo = radial_gradient_rgba((W, H), peak_x, peak_y, 2, 28, (16, 217, 160), 0.5, 0.0)
    canvas = Image.alpha_composite(canvas, peak_halo)

    dot_layer = Image.new("RGBA", (W, H), (0, 0, 0, 0))
    dd = ImageDraw.Draw(dot_layer)
    dd.ellipse([peak_x - 8, peak_y - 8, peak_x + 8, peak_y + 8], fill=(16, 217, 160, 255))
    dd.ellipse(
        [peak_x - 8, peak_y - 8, peak_x + 8, peak_y + 8],
        outline=(255, 255, 255, 255),
        width=3,
    )
    canvas = Image.alpha_composite(canvas, dot_layer)

    # 6. SpO2 dashed line
    spo2_pts = [(80, 285), (140, 278), (200, 272), (260, 268), (320, 266), (380, 265), (430, 265)]
    spo2_layer = Image.new("RGBA", (W, H), (0, 0, 0, 0))
    spo2_draw = ImageDraw.Draw(spo2_layer)
    draw_dashed_line(spo2_draw, spo2_pts, dash=6, gap=8, fill=(16, 217, 160, 128), width=3)
    canvas = Image.alpha_composite(canvas, spo2_layer)

    # 7. "AT" monogram, gradient text (blue -> green), bold
    font = ImageFont.truetype("/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf", 150)
    text = "AT"
    text_mask = Image.new("L", (W, H), 0)
    tdraw = ImageDraw.Draw(text_mask)
    bbox = tdraw.textbbox((0, 0), text, font=font)
    tw, th = bbox[2] - bbox[0], bbox[3] - bbox[1]
    tx, ty = 256 - tw / 2 - bbox[0], 418 - th / 2 - bbox[1]
    tdraw.text((tx, ty), text, font=font, fill=255)

    text_grad = diagonal_gradient((W, H), "4f9cf9", "10d9a0").convert("RGBA")
    text_grad.putalpha(text_mask)
    canvas = Image.alpha_composite(canvas, text_grad)

    # 8. Decorative underline, horizontal gradient fading at both ends
    line_layer = np.zeros((H, W, 4), dtype=np.uint8)
    xs = np.arange(W)
    t = np.clip((xs - 176) / (336 - 176), 0, 1)
    alpha_line = np.where(t < 0.5, t / 0.5, (1 - t) / 0.5)
    alpha_line = np.clip(alpha_line, 0, 1) * 255
    line_layer[:, :, 0] = 79
    line_layer[:, :, 1] = 156
    line_layer[:, :, 2] = 249
    line_mask = np.zeros((H, W), dtype=np.uint8)
    line_mask[447:450, 176:337] = 1
    line_layer[:, :, 3] = (alpha_line[None, :] * line_mask).astype(np.uint8)
    canvas = Image.alpha_composite(canvas, Image.fromarray(line_layer, mode="RGBA"))

    # 9. Decorative dots
    dots_layer = Image.new("RGBA", (W, H), (0, 0, 0, 0))
    ddraw = ImageDraw.Draw(dots_layer)
    for x, y in [(440, 80), (60, 180), (470, 380), (50, 420)]:
        ddraw.ellipse([x - 3, y - 3, x + 3, y + 3], fill=(79, 156, 249, 38))
    canvas = Image.alpha_composite(canvas, dots_layer)

    # Final: clip to rounded-rect silhouette (matches the background shape;
    # also means clean edges once CSS crops this into a circular button)
    final_mask = rounded_rect_mask((W, H), 80)
    r, g, b, a = canvas.split()
    a = Image.composite(a, Image.new("L", (W, H), 0), final_mask)
    canvas = Image.merge("RGBA", (r, g, b, a))

    canvas.save("/home/claude/icon-render/asthmatrack-512.png")
    print("saved")


if __name__ == "__main__":
    main()
