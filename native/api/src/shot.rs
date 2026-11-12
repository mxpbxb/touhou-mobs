//! A danmaku bullet: `EntityTHShot`'s state and its tick, without the entity. The server owns every
//! shot and settles what it hits; a client is told each shot once, when it is fired, and runs the
//! same motion itself to draw it, so a pattern of hundreds costs one message, not hundreds of entities.

use crate::vec::{V3, rotate, v3};

/// `THShotLib.SIZE`: each form's diameter, in blocks.
pub const SIZE: [f32; 32] = [
    0.3, 0.15, 0.5, 0.3, 0.3, 0.3, 0.15, 0.3, 0.25, 0.5, 0.15, 0.15, 0.5, 0.15, 0.15, 0.6, 0.5, 1.0, 0.15, 0.0,
    0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.4, 0.3, 1.0, 0.9, 0.9,
];

/// `THShotLib.DAMAGE`: each form's damage.
pub const DAMAGE: [f32; 32] = [
    2.4, 1.0, 2.8, 5.0, 2.0, 3.4, 1.6, 2.8, 1.6, 3.0, 1.2, 1.8, 3.2, 2.6, 1.0, 6.8, 3.0, 1.0, 3.0, 0.0, 0.0,
    0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 5.0, 2.6, 4.0, 5.0, 5.0,
];

/// The forms, by `DanmakuConstants`' numbers; the rest are drawn and fired as the engine grows.
#[allow(dead_code)]
pub mod form {
    pub const SMALL: u8 = 0;
    pub const TINY: u8 = 1;
    pub const MEDIUM: u8 = 2;
    pub const PEARL: u8 = 3;
    pub const CIRCLE: u8 = 4;
    pub const LIGHT: u8 = 5;
    pub const SCALE: u8 = 6;
    pub const BUTTERFLY: u8 = 7;
    pub const SMALL_STAR: u8 = 8;
    pub const STAR: u8 = 9;
    pub const RICE: u8 = 10;
    pub const CRYSTAL: u8 = 11;
    pub const HEART: u8 = 12;
    pub const KUNAI: u8 = 13;
    pub const TALISMAN: u8 = 14;
    pub const BIG_LIGHT: u8 = 15;
    pub const OVAL: u8 = 16;
    pub const FAMILIAR: u8 = 17;
    pub const ARROW: u8 = 18;
    pub const PHANTOM: u8 = 26;
    pub const AMULET: u8 = 27;
    pub const KNIFE: u8 = 28;
    pub const WIND: u8 = 29;
    pub const BIG: u8 = 30;
    pub const KISHITU: u8 = 31;
}

/// Colours 0 to 7, then the two that pick: `RANDOM` one of the eight, `RAINBOW` one of the first seven.
pub const RANDOM: u8 = 8;
pub const RAINBOW: u8 = 9;

/// Special movements, by `DanmakuConstants`' numbers and the spell cards that register their own.
pub mod special {
    /// Falls: two hundredths of a block a tick, every tick.
    pub const FALL: i32 = 30;
    /// Knocks what it hits up and away (Yasaka's Divine Wind).
    pub const WIND: i32 = 180;
    /// Perfect Freeze's frozen shot: still, then off again from its twentieth tick.
    pub const FREEZE: i32 = 900;
    /// Little Bug Storm's: at its end it turns 70 degrees and sets off again (3451 the other way).
    pub const BUG_LEFT: i32 = 3450;
    pub const BUG_RIGHT: i32 = 3451;
    /// Gagouji Cyclone's: at its end it turns at its shooter's target. The server does it.
    pub const SEEK_TARGET: i32 = 4020;
    /// Journey To Gensokyo's seed: at twenty ticks a fast thin light (its laser trail is to come).
    pub const SEED: i32 = 11000;
    /// Bad Fortune's: at ten ticks a fan and four rice shots (30609) or a ring of 36 (30610) burst
    /// from it; at forty it sets off; at a hundred it is gone. The server fires the bursts.
    pub const MISFORTUNE_FAN: i32 = 30609;
    pub const MISFORTUNE_RING: i32 = 30610;
    /// Bad Fortune's mark: at its end a ring of talismans along its shooter's look. The server fires it.
    pub const MISFORTUNE_MARK: i32 = 30611;
}

