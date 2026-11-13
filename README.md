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

All 30 living mobs of the three mods (Grimoire of Alice adds none):

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

Each mob's attacks are its original `danmakuPattern` ported line for line, at 35% of the original
rate of fire. Its model is its Java model, converted part for part. Hostile mobs (but not creepers)
go for the Touhou mobs that are attacking, and a Touhou mob a vanilla mob hurts fights it back, its
danmaku aimed at that mob until it is dead or 40 blocks off.

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

- Most bosses' spell cards: nine of the originals' cards are ported; the rest come in turn.
- Lasers, and the shapes of the star, butterfly, rice and crystal bullets (they are drawn as glows).
- Sakuya's and Sanae's trades, and most of Rinnosuke's, which are for Touhou Items Mod's items.

## Building

From the decompiled originals and their jars (not in this repository):

- `convert_models.py` turns the Java models into `native/api/src/models_gen.rs`; `models.sh` runs it.
- The native crate's tests write `mobs.json`; `gen_content.py` builds the content mod from it and the
  jars' art. `spawns.py` writes `SPAWNS.md`.
- `preview.py` draws a converted model from the front, to check it without the game.

The native mod is built by the Horizon launcher (`native-build`), pinned to one Horizon revision.
