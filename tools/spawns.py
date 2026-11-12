"""Prints SPAWNS.md: where, how and how often each Touhou Mobs mob appears, read from `mobs.rs`.

usage: spawns.py <th-ports folder>
"""
import re
import sys
from pathlib import Path

root = Path(sys.argv[1])
src = (root / "mobs/native/api/src/mobs.rs").read_text()
lists = {m.group(1): re.findall(r'"([\w:]+)"', m.group(2)) for m in re.finditer(r"const (\w+): &\[&str\] = &\[(.*?)\];", src, re.S)}
LIGHT = {
    "Fairy": "in daylight, under open sky, on grass, dirt or sand; half the tries fail",
    "Dark": "in the dark, as a monster",
    "Any": "any light",
    "Rare": "any light, but only 2 tries in 100 succeed",
}
rows, eggs = [], []
for block in re.split(r"\n    Mob \{", src)[1:]:
    mid = re.search(r'id: "(\w+)"', block).group(1)
    name = re.search(r'name: "([^"]+)"', block).group(1)
    m = re.search(r"spawn: spawn\((\d+), (\d+), (\d+), ([A-Z_]+|&\[[^\]]*\]), Light::(\w+)\)", block)
    if not m:
        eggs.append(name)
        continue
    weight, lo, hi, biomes, light = int(m.group(1)), m.group(2), m.group(3), m.group(4), m.group(5)
    biomes = lists[biomes] if biomes in lists else re.findall(r'"([\w:]+)"', biomes)
    rows.append((name, weight, lo, hi, biomes, light))
total = sum(r[1] for r in rows)
out = ["# Touhou Mobs: where they spawn", "",
       "About once every 0.8 seconds for each player, Touhou Mobs tries to spawn one group, 24 to 48 blocks away, of a mob "
       "picked by weight below. The spot must be in one of the mob's biomes and pass its light rule. At most 12 of "
       "these mobs can be within 64 blocks of a player. On peaceful none spawn. They disappear when no player is "
       "within 128 blocks, and may wander off beyond 32, as monsters do.", "",
       "| Mob | Weight | Share of picks | Group | Light | Biomes |", "|---|---|---|---|---|---|"]
for name, weight, lo, hi, biomes, light in sorted(rows, key=lambda r: -r[1]):
    group = lo if lo == hi else f"{lo}-{hi}"
    out.append(f"| {name} | {weight} | {weight / total:.1%} | {group} | {LIGHT[light]} | {', '.join(b.replace('biomesoplenty:', 'BoP ').replace('_', ' ') for b in biomes)} |")
out += ["", "Only from spawn eggs (in the Touhou Mobs creative tab): " + ", ".join(eggs) + ".", ""]
(root / "SPAWNS.md").write_text("\n".join(out), encoding="utf-8")
print("\n".join(out))
