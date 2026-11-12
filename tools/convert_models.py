"""Turns the 1.7.10 Java mob models (decompiled `ModelBase` subclasses) into a Rust table of parts and
boxes, which the native mod builds Horizon model layers from. The animations (`setRotationAngles`)
are ported by hand; this carries the geometry exactly.

usage: convert_models.py <out.rs> <model.java>...
Each model is named `<mod>/<ModelClass>`, from the source folder it is in.
"""
import math
import re
import sys
from pathlib import Path

ADD_BOX = {"func_78789_a", "func_78790_a"}
F_NUM = r"-?\d+(?:\.\d+)?(?:[eE]-?\d+)?[Ff]?"


ENV = {}


def number(expr):
    e = expr.strip()
    # `this.getBodyHeight()`: the size a `setBodyHeight` gave.
    e = re.sub(r"\b(?:this\.)?get(\w+)\(\)", lambda m: m.group(1)[0].lower() + m.group(1)[1:], e)
    e = re.sub(r"\bthis\.(\w+)", r"\1", e)
    for k in sorted(ENV, key=len, reverse=True):
        e = re.sub(rf"\b{k}\b", f"({ENV[k]})", e)
    e = re.sub(r"\((?:float|double|int)\)\s*", "", e)
    e = re.sub(r"(\d(?:\.\d+)?(?:[eE]-?\d+)?)[FfDd]\b", r"\1", e)
    e = e.replace("Math.PI", "math.pi").replace("(float)", "")
    return float(eval(e, {"math": math, "__builtins__": {}}))


def split_args(s):
    out, depth, cur = [], 0, ""
    for ch in s:
        if ch == "(":
            depth += 1
        elif ch == ")":
            depth -= 1
        if ch == "," and depth == 0:
            out.append(cur)
            cur = ""
        else:
            cur += ch
    if cur.strip():
        out.append(cur)
    return [a.strip() for a in out]


def calls(text):
    """`.name(args)` calls in order, with balanced parentheses."""
    i = 0
    while True:
        m = re.compile(r"\.(\w+)\(").search(text, i)
        if not m:
            return
        depth, j = 1, m.end()
        while depth and j < len(text):
            depth += {"(": 1, ")": -1}.get(text[j], 0)
            j += 1
        yield m.group(1), text[m.end():j - 1]
        i = j


def method_body(src, name_pattern):
    m = re.search(name_pattern, src)
    if not m:
        return ""
    i = src.index("{", m.end()) + 1
    depth = 1
    j = i
    while depth and j < len(src):
        depth += {"{": 1, "}": -1}.get(src[j], 0)
        j += 1
    return src[i:j - 1]


def set_child(src, path):
    """`setChild()`: the class's own links, and its base class's when it calls `super.setChild()`."""
    body = method_body(src, r"public void setChild\(\)\s*")
    links = []
    if "super.setChild()" in body or not body:
        base = re.search(r"extends (\w+)", src)
        if base and (path.parent / f"{base.group(1)}.java").exists():
            bsrc = (path.parent / f"{base.group(1)}.java").read_text(encoding="utf-8", errors="replace")
            links.append(method_body(bsrc, r"public void setChild\(\)\s*"))
    links.append(body)
    return ";".join(re.findall(r"this\.\w+\.func_78792_a\(this\.\w+\)", "\n".join(links)))