/// `ShotData`: what a pattern asks for.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShotData {
    pub form: u8,
    pub color: u8,
    pub size: f32,
    pub damage: f32,
    pub delay: i32,
    pub end: i32,
    pub special: i32,
}

/// `ShotData.shot(form, color, size, damage, delay, end, special)`.
pub fn shot_full(form: u8, color: u8, size: f32, damage: f32, delay: i32, end: i32, special: i32) -> ShotData {
    ShotData { form: form % 32, color: color % 16, size, damage, delay, end, special }
}

/// `ShotData.shot(form, color, delay, end, special)`, with the form's own size and damage.
pub fn shot(form: u8, color: u8, delay: i32, end: i32, special: i32) -> ShotData {
    let f = usize::from(form % 32);
    shot_full(form, color, SIZE[f], DAMAGE[f], delay, end, special)
}

#[derive(Clone, Debug, PartialEq)]
pub struct Shot {
    pub id: u32,
    pub pos: V3,
    /// Where it was a tick ago, to draw it between ticks.
    pub prev: V3,
    pub motion: V3,
    /// The way it faces: speeding up and slowing down go along this, gravity does not turn it.
    pub angle: V3,
    /// The axis it turns about, `rot_speed` degrees a tick, until tick `rot_end`.
    pub rotate: V3,
    pub rot_speed: f32,
    pub rot_end: i32,
    pub limit: f64,
    pub accel: f64,
    pub gravity: V3,
    pub form: u8,
    pub color: u8,
    pub size: f32,
    pub damage: f32,
    pub end: i32,
    pub special: i32,
    /// Ticks it waited before it first moved; it waits again if it is fired anew.
    pub delay: i32,
    /// Its tilt about its own course, in degrees, for the shapes that show one.
    pub slope: f32,
    /// Ticks since it was fired (`ticksExisted`).
    pub ticks: i32,
    /// `animationCount`: below zero while it waits out its delay, then its age.
    pub count: i32,
    last_time: i32,
}

