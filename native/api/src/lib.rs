//! Touhou Mobs for Horizon: the fairies, youkai, bosses and NPCs of Touhou Items Mod (kuro-anco),
//! SPELLCARD PLUS and Journey To Gensokyo (Katrix), in one mod. A port for personal use; the
//! original art is in the content mod `thmobs`.
//!
//! Each mob is a living type of the content mod copying the wandering trader, whose behaviour is all
//! goals: this mod clears them and gives it the danmaku mobs' own (`ai`), which runs its attack
//! (`attack`) from its row in `mobs`. A boss that runs out of health goes on to its next phase
//! (`living_death`). Each is drawn with its Java model, converted part for part (`model`); its danmaku
//! fly in the same engine as the spell cards' (`shot`, `pattern`, `cards`): the server moves them and
//! settles hits, clients are told each shot once.
#![allow(dead_code)] // The danmaku engine is shared with the spell cards port; not every part is used yet.

mod ai;
mod attack;
mod cards;
mod config;
mod model;
mod models_gen;
mod mobs;
mod pattern;
mod shot;
mod vec;

use std::collections::{HashMap, HashSet};
use std::sync::{LazyLock, Mutex};

use horizon_client::mobs::goals;
use horizon_client::mod_api::{
    ClientLevelUnloadEvent, ClientTickPostEvent, Compound, Flow, Hooks, Host, InteractEntityEvent, LayerDefinitionsEvent,
    LevelTickPostEvent, LivingDeathEvent, LivingIncomingDamageEvent, MobSubmitEvent, ModelPart, NativeHeightmap,
    PlayerTeleportedEvent,
    NativeInventoryKind, NativeMobRender, NativeWorldDraw, NetFromServerEvent, RegisterGoalsEvent, Tag, UseItemOnEvent,
    WorldSubmitEvent,
};

use cards::{Actor, Card, Ctx, card_position};
use mobs::Look;
use pattern::Fire;
use shot::{Batch, Shot, form, shot_full, special};
use vec::{Rng, V3, angle, angle_to, v3};

pub fn hooks() -> Box<dyn Hooks> {
    Box::new(Mod)
}

struct Mod;

const CHANNEL: &str = "thmobs_native:shots";
const DIMENSIONS: [&str; 3] = ["minecraft:overworld", "minecraft:the_nether", "minecraft:the_end"];

// ---- The server's side ----

struct Live {
    shot: Shot,
    owner: u32,
}

/// A spell card a mob declared.
struct Cast {
    card: Card,
    mob: u32,
    time: i32,
    /// Toward its next tick, at the mobs' pace.
    pace: f32,
    card_pos: V3,
    card_yaw: f32,
    state: [f32; 2],
    target: V3,
}

struct Server {
    config: config::Loaded,
    shots: HashMap<String, Vec<Live>>,
    casts: HashMap<String, Vec<Cast>>,
    next_id: u32,
    rng: Rng,
    /// Spawner ticks before the next named character (a boss, or one of the people) may spawn.
    named_wait: u32,
}

/// A named character: a boss with phases, or one of the people. One of each at a time, one at a
/// time, and seldom: about once or twice a night.
fn named(m: &mobs::Mob) -> bool {
    m.npc || !m.phases.is_empty()
}

/// Ticks between named characters: five minutes, half a night.
const NAMED_EVERY: u32 = 6000;
/// After that wait, the chance a named character the spawner picks really comes, in 100.
const NAMED_CHANCE: u32 = 35;
/// Yukari's chance to come through when a Yukari gap is used: 0.05%, 1 in 2,000.
const YUKARI_FROM_GAP: u32 = 2_000;

static SERVER: LazyLock<Mutex<Server>> = LazyLock::new(|| {
    let seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(1, |d| d.as_nanos() as u64);
    Mutex::new(Server {
        config: config::Loaded::new(),
        shots: HashMap::new(),
        casts: HashMap::new(),
        next_id: 1,
        rng: Rng::new(seed),
        named_wait: NAMED_EVERY / 2,
    })
});

fn ray_box(a: V3, d: V3, reach: f64, b: [[f64; 3]; 2]) -> Option<f64> {
    let (mut near, mut far) = (0.0f64, reach);
    for (o, v, lo, hi) in [(a.x, d.x, b[0][0], b[1][0]), (a.y, d.y, b[0][1], b[1][1]), (a.z, d.z, b[0][2], b[1][2])] {
        if v.abs() < 1e-12 {
            if o < lo || o > hi {
                return None;
            }
        } else {
            let (t0, t1) = ((lo - o) / v, (hi - o) / v);
            near = near.max(t0.min(t1));
            far = far.min(t0.max(t1));
            if near > far {
                return None;
            }
        }
    }
    Some(near)
}

fn grow(b: [[f64; 3]; 2], by: f64) -> [[f64; 3]; 2] {
    [[b[0][0] - by, b[0][1] - by, b[0][2] - by], [b[1][0] + by, b[1][1] + by, b[1][2] + by]]
}

fn middle(b: [[f64; 3]; 2]) -> V3 {
    v3((b[0][0] + b[1][0]) / 2.0, (b[0][1] + b[1][1]) / 2.0, (b[0][2] + b[1][2]) / 2.0)
}

fn passable(block: &str) -> bool {
    const SOFT: [&str; 30] = [
        "air", "water", "lava", "grass", "fern", "torch", "vine", "vines", "sapling", "rail", "button", "carpet", "sign",
        "kelp", "kelp_plant", "snow", "bush", "flower", "tulip", "poppy", "dandelion", "orchid", "allium", "bluet",
        "daisy", "cornflower", "lily_of_the_valley", "mushroom", "roots", "sprouts",
    ];
    let name = block.split('[').next().unwrap_or(block);
    SOFT.iter().any(|s| name.ends_with(s))
}

fn is_ours(kind: &str) -> bool {
    kind.starts_with("thmobs:")
}

/// Whether it is day, as the sky's light goes.
fn daytime(time: i64) -> bool {
    let t = time.rem_euclid(24000);
    !(12_542..=23_459).contains(&t)
}

