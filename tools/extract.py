"""Prints the parts of a mob class a port needs: constructor, phases, cards and attacks, compactly."""
import re, sys
from pathlib import Path
def body(src, pat):
    m = re.search(pat, src)
    if not m: return ""
    i = src.index("{", m.end()) + 1; d = 1; j = i
    while d and j < len(src):
        d += {"{": 1, "}": -1}.get(src[j], 0); j += 1
    return src[i:j-1]
def squash(s):
    s = re.sub(r"\n\s*", " ", s)
    s = re.sub(r"\s+", " ", s)
    s = s.replace("this.field_70146_Z", "rand").replace("this.field_70177_z", "yaw").replace("this.field_70125_A", "pitch")
    s = s.replace("this.attackCounter", "C").replace("THShotLib.", "").replace("this.", "").replace("ShotData.shot", "shot")
    s = s.replace("func_70040_Z()", "look()").replace("field_70170_p.field_72995_K", "isClient").replace("func_76126_a", "sin").replace("func_76134_b", "cos")
    s = s.replace("field_70789_a", "target").replace("field_70165_t", "x").replace("field_70163_u", "y").replace("field_70161_v", "z")
    s = s.replace("nextFloat()", "nf()").replace("nextInt", "ni")
    return s
for f in sys.argv[1:]:
    src = Path(f).read_text(errors="replace"); cls = Path(f).stem
    print(f"===== {cls}")
    print("CTOR:", squash(body(src, rf"public {cls}\(World world\)")))
    print("CARD:", squash(body(src, r"public int getUsingSpellCardNo\(\)")))
    print("DEATH:", squash(body(src, r"protected void func_70609_aI\(\)")))
    print("ATTACK:", squash(body(src, r"void danmakuPattern\(int level\)")))
    extra = body(src, r"public void func_70071_h_\(\)")
    if extra: print("TICK:", squash(extra))
    for m in re.finditer(r"(?:private|protected|public) void ((?:danmaku|spellcard|attack|spell|normal)\w*)\(([^)]*)\)", src):
        if m.group(1) == "danmakuPattern": continue
        print(f"{m.group(1).upper()}({m.group(2)}):", squash(body(src[m.start():], r"\)\s*")))
