# Touhou Mobs

The fairies, youkai, bosses and people of three 1.7.10 Touhou mods, brought to
[Horizon](https://playhorizon.dev) in one mod:

- **Touhou Items Mod** (Itutu no Nandai MOD+) by kuro-anco
- **SPELLCARD PLUS** (五つの難題MOD SPELLCARDPLUS)
- **Journey To Gensokyo** by Katrix

Ported with the permission of the original creators. Their art, models and danmaku patterns are
theirs; this repository holds the Horizon port's code and the tools that build it.

## Two parts

Install both from the release, then press Build in the launcher:

- **`thmobs_native`** (`.hnmod`): the code. A native mod: it runs with your account's permissions,
  so the launcher asks before building it.
- **`thmobs`** (`.hmod`): the mobs' types, textures, spawn eggs and the danmaku sheets.

## The mobs

All 30 living mobs of the three mods (Grimoire of Alice adds none), and one of this port's own:

- **Fairies and small youkai:** fairy, sunflower fairy (with its familiar), phantom, familiar,
  Hanabeeper, ice fairy, Nether fairy, moon fairy, moon sunflower fairy, crow tengu, hell raven, ice
  familiar.
- **Bosses**, which come back in their next phase when their health runs out, as in the originals:
  Cirno, Rumia, Soga no Tojiko, Wriggle Nightbug, Reimu Hakurei (both mods' versions), Wakasagihime,
  Hina Kagiyama, SPELLCARD PLUS's Cirno and Rumia, Marisa Kirisame, Yukari Yakumo.
- **People:** Sakuya, Sanae and Rinnosuke wander and look about (Rinnosuke trades); right-click Miko
  to set the danmaku level, easy to lunatic.
- **Summons:** the Cursed Decoy Doll (draws every hostile mob within 40 blocks for nine seconds) and
  Yukari's yin-yang orb (circles its player).
- **The Old Crow Tengu:** the crow tengu as it was before this port made it slower and rarer, at the
  original's speed and full rate of fire. A rare spawn, always alone, in the mountains.

The crow tengu and the hell raven fly with their original animation: the body bobs, and their
two-piece wings beat.

Each mob's attacks are its original `danmakuPattern` ported line for line, at 35% of the original
rate of fire. Its model is its Java model, converted part for part. Hostile mobs (but not creepers)
go for the Touhou mobs that are attacking, and a Touhou mob a vanilla mob hurts fights it back, its
danmaku aimed at that mob until it is dead or 40 blocks off.

## Classic bosses

Eleven named characters also come as a **Classic** version, beside the ones above, which are
untouched. A classic boss fights as the original mod had it: at the original's full rate of fire,
through every stage, and with every spell card it declared, lasers included. A classic boss has no
spawning of its own: once in 20,000 one comes in place of its counterpart, and each has its own
spawn egg.

- **Reimu Hakurei (Classic)**: Spirit Sign "Fantasy Seal" (homing orbs that grow).
- **Reimu Hakurei (Journey To Gensokyo, Classic)**: her amulets and yin-yang orbs, then Fantasy Seal.
- **Marisa Kirisame (Classic)**: three stages with star familiars; Stardust Reverie, Non-Directional
  Laser and Master Spark; then her final word, Loving Heart "Double Spark".
- **Yukari Yakumo (Classic)**: five kunai stages; Flying-glowworm Nest (gaps that fire lasers), Curse
  of Dreams and Reality, Double Black Death Butterfly, Mesh of Light and Darkness, and Boundary
  between Wave and Particle.
- **Yukari Great Doll**: classic Yukari at half her size, with a quarter of her health, whose
  danmaku flies at half its speed and does half its damage. When you come out of a gap made global with a Yukari umbrella (Yukari Gaps), the
  Yukari who sometimes follows is always her, and she always drops an umbrella. Yukari drops one
  70 times in 100.
- **Cirno (Classic)**: her second stage's fans of lasers; Icicle Fall and Perfect Freeze.
- **Rumia (Classic)**: Moonlight Ray (seven turning lasers).
- **Soga no Tojiko (Classic)**: Gagouji's Cyclone (zigzagging arrows).
- **Wriggle Nightbug (Classic)**: Meteor on Earth (familiars shedding lights), then Little Bug Storm.
- **Sakuya Izayoi (Classic)**: Killing Doll. Time stops for three seconds: every danmaku shot within
  40 blocks hangs in the air, and a quarter of her knives are thrown again.
- **Sanae Kochiya (Classic)**: her drawn star, then Miracle Fruit, Moses's Miracle, Yasaka's Divine
  Wind and Bewitching Power Spoiler (scales that come back to her; one that touches you withers and
  starves you; if she catches it again she gains regeneration and strength).
- **Toyosatomimi no Miko (Classic)**: Fantasy Seal.

A classic boss has all its stages in one health bar, as the Wither has its phases: its health
times its number of stages. Each time it loses one stage's worth, it goes on to the next stage,
and one blow can break only one stage. While a classic boss is near, a boss bar shows all of its
health and the spell card it is casting. A broken stage ends in a blast, and the last one ends in
a great explosion. Like the originals, Reimu, Marisa, Sakuya, Sanae and Miko don't fight until you hit them.

## Drops

The fairies and crow tengu (fairies, sunflower, ice, Nether and moon fairies, crow tengu, the Old
Crow Tengu and hell ravens) drop what Arcadian Dream's fairies drop: 1 to 4 of its power and point
items, and now and then a bomb, an extend, a star or the Fairy Playground disc. They sound as its
fairies do, with a bat's voice. A boss killed with `/kill` dies for good, with its drops.

With [Touhou Items](https://github.com/mxpbxb/touhou-items) installed, the other mobs scatter its Point
items when they die for good, a boss many more than a fairy, and power items: a fairy now and then a
small one, a boss big ones and sometimes Full Power, an Extend or a Bomb. Alice Dolls Restringed's
dolls take them as love.

## Spawning

Mobs spawn on their own in their original biomes, Biomes O' Plenty's included, by weight and light
(see [SPAWNS.md](SPAWNS.md)). Every mob's spawning is set in `spawns.cfg`, in the mod's config folder,
written with the defaults the first time the game runs:

```
rate = 1.25          # tries a second for each player, about
cap = 12             # the most of these mobs within 64 blocks of a player
fairy = on 30 1 6    # on/off, weight, smallest group, largest group
```

A saved change takes hold in game within ten seconds.

The named characters (the bosses, and the people) are rare: one comes at most every five minutes,
about once or twice a night, alone and never while another of the same one is about. And whenever a
player comes through a Yukari gap ([Yukari Gaps](https://github.com/mxpbxb/yukari-gaps)), once in
2,000 Yukari comes through after them.

## Not yet

- The other bosses' spell cards: SPELLCARD PLUS's (Wakasagihime, Hina, its Cirno and Rumia) are
  not ported, and the bosses above still fight with the nine cards ported before.
- The shapes of the star, butterfly, rice and crystal bullets for the bosses above (the classic
  bosses' are drawn in their shapes; the originals' 3D shot models are not).
- In Killing Doll's stopped time, players and mobs go on moving; only the danmaku stops.
- Sakuya's and Sanae's trades, and most of Rinnosuke's, which are for Touhou Items Mod's items.

## Building

From the decompiled originals and their jars (not in this repository):

- `convert_models.py` turns the Java models into `native/api/src/models_gen.rs`; `models.sh` runs it.
- The native crate's tests write `mobs.json`; `gen_content.py` builds the content mod from it and the
  jars' art. `spawns.py` writes `SPAWNS.md`.
- `preview.py` draws a converted model from the front, to check it without the game.

The native mod is built by the Horizon launcher (`native-build`), pinned to one Horizon revision.