/// A mob as a caster: where it stands and looks.
fn actor_of(host: &dyn Host, id: u32) -> Option<Actor> {
    let e = host.entity(id)?;
    let pos = V3::from_array(host.entity_position(e)?);
    let [yaw, pitch] = host.entity_rotation(e).unwrap_or([0.0, 0.0]);
    let eye_height = host.entity_eye_height(e).unwrap_or(1.62);
    Some(Actor { pos, eye_height, yaw, pitch })
}

impl Server {
    fn give_id(&mut self, mut s: Shot) -> Shot {
        s.id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1).max(1);
        s
    }

    fn add(&mut self, dimension: &str, s: Shot, owner: u32, batch: &mut Batch) {
        let s = self.give_id(s);
        batch.fired.push(s.clone());
        self.shots.entry(dimension.to_owned()).or_default().push(Live { shot: s, owner });
    }

    fn tick(&mut self, host: &mut dyn Host, dimension: &str) -> Batch {
        let mut batch = Batch::default();
        let level = host.level(dimension);
        let entities = host.level_entities(dimension);
        let here: HashSet<u32> = entities.iter().map(|(id, _, _)| *id).collect();
        let player = host
            .server_players()
            .into_iter()
            .filter(|p| p.dimension == dimension && p.health > 0.0 && p.game_mode != 3)
            .map(|p| V3::from_array(p.position) + v3(0.0, 0.9, 0.0))
            .collect::<Vec<_>>();

        // What the mobs of this level fired, and the cards they declared.
        let fired: Vec<(u32, Shot, bool)> = ai::FIRED.lock().map(|mut q| std::mem::take(&mut *q)).unwrap_or_default();
        for (owner, s, sound) in fired {
            if !here.contains(&owner) {
                continue;
            }
            if sound && let Some(l) = level {
                host.level_play_sound(l, "minecraft:entity.arrow.shoot", s.pos.array(), 5, 0.6, 1.2);
            }
            self.add(dimension, s, owner, &mut batch);
        }
        let declared: Vec<(u32, u32)> = ai::CASTS.lock().map(|mut q| std::mem::take(&mut *q)).unwrap_or_default();
        for (mob, number) in declared {
            if !here.contains(&mob) {
                continue;
            }
            let (Some(card), Some(user)) = (Card::by_number(number), actor_of(host, mob)) else { continue };
            let casts = self.casts.entry(dimension.to_owned()).or_default();
            casts.retain(|c| c.mob != mob);
            let target = player.first().copied().unwrap_or(user.pos + user.look() * 16.0);
            casts.push(Cast { card, mob, time: 0, pace: 0.0, card_pos: card_position(&user), card_yaw: user.yaw, state: [0.0; 2], target });
        }

        // The spell cards being cast.
        let mut casts = self.casts.remove(dimension).unwrap_or_default();
        casts.retain_mut(|cast| {
            let Some(user) = actor_of(host, cast.mob) else { return false };
            cast.pace += ai::PACE;
            if cast.pace < 1.0 {
                return true;
            }
            cast.pace -= 1.0;
            let near = player.iter().copied().min_by(|a, b| (*a - user.pos).length().total_cmp(&(*b - user.pos).length()));
            if let Some(t) = near {
                cast.target = t;
            }
            let ctx = Ctx {
                time: cast.time,
                level: ai::level(),
                user,
                target: cast.target,
                target_alive: near.is_some(),
                card: cast.card_pos,
                card_yaw: cast.card_yaw,
            };
            let mut out = Vec::new();
            cast.card.tick(&ctx, &mut cast.state, &mut Fire { out: &mut out, rng: &mut self.rng });
            if cast.card == Card::PerfectFreeze && cast.time == 64 {
                self.freeze(dimension, ctx.pos_user(), &mut out, &mut batch.gone);
            }
            for s in out {
                self.add(dimension, s, cast.mob, &mut batch);
            }
            cast.time += 1;
            cast.time < cast.card.info().end
        });
        if !casts.is_empty() {
            self.casts.insert(dimension.to_owned(), casts);
        }

        if self.shots.get(dimension).is_some_and(|v| !v.is_empty()) {
            let difficulty = level.and_then(|l| host.level_difficulty(l)).unwrap_or("normal");
            let bodies: Vec<(u32, bool, [[f64; 3]; 2])> = entities
                .iter()
                .filter_map(|(id, kind, _)| {
                    let e = host.entity(*id)?;
                    host.living(e)?;
                    Some((*id, is_ours(kind), host.entity_bounding_box(e)?))
                })
                .collect();
            let mut shots = self.shots.remove(dimension).unwrap_or_default();
            let mut kept = Vec::with_capacity(shots.len());
            let mut fresh = Vec::new();
            let drained: Vec<Live> = shots.drain(..).collect();
            for mut live in drained {
                if !live.shot.step() {
                    continue;
                }
                // Gagouji Cyclone's shots turn at their shooter's target at their end.
                if live.shot.special == special::SEEK_TARGET && live.shot.ticks >= live.shot.end {
                    if let (Some(t), Some(from)) = (player.first(), bodies.iter().find(|b| b.0 == live.owner)) {
                        let s = &mut live.shot;
                        s.angle = angle_to(middle(from.2), *t - v3(0.0, 0.9, 0.0));
                        s.motion = s.angle * s.speed();
                    }
                    let s = &mut live.shot;
                    s.end = 90;
                    s.accel = 0.03;
                    s.limit = 0.2 + f64::from(ai::level()) * 0.1;
                    s.special = 0;
                    batch.fired.push(s.clone());
                }
                // Bursts: shots some specials fire from where they are.
                let burst = bursts(&live.shot, actor_of(host, live.owner).map(|u| u.look()), &mut self.rng);
                for child in burst {
                    let child = self.give_id(child);
                    batch.fired.push(child.clone());
                    fresh.push(Live { shot: child, owner: live.owner });
                }
                // Fantasy Seal's amulets: out, still, a change of colour, then at the target, twice.
                if (11100..=11105).contains(&live.shot.special) && live.shot.ticks == live.shot.end {
                    let from = bodies.iter().find(|b| b.0 == live.owner).map(|b| middle(b.2));
                    fantasy_seal(&mut live.shot, from.zip(player.first().copied()));
                    batch.fired.push(live.shot.clone());
                }
                let s = &live.shot;
                let mut hit = false;
                // A familiar's shot carries on; it doesn't hit.
                if s.solid() && s.form != form::FAMILIAR {
                    let travel = s.pos - s.prev;
                    let reach = travel.length();
                    let dir = if reach > 1e-9 { travel * (1.0 / reach) } else { v3(0.0, 1.0, 0.0) };
                    for (id, ours, b) in &bodies {
                        // A mob's danmaku passes through the mobs of this mod.
                        if *id == live.owner || *ours {
                            continue;
                        }
                        if ray_box(s.prev, dir, reach.max(1e-6), grow(*b, f64::from(s.size) * 0.5)).is_some() {
                            host.hurt_entity(dimension, *id, s.damage_for(false, difficulty), "minecraft:mob_projectile", Some(live.owner));
                            hit = true;
                            break;
                        }
                    }
                    if !hit && let Some(l) = level {
                        let at = [s.pos.x.floor() as i32, s.pos.y.floor() as i32, s.pos.z.floor() as i32];
                        hit = host.level_block(l, at).is_some_and(|b| !passable(&b));
                    }
                }
                if hit {
                    batch.gone.push(live.shot.id);
                } else {
                    kept.push(live);
                }
            }
            kept.append(&mut fresh);
            *self.shots.entry(dimension.to_owned()).or_default() = kept;
        }
        batch
    }

    /// Perfect Freeze at its sixty-fourth tick: every shot within twenty blocks stops, turns white and
    /// faces a random way, to set off again twenty ticks on.
    fn freeze(&mut self, dimension: &str, at: V3, fired: &mut Vec<Shot>, gone: &mut Vec<u32>) {
        let Some(shots) = self.shots.get_mut(dimension) else { return };
        let mut frozen = Vec::new();
        shots.retain(|live| {
            let d = live.shot.pos - at;
            let near = d.x.abs() <= 20.0 && d.y.abs() <= 20.0 && d.z.abs() <= 20.0;
            if near {
                frozen.push(live.shot.clone());
                gone.push(live.shot.id);
            }
            !near
        });
        for old in frozen {
            let a = angle(self.rng.float() * 360.0, self.rng.float() * 180.0 - 90.0);
            let data = shot_full(old.form, 7, old.size, old.damage, 0, old.end - old.ticks + 50, special::FREEZE);
            fired.push(Shot::new(old.pos, a, 0.0, pattern::rotate_default(), 0.0, 9999, 1e-4, 1e-4, 0.0, V3::ZERO, data));
        }
    }

    /// Where something may stand in column `x`, `z` near height `near`: on the surface in the
    /// overworld, else the first floor found going down from a little above.
    fn floor(host: &dyn Host, level: horizon_client::mod_api::LevelRef, dimension: &str, x: i32, z: i32, near: i32) -> Option<i32> {
        if dimension == "minecraft:overworld" {
            let heights = host.surface_heights(dimension, &[(x, z)], NativeHeightmap::MotionBlocking).ok()?;
            return heights.first().copied().flatten();
        }
        let open = |y: i32| host.level_block(level, [x, y, z]).is_some_and(|b| b.ends_with("air"));
        (near - 16..near + 16).rev().find(|&y| open(y) && open(y + 1) && !open(y - 1))
    }

    /// About once a second for each player, a try at a group of one of our mobs 24 to 48 blocks off,
    /// by the original's weights, biomes and rules.
    fn spawn(&mut self, host: &mut dyn Host, dimension: &str) {
        let Some(level) = host.level(dimension) else { return };
        let players: Vec<_> = host.server_players().into_iter().filter(|p| p.dimension == dimension && p.game_mode != 3).collect();
        if players.is_empty() || host.level_difficulty(level) == Some("peaceful") {
            return;
        }
        self.named_wait = self.named_wait.saturating_sub(1);
        let entities = host.level_entities(dimension);
        let ours: Vec<[f64; 3]> = entities.iter().filter(|(_, k, _)| is_ours(k)).map(|(_, _, p)| *p).collect();
        let alive: HashSet<String> = entities.iter().filter(|(_, k, _)| is_ours(k)).map(|(_, k, _)| k.to_string()).collect();
        self.config.refresh(|| host.config_dir().ok());
        let cfg = self.config.config.clone();
        let total: u32 = mobs::MOBS.iter().filter_map(|m| cfg.rule(m)).map(|r| r.weight).sum();
        for p in players {
            // `rate` tries a second, about, at twenty ticks a second.
            if f64::from(self.rng.float()) >= cfg.rate / 20.0 || total == 0 {
                continue;
            }
            let near = ours.iter().filter(|q| (q[0] - p.position[0]).hypot(q[2] - p.position[2]) < 64.0).count();
            if near >= cfg.cap {
                continue;
            }
            let mut pick = self.rng.int(total);
            let Some(mob) = mobs::MOBS.iter().find(|m| match cfg.rule(m) {
                Some(r) if pick < r.weight => true,
                Some(r) => {
                    pick -= r.weight;
                    false
                }
                None => false,
            }) else {
                continue;
            };
            let (Some(rule), Some(chosen)) = (mob.spawn, cfg.rule(mob)) else { continue };
            // A named character: only after the wait, now and then, and never two of the same.
            let is_named = named(mob);
            if is_named
                && (self.named_wait > 0 || self.rng.int(100) >= NAMED_CHANCE || alive.contains(&format!("thmobs:{}", mob.id)))
            {
                continue;
            }
            let a = f64::from(self.rng.float()) * std::f64::consts::TAU;
            let r = 24.0 + f64::from(self.rng.float()) * 24.0;
            let (x, z) = ((p.position[0] + a.cos() * r).floor() as i32, (p.position[2] + a.sin() * r).floor() as i32);
            let Some(y) = Self::floor(host, level, dimension, x, z, p.position[1] as i32) else { continue };
            let Some(biome) = host.biome([x, y, z]) else { continue };
            let biome = biome.strip_prefix("minecraft:").unwrap_or(&biome).to_owned();
            if !rule.biomes.contains(&biome.as_str()) {
                continue;
            }
            let (block_light, sky) = host.light([x, y, z]);
            let day = daytime(host.level_day_time(level).unwrap_or(0));
            match rule.light {
                mobs::Light::Fairy => {
                    if self.rng.int(100) < 50 || (y < 64 && y < self.rng.int(64) as i32) || sky <= 7 || !day {
                        continue;
                    }
                    let below = host.level_block(level, [x, y - 1, z]).unwrap_or_default();
                    if !["minecraft:grass_block", "minecraft:dirt", "minecraft:sand"].iter().any(|b| below.starts_with(b)) {
                        continue;
                    }
                }
                mobs::Light::Dark => {
                    let lit = if day { u32::from(sky.max(block_light)) } else { u32::from(block_light) };
                    if lit > self.rng.int(8) {
                        continue;
                    }
                }
                mobs::Light::Any => {}
                mobs::Light::Rare => {
                    if self.rng.int(100) < 98 {
                        continue;
                    }
                }
            }
            let count = if is_named { 1 } else { chosen.min + self.rng.int(chosen.max - chosen.min + 1) };
            if is_named {
                self.named_wait = NAMED_EVERY;
            }
            for _ in 0..count {
                let (dx, dz) = (self.rng.int(5) as i32 - 2, self.rng.int(5) as i32 - 2);
                let at = [f64::from(x + dx) + 0.5, f64::from(y) + 0.1, f64::from(z + dz) + 0.5];
                summon(host, dimension, mob, at);
            }
        }
    }
}

