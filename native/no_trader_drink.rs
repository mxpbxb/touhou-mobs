// Before a wandering trader takes up its potion or milk: a Touhou mob (`thmobs:`) never does.
fn no_trader_drink(m: &crate::mobs::Mob) -> core::ops::ControlFlow<bool> {
    if m.kind.id.starts_with("thmobs:") {
        core::ops::ControlFlow::Break(false)
    } else {
        core::ops::ControlFlow::Continue(())
    }
}
