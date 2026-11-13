//! `EntityDanmakuMob.onUpdate` as a goal: hover above the ground, find the player in range, face them,
//! keep at its attack distance, and run its attack while it can see them. A goal has no host, so what
//! it fires waits in `FIRED`, and the spell cards it declares in `CASTS`, for the level's tick.

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

use horizon_client::mobs::{Mob, Who, goals::{self, Cx, Goal}};

use crate::attack::Atk;
use crate::mobs::{self, Pattern};
use crate::pattern::Fire;
use crate::shot::Shot;
use crate::vec::{Rng, V3, v3, vec_from_angle};

/// Shots fired this tick, by their mob.
pub static FIRED: Mutex<Vec<(u32, Shot, bool)>> = Mutex::new(Vec::new());
/// Spell cards declared this tick: the mob, the card.
pub static CASTS: Mutex<Vec<(u32, u32)>> = Mutex::new(Vec::new());

/// A boss's phase as the death hook last set it, for its goal to take up.
#[derive(Clone, Copy, Debug)]
pub struct Boss {
    pub phase: i32,
    pub counter: Option<i32>,
    pub invincible: i32,
    pub ai: Option<mobs::Ai>,
}

pub static BOSSES: LazyLock<Mutex<HashMap<u32, Boss>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

/// The phase a mob is in, for the death hook: its start until a phase ends.
pub fn phase_of(id: u32, spec: &mobs::Mob) -> i32 {
    BOSSES.lock().ok().and_then(|b| b.get(&id).map(|b| b.phase)).unwrap_or_else(|| mobs::start_of(spec, id))
}

/// Whether the mob can't be hurt just now (a phase's first ticks).
pub fn invincible(id: u32) -> bool {
    BOSSES.lock().ok().and_then(|b| b.get(&id).map(|b| b.invincible > 0)).unwrap_or(false)
}

/// The Cursed Decoy Dolls about: every hostile mob within forty blocks goes for one.
pub static DECOYS: LazyLock<Mutex<std::collections::HashSet<u32>>> = LazyLock::new(Default::default);

fn decoy(id: u32) -> bool {
    DECOYS.lock().is_ok_and(|d| d.contains(&id))
}

/// `EntityCursedDecoyDoll`: it stands where it was put for nine seconds, drawing every hostile
/// mob near on itself (`inDecoyEffectedArea`), then is gone.
#[derive(Clone, Debug, Default)]
pub struct Decoy {
    ticks: i32,
}

impl Goal for Decoy {
    fn flags(&self) -> u8 {
        goals::MOVE | goals::LOOK | goals::JUMP
    }
    fn can_use(&mut self, _: &mut Mob, _: &mut Cx) -> bool {
        true
    }
    fn every_tick(&self) -> bool {
        true
    }
    fn tick(&mut self, m: &mut Mob, _: &mut Cx) {
        self.ticks += 1;
        m.velocity.x = 0.0;
        m.velocity.z = 0.0;
        if let Ok(mut d) = DECOYS.lock() {
            if self.ticks > 180 || m.health <= 0.0 {
                d.remove(&m.id);
                m.removed = self.ticks > 180;
            } else {
                d.insert(m.id);
            }
        }
    }
    fn box_clone(&self) -> Box<dyn Goal> {
        Box::new(self.clone())
    }
}

/// `EntityYin_yang_orb_Yukari`: it circles the player three blocks out (one while they sneak), at
/// their waist, a turn of 7.5 degrees a tick. Its lasers come with the lasers.
#[derive(Clone, Debug, Default)]
pub struct Orbit {
    turn: f32,
}