/// The shots a special fires from where its shot is, this tick.
fn bursts(s: &Shot, owner_look: Option<V3>, rng: &mut Rng) -> Vec<Shot> {
    let mut out = Vec::new();
    // A spray that runs every tick runs only on the mobs' share of them.
    let paced = |t: i32| ((t as f32) * ai::PACE).floor() != ((t - 1) as f32 * ai::PACE).floor();
    if (11600..=11602).contains(&s.special) && !paced(s.ticks) {
        return out;
    }
    let mut fire = Fire { out: &mut out, rng };
    match s.special {
        // Marisa's familiars spray stars as they circle.
        11600 => fire.sphere(s.pos, s.angle, 0.0, pattern::rotate_default(), 0.0, 9999, 0.3, 0.5, 0.05, V3::ZERO, shot::shot(8, s.color, 0, 60, 0), 3, 0.0, 0.0),
        11601 if s.ticks % 2 == 0 => {
            fire.sphere(s.pos, s.angle, 0.0, pattern::rotate_default(), 0.0, 9999, 0.3, 0.5, 0.05, V3::ZERO, shot::shot(8, s.color, 0, 60, 0), 2, 0.0, 0.0)
        }
        11602 if s.ticks % 2 == 0 => {
            fire.sphere(s.pos, s.angle, 0.0, pattern::rotate_default(), 0.0, 9999, 0.3, 0.5, 0.05, V3::ZERO, shot::shot(8, shot::RANDOM, 0, 60, 0), 3, 0.0, 0.0)
        }
        special::MISFORTUNE_FAN if s.ticks == 10 => {
            let d = shot_full(0, s.color, shot::SIZE[0], s.damage, 0, 60, 0);
            fire.wide(s.pos, s.angle, 0.0, 0, 0.5, 0.0, 0.0, V3::ZERO, d, 5, 60.0, 0.0, 0.0);
            for limit in [0.7, 0.8, 0.9, 1.0] {
                let d = shot_full(10, s.color, 0.1, s.damage, 0, 60, 0);
                fire.shot(s.pos, s.angle, 0.0, 0.5, limit, 0.1, V3::ZERO, d);
            }
        }
        special::MISFORTUNE_RING if s.ticks == 10 => {
            let d = shot_full(0, s.color, shot::SIZE[0], s.damage, 0, 60, 0);
            fire.circle(s.pos, s.angle, 0.0, 0, 0.5, 0.0, 0.0, V3::ZERO, d, 36, 0.0, 0.0);
        }
        special::MISFORTUNE_MARK if s.ticks == s.end => {
            let d = shot_full(14, s.color, shot::SIZE[14], s.damage, 0, 60, 0);
            let look = owner_look.unwrap_or(s.angle);
            fire.circle(s.pos, look, 0.0, 0, 0.0, 0.5, 0.05, V3::ZERO, d, ai::level(), 0.0, (s.id % 36) as f32 * 10.0);
        }
        _ => {}
    }
    out
}

