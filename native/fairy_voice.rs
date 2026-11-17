// The fairies and crows of Touhou Mobs (`thmobs:`) sound as Arcadian Dream's fairies do: a bat's
// ambient, hurt and death sounds, not the wandering trader's they are built on.
fn fairy_voice(m: &crate::mobs::Mob) -> core::ops::ControlFlow<[&'static str; 3]> {
    const FAIRIES: [&str; 9] = [
        "thmobs:fairy",
        "thmobs:sunflower_fairy",
        "thmobs:ice_fairy",
        "thmobs:nether_fairy",
        "thmobs:moon_fairy",
        "thmobs:moon_sunflower_fairy",
        "thmobs:crow_tengu",
        "thmobs:old_crow_tengu",
        "thmobs:hell_raven",
    ];
    if FAIRIES.contains(&m.kind.id.as_str()) {
        core::ops::ControlFlow::Break(["entity.bat.ambient", "entity.bat.hurt", "entity.bat.death"])
    } else {
        core::ops::ControlFlow::Continue(())
    }
}
