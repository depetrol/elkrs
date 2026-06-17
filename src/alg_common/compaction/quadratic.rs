
use super::compare_fuzzy;
use super::one_dimensional_compactor::OneDimensionalCompactor;

/// Creates a constraint between CNodes A and B if B collides with the right
/// shadow of A considering vertical spacing.
pub fn quadratic_constraints(compactor: &mut OneDimensionalCompactor) {
    // resetting constraints
    for n in &mut compactor.cgraph.cnodes {
        n.constraints.clear();
    }

    let count = compactor.cgraph.cnodes.len();
    let horizontal = compactor.direction.is_horizontal();

    for i in 0..count {
        for j in 0..count {
            if i == j {
                continue;
            }
            // no constraints between nodes of the same group
            let gi = compactor.cgraph.cnodes[i].cgroup;
            let gj = compactor.cgraph.cnodes[j].cgroup;
            if gi.is_some() && gi == gj {
                continue;
            }

            let spacing = if horizontal {
                compactor.spacings_handler.vertical_spacing(&compactor.cgraph, i, j)
            } else {
                compactor.spacings_handler.horizontal_spacing(&compactor.cgraph, i, j)
            };

            let hb1 = compactor.cgraph.cnodes[i].hitbox;
            let hb2 = compactor.cgraph.cnodes[j].hitbox;

            if ((hb2.x > hb1.x) || (hb1.x == hb2.x && hb1.width < hb2.width))
                && compare_fuzzy::gt(hb2.y + hb2.height + spacing, hb1.y)
                && compare_fuzzy::lt(hb2.y, hb1.y + hb1.height + spacing)
            {
                compactor.cgraph.cnodes[i].constraints.push(j);
            }
        }
    }
}