/// One step of Fantasy Seal's amulets (`ShotFantasySeal`), at the end of the step before.
fn fantasy_seal(s: &mut Shot, aim: Option<(V3, V3)>) {
    let speed = s.speed();
    let aim_at = |s: &mut Shot| {
        if let Some((from, to)) = aim {
            s.angle = angle_to(from, to);
        }
    };
    match s.special {
        11100 => {
            (s.form, s.special, s.end, s.limit, s.accel) = (4, 11101, 10, 0.0, 0.0);
            s.restart(0.0);
        }
        11101 => {
            (s.color, s.special, s.end) = (0, 11102, 10);
            s.restart(speed);
        }
        11102 => {
            (s.form, s.special, s.end, s.limit, s.accel) = (27, 11103, 30, 0.0, -0.04);
            aim_at(s);
            s.restart(0.75);
        }
        11103 => {
            (s.form, s.special, s.end) = (4, 11104, 10);
            s.restart(speed);
        }
        11104 => {
            (s.color, s.special, s.end) = (4, 11105, 10);
            s.restart(speed);
        }
        _ => {
            (s.form, s.special, s.end, s.limit, s.accel) = (27, 0, 50, 0.3, 0.0);
            aim_at(s);
            s.restart(0.3);
        }
    }
}

/// A trade for a wandering trader's `Offers`: `count` of `buy` for one `sell`.
fn offer(buy: &str, count: i32, sell: &str) -> Tag {
    let stack = |id: &str, count: i32| -> Tag {
        Tag::Compound([("id".into(), Tag::String(id.into())), ("count".into(), Tag::Int(count))].into())
    };
    Tag::Compound(
        [
            ("buy".into(), stack(buy, count)),
            ("sell".into(), stack(sell, 1)),
            ("maxUses".into(), Tag::Int(9999)),
            ("rewardExp".into(), Tag::Byte(0)),
        ]
        .into(),
    )
}

