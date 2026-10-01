use std::{collections::HashSet, sync::OnceLock};
use engage::app::{
    goddata::IGodDataMethods,
    godunit::{GodUnit, IGodUnitMethods},
    persondata::IPersonDataMethods,
    unit::{Unit, IUnitMethods},
};
use unity::prelude::*;
use engage::app::basicmenuitem::BasicMenuItem_Attribute;
use engage::app::unitring::UnitRing;

fn rules() -> &'static HashSet<(String, String)> {
    static RULES: OnceLock<HashSet<(String, String)>> = OnceLock::new();
    RULES.get_or_init(|| {
        let mut rules = HashSet::new();
        for path in mods::list_recursive("patches/god_equip_restrictions") {
            if path.extension() != Some("txt") { continue; }
            // Union all layers even when independent mods choose the same name.
            match mods::read_layers(path) {
                Ok(layers) => for bytes in layers {
                    match std::str::from_utf8(&bytes) {
                        Ok(text) => {
                            let invalid = crate::god_equip_rules::parse_rules(text, &mut rules);
                            if invalid != 0 { println!("[GodEquip] {path}: {invalid} invalid rule lines ignored"); }
                        }
                        Err(error) => println!("[GodEquip] {path}: invalid UTF-8: {error}"),
                    }
                },
                Err(error) => println!("[GodEquip] {path}: {error}"),
            }
        }
        crate::checkpoint(&format!("[GodEquip v4] Loaded {} restrictions\n", rules.len()));
        rules
    })
}

fn is_same_character_pair(unit: Unit, god: GodUnit) -> bool {
    if unit.is_null() || god.is_null() { return false; }
    let person = unit.get_person();
    let data = god.get_data();
    if person.is_null() || data.is_null() { return false; }
    rules().contains(&(person.get_pid().to_string(), data.get_gid().to_string()))
}

// Engage 2.0.0 offset from the tagged official Unit binding.
#[skyline::hook(offset = 0x1a2b6e0)]
pub fn can_god_equip(unit: Unit, god: GodUnit, method_info: OptionalMethod) -> bool {
    // Keep every native denial, including Alear's special restrictions.
    use std::sync::atomic::{AtomicUsize, Ordering};
    static CALLS: AtomicUsize = AtomicUsize::new(0);
    if CALLS.fetch_add(1, Ordering::Relaxed) < 8 { crate::checkpoint("[GodEquip v4] native check called\n"); }
    let allowed = call_original!(unit, god, method_info);
    if !allowed || unit.is_null() || god.is_null() { return allowed; }
    !is_same_character_pair(unit, god)
}

// The ring selection screen computes its disabled state here, independently of
// Unit::CanGodEquip. Preserve every native disabled reason, then apply our IDs.
#[skyline::hook(offset = 0x1d60e90)]
pub fn ring_select_attribute(unit: Unit, god: GodUnit, ring: UnitRing, method_info: OptionalMethod) -> BasicMenuItem_Attribute {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static CALLS: AtomicUsize = AtomicUsize::new(0);
    if CALLS.fetch_add(1, Ordering::Relaxed) < 8 { crate::checkpoint("[GodEquip v4] ring attribute called\n"); }
    let native = call_original!(unit, god, ring, method_info);
    if native.value != BasicMenuItem_Attribute::enable().value { return native; }
    if is_same_character_pair(unit, god) { BasicMenuItem_Attribute::disable() } else { native }
}
