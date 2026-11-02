//! What a frame exposes, and what it withholds.

use super::*;

#[test]
fn model_snapshot_exposes_only_declared_behavior_identities() {
    let catalog = PresetModelCatalog::open(
        repository_root().join("resources/models"),
        ModelPackageLimits::default(),
    )
    .expect("preset catalog");
    let snapshot = catalog
        .load(&ModelId::parse("standard").expect("model id"))
        .expect("committed preset")
        .snapshot();

    // Four motions in each of two groups, plus seven expressions. The standard
    // model rigs only the eye-open parameters and the two head angles, so the
    // mouth, brow, cheek and roll behaviours are not declared: a declared
    // behaviour the moc3 cannot draw is a dead palette chip, which is exactly
    // what the user reported before these four were dropped.
    assert_eq!(snapshot.behaviors.len(), 15);
    for absent in ["smile.exp3.json", "surprised.exp3.json", "angry.exp3.json"] {
        assert!(
            !snapshot
                .behaviors
                .contains(&ModelBehaviorSnapshot::Expression {
                    name: absent.to_owned(),
                }),
            "{absent} declares parameters the standard model does not rig"
        );
    }
    assert!(snapshot.behaviors.contains(&ModelBehaviorSnapshot::Motion {
        group: "CAT_motion".to_owned(),
        index: 0,
    }));
    assert!(snapshot.behaviors.contains(&ModelBehaviorSnapshot::Motion {
        group: "CAT_motion".to_owned(),
        index: 2,
    }));
    assert!(
        snapshot
            .behaviors
            .contains(&ModelBehaviorSnapshot::Expression {
                name: "live2d_expression2.exp3.json".to_owned(),
            })
    );
    assert!(
        snapshot
            .behaviors
            .contains(&ModelBehaviorSnapshot::Expression {
                name: "wink.exp3.json".to_owned(),
            })
    );
    assert!(snapshot.behaviors.iter().all(|behavior| match behavior {
        ModelBehaviorSnapshot::Motion { group, .. } =>
            !group.contains('/') && !group.contains('\\'),
        ModelBehaviorSnapshot::Expression { name } => !name.contains('/') && !name.contains('\\'),
    }));
}