fn summon(host: &mut dyn Host, dimension: &str, mob: &mobs::Mob, at: [f64; 3]) -> Option<u32> {
    let mut nbt: Compound = [("Silent".into(), Tag::Byte(1)), ("Health".into(), Tag::Float(mob.health.to_bits()))].into();
    if mob.id == "rinnosuke" {
        // The one trade of his made of vanilla things; the rest are Touhou Items Mod's items, to come.
        let offers: Compound = [("Recipes".into(), Tag::List(vec![offer("minecraft:gold_ingot", 1, "minecraft:ender_pearl")]))].into();
        nbt = [
            ("Silent".into(), Tag::Byte(1)),
            ("Health".into(), Tag::Float(mob.health.to_bits())),
            ("Offers".into(), Tag::Compound(offers)),
        ]
        .into();
    }
    let id = host.summon(dimension, &format!("thmobs:{}", mob.id), at, nbt)?;
    // A sunflower fairy comes with a familiar.
    if mob.id == "sunflower_fairy"
        && let Some(familiar) = mobs::by_kind("thmobs:familiar")
    {
        let up = [at[0], at[1] + 1.8, at[2]];
        let nbt: Compound = [("Silent".into(), Tag::Byte(1)), ("Health".into(), Tag::Float(familiar.health.to_bits()))].into();
        host.summon(dimension, "thmobs:familiar", up, nbt);
    }
    Some(id)
}

// ---- The client's side ----

static CLIENT: LazyLock<Mutex<HashMap<u32, Shot>>> = LazyLock::new(|| Mutex::new(HashMap::new()));
/// Each model, baked once.
static BAKED: LazyLock<Mutex<HashMap<&'static str, ModelPart>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

const TINT: [[u8; 3]; 8] =
    [[224, 0, 0], [0, 0, 224], [0, 224, 0], [224, 224, 0], [224, 0, 224], [0, 224, 224], [255, 128, 0], [255, 255, 255]];

fn strip(form: u8) -> Option<&'static str> {
    Some(match form {
        form::SMALL | form::TINY | form::MEDIUM => "thmobs:entity/shot/small",
        form::PEARL => "thmobs:entity/shot/pearl",
        form::CIRCLE => "thmobs:entity/shot/circle",
        form::HEART => "thmobs:entity/shot/heart",
        form::TALISMAN => "thmobs:entity/shot/talisman",
        form::BIG => "thmobs:entity/shot/big",
        _ => return None,
    })
}

const LIGHT: &str = "thmobs:entity/shot/light";

fn draw_shot(host: &dyn Host, draw: &mut NativeWorldDraw, s: &Shot, right: V3, up: V3) {
    let at = s.prev.lerp(s.pos, f64::from(draw.partial));
    let color = usize::from(s.color % 8);
    let tint = TINT[color];
    let frame = f32::from(u8::from(s.count.rem_euclid(2) == 1)) * 8.0;
    let light_uv = [frame, 0.0, frame + 8.0, 16.0];
    let mut quads: Vec<(&str, [f32; 4], f64, [u8; 4])> = Vec::with_capacity(2);
    if s.count < 0 {
        let grown = (f64::from((-s.count).min(10)) * 0.3 * f64::from(s.size)).min(1.0);
        quads.push((LIGHT, light_uv, grown, [tint[0], tint[1], tint[2], 128]));
    } else if let Some(sheet) = strip(s.form) {
        let u = color as f32 * 2.0;
        quads.push((sheet, [u, 0.0, u + 2.0, 16.0], f64::from(s.size), [255, 255, 255, 255]));
    } else {
        let (outer, inner) = if matches!(s.form, form::LIGHT | form::BIG_LIGHT) { (2.0, 1.0) } else { (1.4, 0.7) };
        let size = f64::from(s.size);
        quads.push((LIGHT, light_uv, size * outer, [tint[0], tint[1], tint[2], 204]));
        quads.push((LIGHT, light_uv, size * inner, [255, 255, 255, 255]));
    }
    draw.push();
    if draw.translate_world(at.array()) {
        for (sprite, uv, half, rgba) in quads {
            let (r, u) = (right * half, up * half);
            let c = |p: V3| [p.x as f32, p.y as f32, p.z as f32];
            host.world_quad(draw, sprite, [c(-r + u), c(-r - u), c(r - u), c(r + u)], uv, rgba);
        }
    }
    draw.pop();
}

/// The phantoms and familiars: a phantom a column of glows rising and shrinking (`RenderTHPhantom`),
/// a familiar its picture, each turned to the camera.
fn draw_glows(host: &dyn Host, draw: &mut NativeWorldDraw, right: V3, up: V3) {
    let partial = f64::from(draw.partial);
    for e in host.client_entities() {
        let Some(spec) = mobs::by_kind(&e.kind) else { continue };
        let Look::Glow(sprite, size) = spec.look else { continue };
        if !e.alive || e.invisible {
            continue;
        }
        let at = V3::from_array(e.previous_position).lerp(V3::from_array(e.position), partial) + v3(0.0, 0.5, 0.0);
        let tint = TINT[(e.id % 8) as usize];
        let mut quads: Vec<(String, [f32; 4], V3, f64, [u8; 4])> = Vec::new();
        if sprite == "light" {
            let t = (draw.game_time as f64 + partial) % 20.0;
            let lift = v3(0.0, 0.1 + t * 0.06, 0.0);
            let shrink = (20.0 - t) / 20.0;
            for (k, scale) in [(0.0, 1.0), (1.0, 0.9 * shrink), (2.0, 0.8 * shrink * 0.9 * shrink)] {
                let centre = at + lift * k;
                quads.push((LIGHT.to_owned(), [0.0, 0.0, 8.0, 16.0], centre, f64::from(size) * scale * 1.4, [tint[0], tint[1], tint[2], 200]));
                quads.push((LIGHT.to_owned(), [0.0, 0.0, 8.0, 16.0], centre, f64::from(size) * scale * 0.7, [255, 255, 255, 255]));
            }
        } else {
            quads.push((format!("thmobs:entity/{sprite}"), [0.0, 0.0, 16.0, 16.0], at, f64::from(size), [255, 255, 255, 255]));
        }
        for (name, uv, centre, half, rgba) in quads {
            draw.push();
            if draw.translate_world(centre.array()) {
                let (r, u) = (right * half, up * half);
                let c = |p: V3| [p.x as f32, p.y as f32, p.z as f32];
                host.world_quad(draw, &name, [c(-r + u), c(-r - u), c(r - u), c(r + u)], uv, rgba);
            }
            draw.pop();
        }
    }
}