impl Goal for Orbit {
    fn flags(&self) -> u8 {
        goals::MOVE | goals::LOOK | goals::JUMP
    }
    fn can_use(&mut self, _: &mut Mob, _: &mut Cx) -> bool {
        true
    }
    fn every_tick(&self) -> bool {
        true
    }
    fn tick(&mut self, m: &mut Mob, cx: &mut Cx) {
        m.no_gravity = true;
        let Some(p) = cx.w.player.as_ref().filter(|p| p.alive) else {
            m.removed = true;
            return;
        };
        let look = p.look;
        let yaw = (-look[0]).atan2(look[2]).to_degrees() as f32;
        let pitch = (-look[1]).asin().to_degrees() as f32;
        let radius = if p.sneaking { 1.0 } else { 3.0 };
        let v = crate::vec::rotation_vector_from_angle(yaw, pitch, self.turn, radius);
        m.position.x = p.position.x + v.x;
        m.position.y = p.position.y + 0.8;
        m.position.z = p.position.z + v.z;
        m.velocity.x = 0.0;
        m.velocity.y = 0.0;
        m.velocity.z = 0.0;
        m.yaw = yaw;
        m.head_yaw = yaw;
        m.body_yaw = yaw;
        self.turn -= 7.5;
    }
    fn box_clone(&self) -> Box<dyn Goal> {
        Box::new(self.clone())
    }
}

/// Every mob of ours about, attacking or not: a creeper never keeps one as its target.
pub static OURS: LazyLock<Mutex<std::collections::HashSet<u32>>> = LazyLock::new(Default::default);

pub fn ours(id: u32) -> bool {
    OURS.lock().is_ok_and(|o| o.contains(&id))
}

/// A creeper's first target goal: one of our mobs as its target, for whatever reason (a shot that
/// hit it, say), is dropped at once, so no creeper ever goes off by one of them.
#[derive(Clone, Debug, Default)]
pub struct CreeperPeace;

impl Goal for CreeperPeace {
    fn flags(&self) -> u8 {
        0
    }
    fn can_use(&mut self, m: &mut Mob, _: &mut Cx) -> bool {
        if let Some(Who::Mob(id)) = m.ai.target
            && ours(id)
        {
            m.ai.target = None;
            m.ai.last_hurt_by = None;
        }
        false
    }
    fn box_clone(&self) -> Box<dyn Goal> {
        Box::new(self.clone())
    }
}

/// Our mobs that are attacking just now.
pub static ATTACKING: LazyLock<Mutex<std::collections::HashSet<u32>>> = LazyLock::new(Default::default);

/// A hostile mob's target goal: the nearest of our mobs that is attacking, within sixteen blocks
/// and in sight, ahead of the player.
#[derive(Clone, Debug, Default)]
pub struct TargetTouhou {
    target: Option<u32>,
}

fn attacking(id: u32) -> bool {
    ATTACKING.lock().is_ok_and(|a| a.contains(&id))
}

impl Goal for TargetTouhou {
    fn flags(&self) -> u8 {
        goals::TARGET
    }
    fn can_use(&mut self, m: &mut Mob, cx: &mut Cx) -> bool {
        let (x, y, z) = (m.position.x, m.position.y, m.position.z);
        let lure = cx
            .w
            .others
            .iter()
            .filter(|s| s.alive && decoy(s.id))
            .map(|s| (s.id, (s.position.x - x).powi(2) + (s.position.y - y).powi(2) + (s.position.z - z).powi(2)))
            .filter(|(_, d)| *d <= 1600.0)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(id, _)| id);
        if lure.is_some() {
            self.target = lure;
            return true;
        }
        let near = cx
            .w
            .others
            .iter()
            .filter(|s| s.alive && attacking(s.id))
            .map(|s| (s.id, (s.position.x - x).powi(2) + (s.position.y - y).powi(2) + (s.position.z - z).powi(2)))
            .filter(|(_, d)| *d <= 256.0)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(id, _)| id);
        self.target = near.filter(|id| m.sees(cx.w, Who::Mob(*id)));
        self.target.is_some()
    }
    fn can_continue(&mut self, _: &mut Mob, cx: &mut Cx) -> bool {
        self.target.is_some_and(|id| (attacking(id) || decoy(id)) && cx.w.seen(id).is_some_and(|s| s.alive))
    }
    fn start(&mut self, m: &mut Mob, _: &mut Cx) {
        m.ai.target = self.target.map(Who::Mob);
    }
    fn stop(&mut self, m: &mut Mob, _: &mut Cx) {
        if m.ai.target == self.target.map(Who::Mob) {
            m.ai.target = None;
        }
        self.target = None;
    }
    fn box_clone(&self) -> Box<dyn Goal> {
        Box::new(self.clone())
    }
}