def parse(path):
    src = path.read_text(encoding="utf-8", errors="replace")
    cls = path.stem
    ENV.clear()
    for m in re.finditer(r"^\s*(?:protected|public|private)?\s*(?:float|int|double)\s+(\w+)\s*=\s*([^;]+);", src, re.M):
        try:
            ENV[m.group(1)] = number(m.group(2))
        except Exception:
            pass
    ctors = [(m.group(1), method_body(src[m.start():], rf"public {cls}\([^)]*\)\s*")) for m in re.finditer(rf"public {cls}\(([^)]*)\)", src)]
    by_arity = {len([x for x in params.split(",") if x.strip()]): (params, body) for params, body in ctors}
    params, ctor = by_arity.get(0, ctors[0] if ctors else ("", ""))
    args = []
    for _ in range(4):
        m = re.match(r"\s*this\(([^;]*)\);", ctor)
        if not m:
            break
        args = [number(x) for x in split_args(m.group(1))]
        if len(args) not in by_arity:
            break
        params, ctor = by_arity[len(args)]
        for name, value in zip([x.split()[-1] for x in params.split(",") if x.strip()], args):
            ENV[name] = value
    sizes = class_method(src, path, "setTouhouModelData")
    for m in re.finditer(r"set(\w+)\(([^)]*)\)", sizes):
        try:
            ENV[m.group(1)[0].lower() + m.group(1)[1:]] = number(m.group(2))
        except Exception:
            pass
    if "setChild()" in ctor:
        ctor += ";" + set_child(src, path)
    # A `ModelTouhouDefault`: its parts come from `getTechneModelData`, linked by `defaultAddChildSetting`.
    techne = method_body(src, r"protected void getTechneModelData\(\)\s*")
    if techne:
        links = method_body(src, r"public void defaultAddChildSetting\(\)\s*")
        base = re.search(r"extends (\w+)", src)
        if (not links or "super.defaultAddChildSetting" in links) and base and (path.parent / f"{base.group(1)}.java").exists():
            bsrc = (path.parent / f"{base.group(1)}.java").read_text(encoding="utf-8", errors="replace")
            links = method_body(bsrc, r"public void defaultAddChildSetting\(\)\s*") + ";" + links
        ctor = "this.field_78090_t = 64;this.field_78089_u = 64;" + techne + ";" + links
    ctor = re.sub(r"if\s*\(\s*this\.\w+\s*!=\s*null\s*\)", ";", ctor).replace("{", ";").replace("}", ";")
    tex = [64, 32]
    if m := re.search(r"field_78090_t\s*=\s*([^;]+);", ctor):
        tex[0] = int(number(m.group(1)))
    if m := re.search(r"field_78089_u\s*=\s*([^;]+);", ctor):
        tex[1] = int(number(m.group(1)))
    parts, order, parent = {}, [], {}

    def part(name):
        if name not in parts:
            parts[name] = {"tex": [0, 0], "mirror": False, "cubes": [], "pivot": [0.0, 0.0, 0.0], "rot": [0.0, 0.0, 0.0]}
            order.append(name)
        return parts[name]

    for stmt in ctor.split(";"):
        s = " ".join(stmt.split())
        if not s:
            continue
        if m := re.match(r"(?:this\.)?(\w+)\s*=\s*new ModelRenderer\(this(?:\s*,\s*(\d+)\s*,\s*(\d+))?(?:\s*,\s*\"[^\"]*\")?\)(.*)", s):
            p = part(m.group(1))
            if m.group(2):
                p["tex"] = [int(m.group(2)), int(m.group(3))]
            target, rest = m.group(1), m.group(4)
        elif m := re.match(r"(?:ModelRenderer\s+)?(?:this\.)?(\w+)\s*=\s*new ModelRenderer\(this(?:\s*,\s*(\d+)\s*,\s*(\d+))?\)(.*)", s):
            p = part(m.group(1))
            if m.group(2):
                p["tex"] = [int(m.group(2)), int(m.group(3))]
            target, rest = m.group(1), m.group(4)
        elif m := re.match(r"(?:this\.)?(\w+)\.field_78809_i\s*=\s*(true|false)", s):
            part(m.group(1))["mirror"] = m.group(2) == "true"
            continue
        elif m := re.match(r"(?:this\.)?(\w+)\.field_(78795_f|78796_g|78808_h)\s*=\s*(.+)", s):
            axis = {"78795_f": 0, "78796_g": 1, "78808_h": 2}[m.group(2)]
            try:
                part(m.group(1))["rot"][axis] = number(m.group(3))
            except Exception:
                pass
            continue
        elif m := re.match(r"(?:this\.)?\w*[Rr]otat\w*\(\s*(?:this\.)?(\w+)\s*,(.*)\)$", s):
            args = split_args(m.group(2))
            if len(args) == 3:
                part(m.group(1))["rot"] = [number(a) for a in args]
            continue
        elif m := re.match(r"(?:this\.)?(\w+)(\..*)", s):
            target, rest = m.group(1), m.group(2)
            if target not in parts:
                continue
        else:
            continue
        p = part(target)
        for name, args in calls(rest):
            a = split_args(args)
            if name == "func_78784_a":
                p["tex"] = [int(number(a[0])), int(number(a[1]))]
            elif name in ADD_BOX:
                nums = [number(x) for x in a[:6]]
                grow = number(a[6]) if len(a) > 6 else 0.0
                p["cubes"].append({"tex": list(p["tex"]), "origin": nums[:3], "size": nums[3:6], "grow": grow, "mirror": p["mirror"]})
            elif name == "func_78793_a":
                p["pivot"] = [number(x) for x in a[:3]]
            elif name == "func_78792_a":
                child = re.sub(r"^this\.", "", a[0])
                # A link to a part the model never made was skipped as null in Java.
                if child in parts:
                    parent[child] = target

    apply_pose(src, path, parts)
    render = method_body(src, r"public void func_78088_a\([^)]*\)\s*")
    roots =[n for n in re.findall(r"(?:this\.)?(\w+)\.func_78785_a\(", render) if n in parts and n not in parent]
    if not roots:
        roots = [n for n in order if n not in parent]
    keep = []

    def walk(n):
        if n in keep:
            return
        keep.append(n)
        for c in order:
            if parent.get(c) == n:
                walk(c)

    for r in dict.fromkeys(roots):
        walk(r)
    return tex, [(n, parent.get(n), parts[n]) for n in keep]