fn set(root: &mut ModelPart, name: &str, f: impl Fn(&mut ModelPart)) {
    if let Some(p) = model::part_mut(root, name) {
        f(p);
    }
}

/// `ModelTHFairy.setRotationAngles` and its kin, for a fairy in the air or on the ground; for the
/// others, a head that turns, limbs that swing and wings that beat, by their parts' names.
fn pose(root: &mut ModelPart, model: &str, mob: &NativeMobRender<'_>) {
    let tick = mob.age;
    let (yaw, pitch) = (mob.head_yaw.to_radians(), mob.pitch.to_radians());
    // The crow tengu and the hell raven: `ModelTenguCrow` and `ModelHellRaven.setRotationAngles`. The
    // head turns, the body bobs, and the two-piece wings beat, the outer piece half as far.
    if let Some(bird) = [("jtg/ModelTenguCrow", "tengu"), ("jtg/ModelHellRaven", "raven")].iter().find(|(m, _)| *m == model).map(|(_, b)| *b) {
        set(root, &format!("{bird}Head"), |p| {
            p.y_rot = yaw;
            p.x_rot = pitch;
        });
        set(root, &format!("{bird}Body"), |p| p.x_rot = std::f32::consts::FRAC_PI_3 + (tick * 0.1).cos() * 0.15);
        let wing = (tick * 0.8).cos() * std::f32::consts::PI * 0.25;
        set(root, &format!("{bird}WingRight"), |p| p.y_rot = wing);
        set(root, &format!("{bird}WingLeft"), |p| p.y_rot = -wing);
        set(root, &format!("{bird}WingRight2"), |p| p.y_rot = wing * 0.5);
        // So the raven's was named: `ravenWingleft2`.
        for left2 in ["tenguWingLeft2", "ravenWingleft2"] {
            set(root, left2, |p| p.y_rot = -wing * 0.5);
        }
        return;
    }
    for head in ["head", "bipedHead", "Head"] {
        set(root, head, |p| {
            p.y_rot = yaw;
            p.x_rot = pitch;
        });
    }
    let flying = !mob.on_ground;
    let fairy = model.contains("Fairy");
    if fairy && flying {
        let lz = ((tick / 10.0).sin() * 0.1).abs();
        let lx = ((tick / 10.0).sin() * 0.2).abs();
        set(root, "rightLeg", |p| {
            p.z_rot = lz;
            p.x_rot = lx;
        });
        // So it was written: the left leg takes the right one's sideways swing forwards.
        set(root, "leftLeg", |p| {
            p.z_rot = -lz;
            p.x_rot = lz;
        });
        let arm = -0.7 - (tick / 10.0).sin() * 0.1;
        set(root, "rightArm", |p| {
            p.x_rot = arm;
            p.y_rot = 0.0;
            p.z_rot = -0.6457718;
        });
        set(root, "leftArm", |p| {
            p.x_rot = arm;
            p.y_rot = 0.0;
            p.z_rot = 0.6457718;
        });
    } else {
        let (m, far) = (mob.walk_position * 0.6662, mob.walk_speed);
        let swing = m.cos() * far;
        let back = (m + std::f32::consts::PI).cos() * far;
        // Swings on top of the pose the model rests in, so arms held out stay out.
        for (name, v) in [("rightLeg", swing * 0.7), ("bipedRightLeg", swing * 1.4), ("leftLeg", back * 0.7), ("bipedLeftLeg", back * 1.4)] {
            set(root, name, |p| p.x_rot += v);
        }
        for (name, v) in [("rightArm", back * 0.5), ("bipedRightArm", back * 0.5), ("leftArm", swing * 0.5), ("bipedLeftArm", swing * 0.5)] {
            set(root, name, |p| p.x_rot += v);
        }
    }
    let wing = if flying {
        (tick * 1.3).cos() * std::f32::consts::PI * 0.25
    } else {
        (tick * 0.5).cos() * std::f32::consts::PI * 0.1 + 0.47123894
    };
    for (name, sign) in [
        ("rightWing", 1.0),
        ("rightWing2", 1.0),
        ("leftWing", -1.0),
        ("leftWing2", -1.0),
        ("wingRight", 1.0),
        ("wingLeft", -1.0),
        ("RightWing", 1.0),
        ("LeftWing", -1.0),
    ] {
        set(root, name, |p| p.y_rot = wing * sign);
    }
}

impl Hooks for Mod {
    fn layer_definitions(&self, event: &mut LayerDefinitionsEvent<'_>) {
        event.layers.extend(model::layers());
    }

    fn register_goals(&self, event: &mut RegisterGoalsEvent<'_>) {
        // Every hostile mob goes for our mobs that are attacking, ahead of anything else.
        let kind = &event.mob.kind;
        if kind.id == "minecraft:creeper" {
            event.mob.ai.targets.add(-100, ai::CreeperPeace);
            return;
        }
        if kind.category == "monster" && !["minecraft:enderman", "minecraft:zombified_piglin", "minecraft:piglin", "minecraft:creeper"].contains(&kind.id.as_str()) {
            event.mob.ai.targets.add(0, ai::TargetTouhou::default());
            return;
        }
        let Some(spec) = mobs::by_kind(&event.mob.kind.id) else { return };
        let id = event.mob.id;
        let ai = &mut event.mob.ai;
        ai.goals.clear();
        ai.targets.clear();
        ai.goals.add(0, goals::Float);
        if spec.id == "cursed_decoy_doll" {
            ai.goals.add(1, ai::Decoy::default());
        } else if spec.id == "yin_yang_orb" {
            ai.goals.add(1, ai::Orbit::default());
        } else if spec.npc {
            ai.goals.add(5, goals::Stroll::new(0.5));
            ai.goals.add(6, goals::LookAtPlayer::new(8.0));
        } else {
            ai.goals.add(1, ai::Danmaku::new(spec, id));
        }
        event.mob.no_gravity = spec.ai.flying_height > 0;
    }