impl Shot {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        pos: V3,
        angle: V3,
        slope: f32,
        rotate: V3,
        rot_speed: f32,
        rot_end: i32,
        first_speed: f64,
        limit: f64,
        accel: f64,
        gravity: V3,
        data: ShotData,
    ) -> Shot {
        Shot {
            id: 0,
            pos,
            prev: pos,
            motion: angle * first_speed,
            angle,
            rotate,
            rot_speed,
            rot_end,
            limit,
            accel,
            gravity,
            form: data.form,
            color: data.color,
            size: data.size,
            damage: data.damage,
            end: data.end,
            special: data.special,
            delay: data.delay,
            slope,
            ticks: 0,
            count: -data.delay,
            last_time: 0,
        }
    }

    pub fn speed(&self) -> f64 {
        self.motion.length()
    }

    /// Whether it can hit anything yet: not while it is still appearing.
    pub fn solid(&self) -> bool {
        self.count >= 1
    }

    /// One tick of `EntityTHShot.onUpdate`, without the hit test, which the server makes after it
    /// along the way the shot went. False once the shot has had its time.
    pub fn step(&mut self) -> bool {
        self.ticks += 1;
        self.prev = self.pos;
        if self.count > self.end {
            return false;
        }
        if matches!(self.special, special::MISFORTUNE_FAN | special::MISFORTUNE_RING) && self.ticks >= 100 {
            return false;
        }
        if self.count < 0 {
            self.count += 1;
            self.last_time = self.ticks;
            return true;
        }
        self.count = self.last_time;
        if self.rot_speed != 0.0 {
            self.angle = rotate(self.rotate, self.angle, self.rot_speed);
            self.motion = self.angle * self.speed();
        }
        if self.renew() {
            return true;
        }
        self.accelerate();
        self.special_motion();
        self.motion = self.motion + self.gravity;
        if self.rot_end < self.last_time {
            self.rot_speed = 0.0;
        }
        self.pos = self.pos + self.motion;
        self.last_time = self.ticks;
        true
    }

    /// `shotAcceleration`: towards `limit`, along the way it faces.
    fn accelerate(&mut self) {
        let a = self.angle;
        if self.accel > 0.0 && self.speed() < self.limit {
            self.motion = self.motion + a * self.accel;
            if self.speed() > self.limit {
                self.motion = a * self.limit;
            }
        } else if self.accel < 0.0 && self.speed() > self.limit {
            if self.accel.abs() > self.speed() - self.limit {
                self.motion = a * self.limit;
            } else {
                self.motion = self.motion + a * self.accel;
            }
        }
    }

    /// `reCreate`: fired anew from where it is, at `speed` along the way it faces.
    pub fn restart(&mut self, speed: f64) {
        self.motion = self.angle * speed;
        self.ticks = 0;
        self.count = -self.delay;
        self.last_time = 0;
    }

    /// The specials that end a shot by firing it anew from where it is (`reCreate`).
    fn renew(&mut self) -> bool {
        let turn = match self.special {
            special::BUG_LEFT => 70.0,
            special::BUG_RIGHT => -70.0,
            _ => return false,
        };
        if self.ticks != self.end {
            return false;
        }
        let speed = self.speed();
        self.special = 0;
        self.end = 80;
        self.angle = rotate(self.rotate, self.angle, turn);
        self.limit = 0.5;
        self.accel = 0.03;
        self.motion = self.angle * speed;
        self.ticks = 0;
        self.count = -self.delay;
        self.last_time = 0;
        true
    }

    fn special_motion(&mut self) {
        match self.special {
            special::FALL => self.motion.y -= 0.02,
            special::MISFORTUNE_FAN | special::MISFORTUNE_RING if self.ticks == 40 => {
                self.limit = 0.5;
                self.accel = 0.1;
            }
            // Night Bird's: at twenty ticks it turns square and sets off (the other way with end 181).
            32501 if self.ticks == 20 => {
                let turn = if self.end == 181 { -90.0 } else { 90.0 };
                self.limit = 0.2;
                self.accel = 0.1;
                self.angle = rotate(self.rotate, self.angle, turn);
            }
            special::SEED if self.ticks == 20 => {
                self.accel = 0.05;
                self.limit = 10.0;
                self.size = 0.1;
                self.form = form::LIGHT;
                self.motion = self.angle * self.speed();
            }
            special::FREEZE if self.ticks >= 20 => {
                self.limit = 0.3;
                self.accel = 0.03;
                self.special = 0;
            }
            _ => {}
        }
    }

    /// Its damage, as `getShotDamageWithDifficulty` gives a mob's: a player's shot does its own.
    pub fn damage_for(&self, by_player: bool, difficulty: &str) -> f32 {
        if by_player {
            return self.damage;
        }
        match difficulty {
            "easy" => self.damage * 0.7,
            "hard" => self.damage * 1.5,
            _ => self.damage,
        }
    }

    // ---- On the wire: a client is told a shot once, as it is fired ----

    pub const WIRE: usize = 4 + 3 * 8 + 3 * 4 * 5 + 4 * 8 + 2 + 4 * 2;

    pub fn encode(&self, out: &mut Vec<u8>) {
        out.extend(self.id.to_le_bytes());
        for v in self.pos.array() {
            out.extend(v.to_le_bytes());
        }
        for v in [self.motion, self.angle, self.rotate, self.gravity] {
            for c in v.array() {
                out.extend((c as f32).to_le_bytes());
            }
        }
        for v in [v3(self.limit, self.accel, 0.0)] {
            out.extend((v.x as f32).to_le_bytes());
            out.extend((v.y as f32).to_le_bytes());
            out.extend(0f32.to_le_bytes());
        }
        out.extend(self.rot_speed.to_le_bytes());
        out.extend(self.rot_end.to_le_bytes());
        out.extend(self.size.to_le_bytes());
        out.extend(self.slope.to_le_bytes());
        out.extend(self.end.to_le_bytes());
        out.extend(self.special.to_le_bytes());
        out.extend(self.ticks.to_le_bytes());
        out.extend(self.count.to_le_bytes());
        out.extend([self.form, self.color]);
        out.extend(self.last_time.to_le_bytes());
        out.extend(self.delay.to_le_bytes());
    }

    pub fn decode(data: &[u8]) -> Option<(Shot, &[u8])> {
        let mut r = Reader(data);
        let id = r.u32()?;
        let pos = v3(r.f64()?, r.f64()?, r.f64()?);
        let mut vecs = [V3::ZERO; 4];
        for v in &mut vecs {
            *v = v3(f64::from(r.f32()?), f64::from(r.f32()?), f64::from(r.f32()?));
        }
        let (limit, accel, _) = (f64::from(r.f32()?), f64::from(r.f32()?), r.f32()?);
        let rot_speed = r.f32()?;
        let rot_end = r.i32()?;
        let size = r.f32()?;
        let slope = r.f32()?;
        let end = r.i32()?;
        let special = r.i32()?;
        let ticks = r.i32()?;
        let count = r.i32()?;
        let form = r.u8()?;
        let color = r.u8()?;
        let last_time = r.i32()?;
        let delay = r.i32()?;
        let [motion, angle, rotate, gravity] = vecs;
        let shot = Shot {
            id,
            pos,
            prev: pos,
            motion,
            angle,
            rotate,
            rot_speed,
            rot_end,
            limit,
            accel,
            gravity,
            form,
            color,
            size,
            damage: 0.0,
            end,
            special,
            delay,
            slope,
            ticks,
            count,
            last_time,
        };
        Some((shot, r.0))
    }
}

