#[path = "app_barbarian/rig.rs"]
mod rig;
use rig::*;
use d2_client::bridge::{BridgeResource, FrameOutputs};

#[test]
fn probe() {
    let mut r = Rig::new(&[LEAP, WHIRLWIND]);
    r.leave_town();
    r.select_right(LEAP);
    r.right_click_point(5, 0);
    for i in 0..30 {
        r.step(1);
        let w = r.app.world();
        let outs = w.resource::<FrameOutputs>().0.clone();
        let b = w.resource::<BridgeResource>();
        let u = b.0.world().local().unwrap();
        println!("f{i} mode {} frame {} pos {:?} ticks {} outs {:?}", u.mode, u.frame, u.position, b.0.world().server_ticks, outs.iter().filter(|o| format!("{o:?}").contains("Skill")).collect::<Vec<_>>());
    }
}