    /// A boss out of health goes on to its next phase, as `moveDanmakuAttack` had it: back to full
    /// health of the next, a moment unhurt, its danmaku gone.
    fn living_death(&self, host: &mut dyn Host, event: &mut LivingDeathEvent<'_>) -> Flow {
        let Some(spec) = mobs::by_kind(event.kind) else { return Flow::Continue };
        let phase = ai::phase_of(event.entity, spec);
        let Some(next) = spec.phases.iter().find(|p| p.from == phase) else { return Flow::Continue };
        if next.hp <= 0.0 {
            return Flow::Continue;
        }
        if let Ok(mut bosses) = ai::BOSSES.lock() {
            bosses.insert(event.entity, ai::Boss { phase: next.to, counter: Some(-next.interval), invincible: next.invincible, ai: next.ai });
        }
        for d in DIMENSIONS {
            if host.set_mob_health(d, event.entity, next.hp) {
                break;
            }
        }
        if let Ok(mut server) = SERVER.lock() {
            for (_, shots) in server.shots.iter_mut() {
                shots.retain(|l| l.owner != event.entity);
            }
            for (_, casts) in server.casts.iter_mut() {
                casts.retain(|c| c.mob != event.entity);
            }
        }
        Flow::Cancel
    }

    /// A boss can't be hurt in a new phase's first moments.
    fn living_incoming_damage(&self, event: &mut LivingIncomingDamageEvent<'_>) -> Flow {
        if is_ours(event.kind) && ai::invincible(event.entity as u32) { Flow::Cancel } else { Flow::Continue }
    }

    /// A right click: Rinnosuke trades; Miko sets the danmaku level, from easy to lunatic and round
    /// again; the others do nothing.
    fn interact_entity(&self, host: &mut dyn Host, event: &mut InteractEntityEvent<'_>) -> Flow {
        if !is_ours(event.kind) {
            return Flow::Continue;
        }
        if event.kind == "thmobs:rinnosuke" {
            return Flow::Continue;
        }
        if event.kind == "thmobs:miko"
            && let Some(player) = host.acting_player()
        {
            static LAST: Mutex<Option<std::time::Instant>> = Mutex::new(None);
            let fresh = LAST.lock().map(|mut last| {
                let now = std::time::Instant::now();
                let fresh = last.is_none_or(|t| now.duration_since(t).as_millis() > 300);
                *last = Some(now);
                fresh
            });
            if fresh.unwrap_or(false) {
                let next = ai::level() % 4 + 1;
                ai::LEVEL.store(next, std::sync::atomic::Ordering::Relaxed);
                let name = ["EASY", "NORMAL", "HARD", "LUNATIC"][(next - 1) as usize];
                host.action_bar(player.uuid, horizon_client::mod_api::Text::literal(name));
            }
        }
        Flow::Cancel
    }

    /// A spawn egg, used on a block: the mob, standing on it.
    fn use_item_on(&self, host: &mut dyn Host, event: &mut UseItemOnEvent<'_>) -> Flow {
        let Some(id) = event.stack.item.strip_prefix("thmobs:").and_then(|i| i.strip_suffix("_spawn_egg")) else {
            return Flow::Continue;
        };
        let Some(player) = host.acting_player() else { return Flow::Continue };
        let Some(mob) = mobs::MOBS.iter().find(|m| m.id == id) else { return Flow::Continue };
        let at = [
            f64::from(event.pos[0] + event.face[0]) + 0.5,
            f64::from(event.pos[1] + event.face[1]),
            f64::from(event.pos[2] + event.face[2]) + 0.5,
        ];
        summon(host, &player.dimension, mob, at);
        if player.game_mode != 1 {
            let mut rest = event.stack.clone();
            rest.count = rest.count.saturating_sub(1);
            let rest = (rest.count > 0).then_some(rest);
            let _ = [usize::from(player.selected_slot), 40].into_iter().any(|slot| {
                host.set_inventory_slot(player.uuid, NativeInventoryKind::Player, slot, Some(event.stack), rest.clone()).is_ok()
            });
        }
        Flow::Cancel
    }

    /// A Yukari gap used: once in 2,000, Yukari comes through after the player.
    fn player_teleported(&self, host: &mut dyn Host, event: &mut PlayerTeleportedEvent<'_>) {
        let Some(level) = host.level(event.dimension) else { return };
        let to = event.to.map(|c| c.floor() as i32);
        let by_gap = (-2..=2).any(|dx| {
            (-1..=3).any(|dy| {
                (-2..=2).any(|dz| {
                    host.level_block(level, [to[0] + dx, to[1] + dy, to[2] + dz])
                        .is_some_and(|b| b.starts_with("yukari_gap:yukari_gap"))
                })
            })
        });
        if !by_gap {
            return;
        }
        let roll = SERVER.lock().map_or(1, |mut s| s.rng.int(YUKARI_FROM_GAP));
        if roll != 0 {
            return;
        }
        let Some(yukari) = mobs::MOBS.iter().find(|m| m.id == "yukari") else { return };
        let at = [event.to[0] + 2.0, event.to[1] + 0.5, event.to[2] + 2.0];
        let dimension = event.dimension.to_owned();
        summon(host, &dimension, yukari, at);
    }

    fn level_tick_post(&self, host: &mut dyn Host, event: &mut LevelTickPostEvent<'_>) {
        let batch = match SERVER.lock() {
            Ok(mut server) => {
                server.spawn(host, event.dimension);
                server.tick(host, event.dimension)
            }
            Err(_) => return,
        };
        if batch.fired.is_empty() && batch.gone.is_empty() {
            return;
        }
        let messages = batch.encode(30_000);
        for p in host.server_players() {
            if p.dimension == event.dimension {
                for m in &messages {
                    let _ = host.net_send_to_player(p.uuid, CHANNEL, m);
                }
            }
        }
    }

    fn net_from_server(&self, _host: &mut dyn Host, event: &mut NetFromServerEvent<'_>) {
        if event.channel != CHANNEL {
            return;
        }
        let batch = Batch::decode(event.data);
        if let Ok(mut shots) = CLIENT.lock() {
            for id in batch.gone {
                shots.remove(&id);
            }
            for s in batch.fired {
                shots.insert(s.id, s);
            }
        }
    }

