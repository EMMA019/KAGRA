# Session

Started from origin/master (94db280). Stayed on master.

Guard is WalkInput.dodge (Shift / C / K). Incoming hit while guarding writes dump name `block`, zero damage, no KO. Combo window COMBO_TIME after a landed hit; second hit in the window increments combo and writes dump name `combo`. coins stays hit count. Hitstun / KO / hurt flash unchanged. Attack / facing / stun / KO / guard / combo live in fight.rs. WorldPlay only dispatches. Ring dump is fight_hitstun_world.json.