POSE_FIELDS = {
    "field_78800_c": ("pivot", 0), "field_78797_d": ("pivot", 1), "field_78798_e": ("pivot", 2),
    "field_78795_f": ("rot", 0), "field_78796_g": ("rot", 1), "field_78808_h": ("rot", 2),
}


def skip_balanced(s, i, open_ch, close_ch):
    """The index just past the bracket that closes the one at `i`."""
    depth = 0
    while i < len(s):
        if s[i] == open_ch:
            depth += 1
        elif s[i] == close_ch:
            depth -= 1
            if depth == 0:
                return i + 1
        i += 1
    return i


def usual_statements(body):
    """A method's statements as they run in the usual case: those outside any `if`, and those in the
    `else` of one (not riding, not sneaking); loops and the `if` blocks themselves are left out."""
    out, cur, i = [], "", 0
    while i < len(body):
        word = re.match(r"(if|for|while|switch)\s*\(", body[i:])
        guard = re.match(r"if\s*\(\s*this\.\w+\s*!=\s*null\s*\)\s*\{", body[i:])
        if guard and (i == 0 or not (body[i - 1].isalnum() or body[i - 1] in "_.")):
            start = i + guard.end() - 1
            end = skip_balanced(body, start, "{", "}")
            out.extend(usual_statements(body[start + 1:end - 1]))
            i = end
            cur = ""
            continue
        if word and (i == 0 or not (body[i - 1].isalnum() or body[i - 1] in "_.")):
            i = skip_balanced(body, i + body[i:].index("("), "(", ")")
            while i < len(body) and body[i].isspace():
                i += 1
            i = skip_balanced(body, i, "{", "}") if body[i:i + 1] == "{" else body.index(";", i) + 1
            while True:
                j = i
                while j < len(body) and body[j].isspace():
                    j += 1
                if not body.startswith("else", j):
                    break
                j += 4
                while j < len(body) and body[j].isspace():
                    j += 1
                if re.match(r"if\s*\(", body[j:]):
                    # An `else if`: one more condition, left out with its block.
                    j = skip_balanced(body, j + body[j:].index("("), "(", ")")
                    while body[j].isspace():
                        j += 1
                    i = skip_balanced(body, j, "{", "}") if body[j] == "{" else body.index(";", j) + 1
                    continue
                if body[j] == "{":
                    end = skip_balanced(body, j, "{", "}")
                    out.extend(usual_statements(body[j + 1:end - 1]))
                    i = end
                else:
                    end = body.index(";", j) + 1
                    out.append(body[j:end - 1])
                    i = end
                break
            cur = ""
            continue
        if body[i] == "{":
            i = skip_balanced(body, i, "{", "}")
            cur = ""
            continue
        if body[i] == ";":
            out.append(cur)
            cur = ""
        else:
            cur += body[i]
        i += 1
    return out


def class_method(src, path, name):
    """The body of method `name` in this class, or else in the class it extends."""
    body = method_body(src, rf"(?:public|protected|private) void {name}\([^)]*\)\s*")
    if body:
        return body
    base = re.search(r"extends (\w+)", src)
    if base and (path.parent / f"{base.group(1)}.java").exists():
        bsrc = (path.parent / f"{base.group(1)}.java").read_text(encoding="utf-8", errors="replace")
        return method_body(bsrc, rf"(?:public|protected|private) void {name}\([^)]*\)\s*")
    return ""


