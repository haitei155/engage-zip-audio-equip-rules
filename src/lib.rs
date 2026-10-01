mod audio_cache;
mod wwise;
mod god_equip;
mod god_equip_rules;

pub(crate) fn checkpoint(message: &str) {
    let _ = horizon_svc::output_debug_string(message);
}

#[skyline::main]
pub fn main() {
    checkpoint("[Cobalt ZIP] preparing ZIP audio before sound banks load\n");
    let ready = wwise::prepare();
    if ready > 0 {
        skyline::install_hooks!(wwise::fee_zip_audio_open_file);
        checkpoint("[Cobalt ZIP] native OpenFile hook installed\n");
    }
    skyline::install_hooks!(god_equip::can_god_equip);
    skyline::install_hooks!(god_equip::ring_select_attribute);
    checkpoint("[Cobalt ZIP] existing equip hooks installed; main returning\n");
}