/// How much of the original's rate of fire the mobs keep: their attacks, and the spell cards they
/// declare, advance on this share of ticks, so every pattern keeps its shape, only slower.
pub const PACE: f32 = 0.35;

const DESPAWN_FAR: f64 = 128.0;
const DESPAWN_NEAR: f64 = 32.0;
/// `THKaguyaConfig.danmakuLevel`: 1 easy, 2 normal, 3 hard, 4 lunatic. Miko changes it.
pub static LEVEL: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(2);

pub fn level() -> i32 {
    LEVEL.load(std::sync::atomic::Ordering::Relaxed)
}

#[derive(Clone, Debug)]
pub struct Danmaku {
    spec: &'static mobs::Mob,
    ai: mobs::Ai,
    phase: i32,
    pattern: Pattern,
    span: i32,
    target: bool,
    counter: i32,
    lost_target: i32,
    move_vec: V3,
    move_timer: i32,
    move_timer_max: i32,
    stop: bool,
    mem: [i32; 4],
    rng: Rng,
    /// Toward the next tick its attack advances on (`PACE`).
    pace: f32,
    /// A mob, not one of ours, that hurt it: it fights that back first.
    revenge: Option<u32>,
}

/// Whom one of our mobs fights: the player, or a mob that hurt it.
struct Foe {
    who: Who,
    at: V3,
    eye: f64,
    middle: f64,
}

/// How far one of our mobs goes after a mob that hurt it.
const REVENGE_RANGE: f64 = 40.0;

impl Danmaku {
    pub fn new(spec: &'static mobs::Mob, id: u32) -> Danmaku {
        let mut rng = Rng::new(u64::from(id).wrapping_mul(0x2545_F491_4F6C_DD1D) ^ 0x7A11);
        let level = &mobs::FAIRY_PATTERNS[(level() - 1) as usize];
        let mut pattern = level[rng.int(level.len() as u32) as usize];
        if spec.id == "ice_fairy" {
            pattern.shot = pattern.shot / 8 * 8 + 5;
        }
        let phase = mobs::start_of(spec, id);
        let ai = spec.start_ai.iter().find(|(p, _)| *p == phase).map_or(spec.ai, |(_, a)| *a);
        Danmaku {
            spec,
            ai,
            phase,
            pattern,
            span: pattern.span,
            target: false,
            counter: 0,
            lost_target: 0,
            move_vec: V3::ZERO,
            move_timer: 0,
            move_timer_max: 0,
            stop: false,
            mem: [0; 4],
            rng,
            pace: 0.0,
            revenge: None,
        }
    }

    fn glide(&mut self, dir: V3, speed: f64, time: i32) {
        self.move_vec = dir * speed;
        self.move_timer = time;
        self.move_timer_max = time;
    }

    /// Take up a phase the death hook set.
    fn follow_boss(&mut self, id: u32) {
        let Ok(mut bosses) = BOSSES.lock() else { return };
        let Some(b) = bosses.get_mut(&id) else { return };
        if b.invincible > 0 {
            b.invincible -= 1;
        }
        if b.phase != self.phase {
            self.phase = b.phase;
            self.stop = false;
        }
        if let Some(c) = b.counter.take() {
            self.counter = c;
        }
        if let Some(a) = b.ai.take() {
            self.ai = a;
        }
    }
}

fn air(cx: &Cx, x: i32, y: i32, z: i32) -> bool {
    let state = cx.w.world.block(x, y, z);
    cx.w.facts.get(state).name.ends_with("air")
}

/// `getPosYFromEye`.
fn shot_y(eye_y: f64, pitch: f32) -> f64 {
    let p = f64::from(pitch).to_radians();
    eye_y + p.sin() * -0.5 + p.cos() * -0.5
}

impl Goal for Danmaku {
    fn flags(&self) -> u8 {
        goals::MOVE | goals::LOOK | goals::JUMP
    }

    fn can_use(&mut self, m: &mut Mob, _: &mut Cx) -> bool {
        m.health > 0.0
    }

    fn every_tick(&self) -> bool {
        true
    }