    fn client_tick_post(&self, _host: &mut dyn Host, _event: &mut ClientTickPostEvent) {
        if let Ok(mut shots) = CLIENT.lock() {
            shots.retain(|_, s| s.step());
        }
    }

    fn client_level_unload(&self, _host: &mut dyn Host, _event: &mut ClientLevelUnloadEvent<'_>) {
        if let Ok(mut shots) = CLIENT.lock() {
            shots.clear();
        }
    }

    fn world_submit(&self, host: &dyn Host, event: &mut WorldSubmitEvent<'_>) {
        let Ok(shots) = CLIENT.lock() else { return };
        let Some(camera) = host.camera() else { return };
        let yaw = f64::from(camera.yaw).to_radians();
        let forward = angle(camera.yaw, camera.pitch);
        let right = v3(-yaw.cos(), 0.0, -yaw.sin());
        let up = vec::cross(right, forward);
        for s in shots.values() {
            draw_shot(host, event.draw, s, right, up);
        }
        draw_glows(host, event.draw, right, up);
    }

    /// Ours are drawn with their own models and textures, posed as the original posed them; the
    /// phantoms and familiars as a glow facing the camera.
    fn mob_submit(&self, host: &dyn Host, event: &mut MobSubmitEvent<'_>) -> Flow {
        let Some(spec) = mobs::by_kind(event.mob.kind) else { return Flow::Continue };
        match spec.look {
            Look::Model(model, _) => {
                let Ok(mut baked) = BAKED.lock() else { return Flow::Continue };
                if !baked.contains_key(model) {
                    let Some(part) = host.bake_layer(&model::layer_id(model)) else { return Flow::Continue };
                    baked.insert(model, part);
                }
                let Some(root) = baked.get_mut(model) else { return Flow::Continue };
                root.reset_all_poses();
                pose(root, model, event.mob);
                let texture = mobs::texture_of(spec, event.mob.id);
                let draw = &mut *event.draw;
                draw.push();
                draw.scale([1.0, -1.0, -1.0]);
                draw.translate([0.0, -1.501, 0.0]);
                host.draw_model_part(draw, root, &format!("thmobs:entity/{texture}"), 0xFF_FF_FF);
                draw.pop();
            }
            Look::Vanilla(layer, sprite) => {
                let Ok(mut baked) = BAKED.lock() else { return Flow::Continue };
                if !baked.contains_key(layer) {
                    let Some(part) = host.bake_layer(layer) else { return Flow::Continue };
                    baked.insert(layer, part);
                }
                let Some(root) = baked.get_mut(layer) else { return Flow::Continue };
                root.reset_all_poses();
                set(root, "head", |p| {
                    p.y_rot = event.mob.head_yaw.to_radians();
                    p.x_rot = event.mob.pitch.to_radians();
                });
                let swing = (event.mob.walk_position * 0.6662).cos() * 1.4 * event.mob.walk_speed;
                for (name, sign) in [("right_hind_leg", 1.0), ("left_front_leg", 1.0), ("left_hind_leg", -1.0), ("right_front_leg", -1.0)] {
                    set(root, name, |p| p.x_rot = swing * sign);
                }
                let draw = &mut *event.draw;
                draw.push();
                draw.scale([1.0, -1.0, -1.0]);
                draw.translate([0.0, -1.501, 0.0]);
                host.draw_model_part(draw, root, sprite, 0xFF_FF_FF);
                draw.pop();
            }
            // Drawn with the danmaku in `world_submit`, facing the camera; nothing of the trader here.
            Look::Glow(..) => {}
        }
        Flow::Cancel
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn day_is_day_and_night_is_night() {
        assert!(daytime(1000));
        assert!(daytime(23_800));
        assert!(!daytime(18_000));
    }

    #[test]
    fn only_our_kinds_are_ours() {
        assert!(is_ours("thmobs:fairy"));
        assert!(!is_ours("minecraft:wandering_trader"));
    }

    /// `mobs.json` beside the crate: what `gen_content.py` needs of each mob.
    #[test]
    fn write_the_manifest() {
        let mut rows = Vec::new();
        for m in mobs::MOBS {
            let (template, look) = match m.look {
                Look::Model(model, textures) => ("minecraft:wandering_trader", format!("{{\"model\": \"{model}\", \"textures\": {textures:?}}}")),
                Look::Glow(sprite, _) => ("minecraft:wandering_trader", format!("{{\"glow\": \"{sprite}\"}}")),
                Look::Vanilla(..) => ("minecraft:wandering_trader", "{}".to_owned()),
            };
            rows.push(format!(
                "{{\"id\": \"{}\", \"name\": \"{}\", \"template\": \"{template}\", \"max_health\": {}, \"speed\": {}, \"look\": {look}, \"egg\": [{}, {}]}}",
                m.id, m.name, m.max_health(), m.ai.speed.min(1.0), m.egg.0, m.egg.1
            ));
        }
        let json = format!("[\n  {}\n]\n", rows.join(",\n  "));
        std::fs::write(concat!(env!("CARGO_MANIFEST_DIR"), "/../../mobs.json"), json).unwrap();
    }

    #[test]
    fn every_mob_attacks_without_trouble() {
        for spec in mobs::MOBS {
            for &phase in spec.start.iter().chain(spec.phases.iter().map(|p| &p.to)) {
                let mut out = Vec::new();
                let mut rng = Rng::new(5);
                let mut mem = [0; 4];
                for c in 0..300 {
                    let mut a = attack::Atk {
                        c,
                        level: ai::level(),
                        phase,
                        pos: v3(0.0, 65.0, 0.0),
                        feet: v3(0.0, 64.0, 0.0),
                        yaw: 30.0,
                        pitch: 5.0,
                        ticks: c,
                        target: Some(v3(0.0, 65.0, 10.0)),
                        fire: Fire { out: &mut out, rng: &mut rng },
                        span: 30,
                        pattern: mobs::FAIRY_PATTERNS[1][c as usize % 9],
                        glide: None,
                        stop: None,
                        card: None,
                        sound: false,
                        wet: c % 2 == 0,
                        mem: &mut mem,
                    };
                    (spec.attack)(&mut a);
                }
                assert!(out.iter().all(|s| s.pos.length().is_finite() && s.motion.length().is_finite()), "{} {phase}", spec.id);
            }
        }
    }
}
