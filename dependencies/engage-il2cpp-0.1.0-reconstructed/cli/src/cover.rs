use std::collections::BTreeSet;

use crate::manifest::Manifest;

pub fn minimum_cover(required: &BTreeSet<String>, manifest: &Manifest) -> BTreeSet<String> {
    let mut keep: BTreeSet<String> = required.iter().cloned().collect();
    let mut changed = true;

    while changed {
        changed = false;

        let candidates: Vec<String> = keep.iter().cloned().collect();

        'outer: for f in candidates {
            for other in &keep {
                if other == &f {
                    continue;
                }

                let other_closure = manifest.closure(other);

                if other_closure.contains(&f) {
                    keep.remove(&f);
                    changed = true;
                    continue 'outer;
                }
            }
        }
    }

    keep
}