    fn tick(&mut self, m: &mut Mob, cx: &mut Cx) {
        if m.health <= 0.0 {
            return;
        }
        self.follow_boss(m.id);
        if let Ok(mut o) = OURS.lock() {
            o.insert(m.id);
        }
        if !m.persistent
            && let Some(p) = cx.w.nearest_player
        {
            let d = ((p.x - m.position.x).powi(2) + (p.y - m.position.y).powi(2) + (p.z - m.position.z).powi(2)).sqrt();
            if d > DESPAWN_FAR || (d > DESPAWN_NEAR && self.rng.int(800) == 0) {
                m.removed = true;
                return;
            }
        }
        let (x, y, z) = (m.position.x.floor() as i32, m.position.y.floor() as i32, m.position.z.floor() as i32);
        // A phantom fades in the sun.
        if self.spec.burns && cx.w.sky_darken() == 0 && cx.w.can_see_sky(x, y + 1, z) && self.rng.float() * 30.0 < 1.2 {
            m.health = 0.0;
            return;
        }
        m.no_gravity = self.ai.flying_height > 0;

        if self.move_timer > 0 {
            let rate = (f64::from(self.move_timer) / f64::from(self.move_timer_max) * std::f64::consts::FRAC_PI_2).cos();
            m.velocity.x = self.move_vec.x * rate;
            m.velocity.z = self.move_vec.z * rate;
            m.velocity.y += self.move_vec.y * rate * 0.1;
            self.move_timer -= 1;
        } else {
            m.velocity.x *= 0.5;
            m.velocity.z *= 0.5;
        }
        if self.stop {
            m.velocity.x = 0.0;
            m.velocity.y = 0.0;
            m.velocity.z = 0.0;
        }

        let player = cx.w.player.as_ref().filter(|p| p.alive && !p.spectator);
        if m.ai.last_hurt_by == Some(Who::Player) && player.is_some() {
            self.target = true;
        }
        // A vanilla mob (anything not ours) that hurts it, it turns on, ahead of the player.
        if let Some(Who::Mob(id)) = m.ai.last_hurt_by
            && !ours(id)
            && self.revenge != Some(id)
        {
            self.revenge = Some(id);
        }
        let (mx, my, mz) = (m.position.x, m.position.y, m.position.z);
        let revenge = self.revenge.and_then(|id| cx.w.seen(id)).filter(|s| {
            s.alive && ((s.position.x - mx).powi(2) + (s.position.y - my).powi(2) + (s.position.z - mz).powi(2)).sqrt() <= REVENGE_RANGE
        });
        let revenge = revenge.map(|s| Foe {
            who: Who::Mob(s.id),
            at: v3(s.position.x, s.position.y, s.position.z),
            eye: s.eye_y,
            middle: (s.bounds.min_position().y + s.bounds.max_position().y) / 2.0,
        });
        if revenge.is_none() {
            if self.revenge.is_some() {
                m.ai.last_hurt_by = None;
            }
            self.revenge = None;
        }
        if let Some(p) = player
            && !self.target
        {
            let d = ((p.position.x - m.position.x).powi(2) + (p.position.y - m.position.y).powi(2) + (p.position.z - m.position.z).powi(2)).sqrt();
            if d <= self.ai.detection_distance {
                self.target = true;
            }
        }
        if player.is_none() {
            self.target = false;
        }
        // While it fights it holds the player as its target, so the dolls and hostile mobs know it.
        m.ai.target = match &revenge {
            Some(f) => Some(f.who),
            None => self.target.then_some(Who::Player),
        };
        if let Ok(mut attacking) = ATTACKING.lock() {
            if self.target || revenge.is_some() {
                attacking.insert(m.id);
            } else {
                attacking.remove(&m.id);
            }
        }

        if self.ai.flying_height > 0 && !self.stop {
            let mut height = 0;
            while height < 8 && air(cx, x, y - height, z) {
                height += 1;
            }
            let foe_y = revenge.as_ref().map(|f| f.at.y).or_else(|| player.filter(|_| self.target).map(|p| p.position.y));
            if let Some(foe_y) = foe_y
                && height > self.ai.flying_height
            {
                m.velocity.y += (foe_y - m.position.y) * 6.0e-4;
            } else if height < self.ai.flying_height {
                m.velocity.y += 0.006;
            }
        }

        let foe = revenge.or_else(|| {
            player.filter(|_| self.target).map(|p| Foe {
                who: Who::Player,
                at: v3(p.position.x, p.position.y, p.position.z),
                eye: p.eye[1],
                middle: (p.bounds.min_position().y + p.bounds.max_position().y) / 2.0,
            })
        });
        let Some(p) = foe else { return };
        let eye = m.eye_y();
        let (dx, dy, dz) = (p.at.x - m.position.x, p.eye - eye, p.at.z - m.position.z);
        let yaw = (-(dx.atan2(dz)) as f32).to_degrees();
        let pitch = (-(dy.atan2((dx * dx + dz * dz).sqrt())) as f32).to_degrees();
        if !self.stop {
            m.yaw = yaw;
            m.body_yaw = yaw;
            m.head_yaw = yaw;
            m.head_pitch = pitch;
        }

        // The attack goes on only on its share of ticks.
        self.pace += PACE;
        let advance = self.pace >= 1.0;
        if advance {
            self.pace -= 1.0;
        }
        let sees = m.sees(cx.w, p.who);
        if sees && !advance {
            self.lost_target = 0;
        } else if sees {
            self.lost_target = 0;
            let wet = {
                let state = cx.w.world.block(x, y, z);
                cx.w.facts.get(state).name.contains("water")
            };
            if self.counter >= 0 && self.phase != 127 {
                let mut fired = Vec::new();
                let mut atk = Atk {
                    c: self.counter,
                    level: level(),
                    phase: self.phase,
                    pos: v3(m.position.x, shot_y(eye, m.head_pitch), m.position.z),
                    feet: v3(m.position.x, m.position.y, m.position.z),
                    yaw: m.yaw,
                    pitch: m.head_pitch,
                    ticks: m.age as i32,
                    target: Some(v3(p.at.x, p.middle, p.at.z)),
                    fire: Fire { out: &mut fired, rng: &mut self.rng },
                    span: self.span,
                    pattern: self.pattern,
                    glide: None,
                    stop: None,
                    card: None,
                    sound: false,
                    wet,
                    mem: &mut self.mem,
                };
                (self.spec.attack)(&mut atk);
                let (c, span, glide, stop, card, sound) = (atk.c, atk.span, atk.glide, atk.stop, atk.card, atk.sound);
                self.counter = c;
                self.span = span;
                if let Some((dir, speed, time)) = glide {
                    self.glide(dir, speed, time);
                }
                if let Some(s) = stop {
                    self.stop = s;
                }
                if let Some(card) = card
                    && let Ok(mut q) = CASTS.lock()
                {
                    q.push((m.id, card));
                }
                if !fired.is_empty()
                    && let Ok(mut q) = FIRED.lock()
                {
                    let mut first = sound;
                    for s in fired {
                        q.push((m.id, s, std::mem::take(&mut first)));
                    }
                }
            }
            self.counter += 1;
            if self.spec.fairy_counter && self.counter > self.span {
                self.counter = 0;
            }
        } else {
            self.lost_target += 1;
            if self.lost_target > 200 {
                self.target = false;
            }
        }

        if self.stop {
            return;
        }
        if self.move_timer <= 0 && self.lost_target > 0 {
            let dir = vec_from_angle(self.rng.float() + 360.0, self.rng.float() * 360.0, 1.0);
            self.glide(dir, self.ai.speed, 30);
        }
        if self.move_timer <= 0 && self.ai.attack_distance > 0.0 {
            let d = (dx * dx + (p.at.y - m.position.y).powi(2) + dz * dz).sqrt();
            let a = self.ai.attack_distance;
            if d < a {
                self.glide(vec_from_angle(yaw + 180.0, pitch, 1.0), self.ai.speed * 0.3 * (1.0 - d / a), 15);
            } else {
                self.glide(vec_from_angle(yaw, pitch, 1.0), self.ai.speed * 0.5 * (d / a), 15);
            }
        }
    }

    fn box_clone(&self) -> Box<dyn Goal> {
        Box::new(self.clone())
    }
}
