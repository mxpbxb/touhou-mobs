"""Builds the content mod `thmobs` from the original mods' jar assets and `mobs.json`, which the
native mod's tests write from its table of mobs: each mob's living type (a wandering trader the
native mod gives its own goals and model), its textures and spawn egg, and the danmaku sheets.

usage: gen_content.py <unpacked jars folder> <content mod folder> <mobs.json>
"""
import json
import shutil
import sys
from pathlib import Path

jars, out, manifest = Path(sys.argv[1]), Path(sys.argv[2]), Path(sys.argv[3])
NS, VERSION = "thmobs", "0.6.0"
if out.exists():
    shutil.rmtree(out)
oa, od = out / "assets" / NS, out / "data" / NS

K = "kaguya/assets/thkaguyamod/textures"
J = "jtg/assets/journeytogensokyo/textures/entity/mob"
S = "spellcard/assets/spellcardplusmod/textures/mob"
# Each texture the native mod names, and where the original kept it.
TEXTURES = {
    "fairy_0": f"{K}/mob/FairyTexture_0.png", "fairy_1": f"{K}/mob/FairyTexture_1.png", "fairy_2": f"{K}/mob/FairyTexture_2.png",
    "fairy_10": f"{K}/mob/FairyTexture_10.png", "fairy_zombie": f"{K}/mob/ZombieFairyTexture.png",
    "cirno": f"{K}/mob/CirnoTexture.png", "rumia": f"{K}/mob/RumiaTexture.png", "toziko": f"{K}/mob/TozikoTexture.png",
    "wriggle": f"{K}/mob/WriggleTexture.png", "reimu": f"{K}/mob/ReimuTexture.png", "sakuya": f"{K}/mob/SakuyaTexture.png",
    "sanae": f"{K}/mob/SanaeTexture.png", "miko": f"{K}/mob/MikoTexture.png", "rinnosuke": f"{K}/mob/RinnosukeTexture.png",
    "familiar": f"{K}/mob/Familiar.png",
    "wakasagihime": f"{S}/WakasagihimeTexture.png", "hina": f"{S}/HineTexture.png",
    "decoy": f"{K}/mob/DecoyTexture.png", "yin_yang_orb": f"{S}/Yin_yang_orb_Yukari.png",
    "fairy_ice": f"{J}/FairyTextureIce.png", "fairy_end_0": f"{J}/FairyTextureEnd_0.png", "fairy_end_1": f"{J}/FairyTextureEnd_1.png",
    "fairy_end_2": f"{J}/FairyTextureEnd_2.png", "fairy_sunflower_end": f"{J}/FairyTextureSunflowerEnd.png",
    "tengu_crow": f"{J}/tenguCrow.png", "hell_raven": f"{J}/hellRaven.png", "marisa": f"{J}/marisa.png", "yukari": f"{J}/yukari.png",
}
SHEETS = {
    "small": "SmallShot.png", "pearl": "PearlShot.png", "circle": "CircleShot.png", "heart": "HeartShot.png",
    "talisman": "TalismanShot.png", "big": "BigShot.png", "light": "MusouFuuin.png",
}


def write(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")


def copy(a, b):
    b.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(a, b)


for name, file in SHEETS.items():
    copy(jars / K / "shot" / file, oa / "textures" / "entity" / "shot" / f"{name}.png")
copy(jars / K / "items" / "spawn_egg.png", oa / "textures" / "item" / "spawn_egg.png")
copy(jars / K / "items" / "spawn_egg_overlay.png", oa / "textures" / "item" / "spawn_egg_overlay.png")

mobs = json.loads(manifest.read_text(encoding="utf-8"))
used = set()
lang = {f"itemGroup.{NS}.mobs": "Touhou Mobs"}
eggs = []
for m in mobs:
    mid = m["id"]
    write(od / "horizon" / "entity_type" / f"{mid}.json", {
        "template": m["template"],
        "attributes": {"minecraft:max_health": float(m["max_health"]), "minecraft:movement_speed": float(m["speed"])},
    })
    look = m["look"]
    for tex in look.get("textures", []) + ([look["glow"]] if look.get("glow") not in (None, "light") else []):
        used.add(tex)
    egg = f"{mid}_spawn_egg"
    eggs.append(f"{NS}:{egg}")
    write(od / "horizon" / "item" / f"{egg}.json", {
        "template": "minecraft:paper",
        "components": {"minecraft:item_name": {"translate": f"item.{NS}.{egg}"}},
    })
    base, spots = m["egg"]
    write(oa / "items" / f"{egg}.json", {"model": {
        "type": "minecraft:model", "model": f"{NS}:item/spawn_egg",
        "tints": [{"type": "minecraft:constant", "value": base}, {"type": "minecraft:constant", "value": spots}],
    }})
    lang[f"entity.{NS}.{mid}"] = m["name"]
    lang[f"item.{NS}.{egg}"] = f"{m['name']} Spawn Egg"
missing = sorted(t for t in used if t not in TEXTURES)
assert not missing, f"no source for textures {missing}"
for tex in sorted(used):
    copy(jars / TEXTURES[tex], oa / "textures" / "entity" / f"{tex}.png")
write(oa / "models" / "item" / "spawn_egg.json", {
    "parent": "minecraft:item/generated",
    "textures": {"layer0": f"{NS}:item/spawn_egg", "layer1": f"{NS}:item/spawn_egg_overlay"},
})
write(oa / "lang" / "en_us.json", lang)
write(od / "horizon" / "creative_tab" / "mobs.json", {"title": "Touhou Mobs", "icon": eggs[0], "items": eggs})
(out / "horizon-mod.toml").write_text(f'''id = "{NS}"
name = "Touhou Mobs (assets)"
version = "{VERSION}"
description = "The mobs, textures and spawn eggs of Touhou Mobs: the fairies, youkai, bosses and people of Touhou Items Mod, SPELLCARD PLUS and Journey To Gensokyo. Needs thmobs_native."
authors = ["kuro-anco, Katrix and the SPELLCARD PLUS author (originals)", "mxpbxb (Horizon port)"]
api = "0.2"
side = "both"
capabilities = ["registries"]
''', encoding="utf-8")
print("built", out, len(mobs), "mobs,", len(used), "textures")