def apply_pose(src, path, parts, body=None, depth=0):
    """The pose `setRotationAngles` (`func_78087_a`) gives in the usual case, where it is a constant:
    many models place their head, arms, legs and skirt there each frame, not in the constructor."""
    if body is None:
        body = class_method(src, path, "func_78087_a")
    for stmt in usual_statements(body):
        s = " ".join(stmt.split())
        up = re.match(r"super\.(\w+)\(.*\)$", s)
        if up and depth < 4:
            base = re.search(r"extends (\w+)", src)
            if base and (path.parent / f"{base.group(1)}.java").exists():
                bsrc = (path.parent / f"{base.group(1)}.java").read_text(encoding="utf-8", errors="replace")
                inner = method_body(bsrc, rf"(?:public|protected|private) void {up.group(1)}\([^)]*\)\s*")
                if inner:
                    apply_pose(src, path, parts, inner, depth + 1)
            continue
        call = re.match(r"(?:this\.)?(\w+)\(.*\)$", s)
        if call and depth < 4 and call.group(1) not in ("super",):
            inner = class_method(src, path, call.group(1))
            if inner:
                apply_pose(src, path, parts, inner, depth + 1)
            continue
        point = re.fullmatch(r"(?:this\.)?(\w+)\.func_78793_a\((.*)\)", s)
        if point and point.group(1) in parts:
            try:
                parts[point.group(1)]["pivot"] = [number(x) for x in split_args(point.group(2))[:3]]
            except Exception:
                pass
            continue
        if re.search(r"[+\-*/]=", s):
            continue
        sides = [x.strip() for x in re.split(r"(?<![=!<>])=(?!=)", s)]
        if len(sides) < 2:
            continue
        expr = sides[-1]
        expr = re.sub(r"(?:this\.)?rad\(", "math.radians(", expr)

        def known(m):
            part, field = m.group(1), m.group(2)
            if part in parts and field in POSE_FIELDS:
                key, axis = POSE_FIELDS[field]
                return repr(parts[part][key][axis])
            return m.group(0)

        expr = re.sub(r"(?:this\.)?(\w+)\.(field_\w+)", known, expr)
        try:
            value = number(expr)
        except Exception:
            continue
        for target in sides[:-1]:
            m = re.fullmatch(r"(?:this\.)?(\w+)\.(field_\w+)", target)
            if m and m.group(1) in parts and m.group(2) in POSE_FIELDS:
                key, axis = POSE_FIELDS[m.group(2)]
                parts[m.group(1)][key][axis] = value


def f(x):
    s = repr(float(x))
    return s if "." in s or "e" in s else s + ".0"


def rust(models):
    out = ["//! Generated by `convert_models.py` from the original mods' Java models: do not edit.", "",
           "use crate::model::{Cube, Model, Part};", "", "pub const MODELS: &[Model] = &["]
    for mid, (tex, parts) in models:
        out.append(f"    Model {{ id: \"{mid}\", tex: [{tex[0]}, {tex[1]}], parts: &[")
        for name, par, p in parts:
            cubes = ", ".join(
                f"Cube {{ tex: [{c['tex'][0]}, {c['tex'][1]}], origin: [{', '.join(map(f, c['origin']))}], "
                f"size: [{', '.join(map(f, c['size']))}], grow: {f(c['grow'])}, mirror: {str(c['mirror']).lower()} }}"
                for c in p["cubes"])
            out.append(
                f"        Part {{ name: \"{name}\", parent: {('Some(\"' + par + '\")') if par else 'None'}, "
                f"pivot: [{', '.join(map(f, p['pivot']))}], rot: [{', '.join(map(f, p['rot']))}], cubes: &[{cubes}] }},")
        out.append("    ] },")
    out.append("];")
    return "\n".join(out) + "\n"


out = Path(sys.argv[1])
models = []
for arg in sys.argv[2:]:
    path = Path(arg)
    mod = path.parts[path.parts.index("src") + 1]
    tex, parts = parse(path)
    if parts:
        models.append((f"{mod}/{path.stem}", (tex, parts)))
        print(f"{mod}/{path.stem}: {len(parts)} parts, {sum(len(p['cubes']) for _, _, p in parts)} boxes, texture {tex}")
    else:
        print(f"{mod}/{path.stem}: no parts (a base class)")
out.write_text(rust(models), encoding="utf-8")