struct Reader<'a>(&'a [u8]);

impl Reader<'_> {
    fn take<const N: usize>(&mut self) -> Option<[u8; N]> {
        let (head, rest) = self.0.split_first_chunk::<N>()?;
        self.0 = rest;
        Some(*head)
    }
    fn u8(&mut self) -> Option<u8> {
        self.take::<1>().map(|b| b[0])
    }
    fn u32(&mut self) -> Option<u32> {
        self.take().map(u32::from_le_bytes)
    }
    fn i32(&mut self) -> Option<i32> {
        self.take().map(i32::from_le_bytes)
    }
    fn f32(&mut self) -> Option<f32> {
        self.take().map(f32::from_le_bytes)
    }
    fn f64(&mut self) -> Option<f64> {
        self.take().map(f64::from_le_bytes)
    }
}

/// What the server tells clients: shots fired, and shots gone before their time.
#[derive(Debug, Default, PartialEq)]
pub struct Batch {
    pub fired: Vec<Shot>,
    pub gone: Vec<u32>,
}

impl Batch {
    /// Messages of at most `limit` bytes each.
    pub fn encode(&self, limit: usize) -> Vec<Vec<u8>> {
        let mut out = Vec::new();
        let mut message = vec![0u8];
        for shot in &self.fired {
            if message.len() + 1 + Shot::WIRE + 16 > limit {
                out.push(std::mem::replace(&mut message, vec![0u8]));
            }
            message.push(1);
            shot.encode(&mut message);
        }
        for id in &self.gone {
            if message.len() + 5 > limit {
                out.push(std::mem::replace(&mut message, vec![0u8]));
            }
            message.push(2);
            message.extend(id.to_le_bytes());
        }
        if message.len() > 1 {
            out.push(message);
        }
        out
    }

