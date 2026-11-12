"""Draws a converted model from the front, as an image, to check its layout without the game.

usage: preview.py <model id, e.g. kaguya/ModelWriggle> <texture png> <out png>
Faces are textured as vanilla unwraps a box; parts turn about their pivots (x, then y, then z).
"""
import math
import re
import sys
from pathlib import Path

sys.path.insert(0, "/home/mxpbx/horizon-dev")
import png  # noqa: E402

mid, tex_path, out = sys.argv[1], sys.argv[2], sys.argv[3]
src = Path("/home/mxpbx/horizon-dev/th-ports/mobs/native/api/src/models_gen.rs").read_text()
block = src[src.index(f'id: "{mid}"'):]
block = block[:block.index("] },\n    Model {") if "] },\n    Model {" in block else len(block)]
tex_w, tex_h = map(int, re.search(r"tex: \[(\d+), (\d+)\]", block).groups())
parts = {}
order = []
for m in re.finditer(r'Part \{ name: "(\w+)", parent: (None|Some\("(\w+)"\)), pivot: \[([^\]]*)\], rot: \[([^\]]*)\], cubes: &\[(.*?)\] \}', block):
    cubes = [tuple(map(float, re.findall(r"-?\d+\.?\d*(?:e-?\d+)?", c))) for c in re.findall(r"Cube \{(.*?)\}", m.group(6))]
    parts[m.group(1)] = (m.group(3), list(map(float, m.group(4).split(","))), list(map(float, m.group(5).split(","))), cubes)
    order.append(m.group(1))

tw, th, texel = png.read(tex_path)
sx, sy = tw / tex_w, th / tex_h


def matmul(a, b):
    return [[sum(a[i][k] * b[k][j] for k in range(4)) for j in range(4)] for i in range(4)]


def transform(name):
    parent, pivot, rot, _ = parts[name]
    m = [[1, 0, 0, pivot[0]], [0, 1, 0, pivot[1]], [0, 0, 1, pivot[2]], [0, 0, 0, 1]]
    for axis, a in zip("zyx", reversed(rot)):
        c, s = math.cos(a), math.sin(a)
        r = {"x": [[1, 0, 0, 0], [0, c, -s, 0], [0, s, c, 0], [0, 0, 0, 1]],
             "y": [[c, 0, s, 0], [0, 1, 0, 0], [-s, 0, c, 0], [0, 0, 0, 1]],
             "z": [[c, -s, 0, 0], [s, c, 0, 0], [0, 0, 1, 0], [0, 0, 0, 1]]}[axis]
        m = matmul(m, r)
    return matmul(transform(parent), m) if parent else m


W, H, S = 64, 64, 6  # model units shown, pixels each
img = [[(40, 40, 60, 255)] * (W * S) for _ in range(H * S)]
depth = [[1e9] * (W * S) for _ in range(H * S)]
for name in order:
    m = transform(name)
    for c in parts[name][3]:
        u, v, ox, oy, oz, w, h, d = c[:8]
        # The front face (towards -z): texels (u+d, v+d) to (u+d+w, v+d+h).
        for py in range(int(h * S)):
            for px in range(int(w * S)):
                lx, ly = ox + (px + 0.5) / S, oy + (py + 0.5) / S
                x = m[0][0] * lx + m[0][1] * ly + m[0][2] * oz + m[0][3]
                y = m[1][0] * lx + m[1][1] * ly + m[1][2] * oz + m[1][3]
                z = m[2][0] * lx + m[2][1] * ly + m[2][2] * oz + m[2][3]
                X, Y = int((x + W / 2) * S), int((y + 8) * S)
                if not (0 <= X < W * S and 0 <= Y < H * S) or z >= depth[Y][X]:
                    continue
                tu = int((u + d + (px + 0.5) / S) * sx)
                tv = int((v + d + (py + 0.5) / S) * sy)
                if 0 <= tu < tw and 0 <= tv < th and texel[tv][tu][3] > 0:
                    depth[Y][X] = z
                    img[Y][X] = texel[tv][tu]
png.write(out, W * S, H * S, img)
print("wrote", out, len(order), "parts")
