use super::*;
use qst_core::{TransformState, glam::Vec3};

#[test]
fn falling_box_reaches_floor_and_emits_collision() {
    let mut world = World::new();
    let box_entity = world
        .spawn((
            Transform::from_state(TransformState {
                translation: Vec3::new(0.0, 3.0, 0.0),
                ..TransformState::identity()
            }),
            BoxCollider {
                half_extents: [0.5; 3],
                dynamic: true,
            },
        ))
        .id();
    world.spawn((
        Transform::from_state(TransformState {
            translation: Vec3::new(0.0, -0.5, 0.0),
            ..TransformState::identity()
        }),
        BoxCollider {
            half_extents: [5.0, 0.5, 5.0],
            dynamic: false,
        },
    ));
    let mut physics = PhysicsWorld::new(PhysicsConfig::default());
    physics.sync_from_scene(&mut world);
    let mut collided = false;
    for _ in 0..120 {
        physics.step(&mut world);
        collided |= physics
            .drain_events()
            .any(|event| event.started && (event.a == box_entity || event.b == box_entity));
    }
    assert!(collided);
    assert!(
        world
            .get::<Transform>(box_entity)
            .unwrap()
            .current
            .translation
            .y
            < 0.6
    );
}

#[test]
fn physics_step_preserves_previous_transform_for_render_interpolation() {
    let mut world = World::new();
    let entity = world
        .spawn((
            Transform::from_state(TransformState {
                translation: Vec3::new(0.0, 3.0, 0.0),
                ..TransformState::identity()
            }),
            BoxCollider {
                half_extents: [0.5; 3],
                dynamic: true,
            },
        ))
        .id();
    let mut physics = PhysicsWorld::new(PhysicsConfig::default());
    physics.sync_from_scene(&mut world);
    world
        .get_mut::<Transform>(entity)
        .unwrap()
        .previous
        .translation
        .y = 5.0;
    physics.step(&mut world);
    let transform = world.get::<Transform>(entity).unwrap();
    assert_eq!(transform.previous.translation.y, 5.0);
    assert!(transform.current.translation.y < 3.0);
}