    pub fn decode(mut data: &[u8]) -> Batch {
        let mut batch = Batch::default();
        if data.first() != Some(&0) {
            return batch;
        }
        data = &data[1..];
        while let Some((&kind, rest)) = data.split_first() {
            match kind {
                1 => match Shot::decode(rest) {
                    Some((shot, rest)) => {
                        batch.fired.push(shot);
                        data = rest;
                    }
                    None => break,
                },
                2 => match rest.split_first_chunk::<4>() {
                    Some((id, rest)) => {
                        batch.gone.push(u32::from_le_bytes(*id));
                        data = rest;
                    }
                    None => break,
                },
                _ => break,
            }
        }
        batch
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vec::angle;

    fn plain(speed: f64, limit: f64, accel: f64, data: ShotData) -> Shot {
        Shot::new(V3::ZERO, angle(0.0, 0.0), 0.0, v3(0.0, 1.0, 0.0), 0.0, 9999, speed, limit, accel, V3::ZERO, data)
    }

    #[test]
    fn a_shot_waits_out_its_delay_then_flies() {
        let mut s = plain(0.5, 0.5, 0.0, shot(form::SMALL, 0, 3, 80, 0));
        for _ in 0..3 {
            assert!(s.step());
            assert_eq!(s.pos, V3::ZERO);
            assert!(!s.solid());
        }
        assert!(s.step());
        assert!((s.pos.z - 0.5).abs() < 1e-9);
    }

    #[test]
    fn it_speeds_up_to_its_limit_and_no_further() {
        let mut s = plain(0.2, 0.7, 0.1, shot(form::SMALL, 0, 0, 80, 0));
        for _ in 0..20 {
            s.step();
        }
        assert!((s.speed() - 0.7).abs() < 1e-6, "{}", s.speed());
    }

    #[test]
    fn it_is_gone_after_its_end() {
        let mut s = plain(0.1, 0.1, 0.0, shot(form::SMALL, 0, 0, 10, 0));
        let lived = (0..100).take_while(|_| s.step()).count();
        assert_eq!(lived, 12);
    }

    #[test]
    fn a_frozen_shot_sets_off_again_at_twenty_ticks() {
        let mut s = plain(1e-4, 1e-4, 0.0, shot_full(form::CIRCLE, 7, 0.3, 4.0, 0, 120, special::FREEZE));
        for _ in 0..19 {
            s.step();
        }
        assert!(s.speed() < 1e-3);
        for _ in 0..12 {
            s.step();
        }
        assert_eq!(s.special, 0);
        assert!((s.speed() - 0.3).abs() < 1e-6, "{}", s.speed());
    }

    #[test]
    fn a_turning_shot_keeps_its_speed() {
        let mut s =
            Shot::new(V3::ZERO, angle(0.0, 0.0), 0.0, v3(0.0, 1.0, 0.0), 10.0, 9999, 0.3, 0.3, 0.0, V3::ZERO, shot(0, 0, 0, 80, 0));
        for _ in 0..9 {
            s.step();
        }
        assert!((s.speed() - 0.3).abs() < 1e-6);
        assert!((s.angle.x - 1.0).abs() < 1e-6, "{:?}", s.angle);
    }

    #[test]
    fn shots_cross_the_wire_unchanged_enough_to_fly_the_same() {
        let mut s = plain(0.4, 0.9, 0.05, shot(form::STAR, 3, 2, 60, special::FALL));
        s.id = 77;
        s.step();
        let batch = Batch { fired: vec![s.clone()], gone: vec![5, 6] };
        let messages = batch.encode(30_000);
        assert_eq!(messages.len(), 1);
        let back = Batch::decode(&messages[0]);
        assert_eq!(back.gone, vec![5, 6]);
        let mut theirs = back.fired[0].clone();
        assert_eq!(theirs.id, 77);
        for _ in 0..40 {
            s.step();
            theirs.step();
        }
        assert!((s.pos - theirs.pos).length() < 1e-3, "{:?} {:?}", s.pos, theirs.pos);
    }

    #[test]
    fn a_big_batch_is_split_into_messages_that_fit() {
        let fired: Vec<Shot> = (0..1000).map(|i| {
            let mut s = plain(0.1, 0.1, 0.0, shot(0, 0, 0, 80, 0));
            s.id = i;
            s
        }).collect();
        let messages = Batch { fired, gone: vec![] }.encode(30_000);
        assert!(messages.iter().all(|m| m.len() <= 30_000));
        let total: usize = messages.iter().map(|m| Batch::decode(m).fired.len()).sum();
        assert_eq!(total, 1000);
    }
}
