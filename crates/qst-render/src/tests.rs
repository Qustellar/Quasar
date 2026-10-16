use super::*;

#[test]
fn snapshot_groups_instances_by_mesh_and_material() {
    for count in [1024_usize, 10_000_usize] {
        let mut snapshot = RenderSnapshot::default();
        for entity in 0..count {
            snapshot.instances.push(RenderInstance {
                entity: entity as u64,
                transform: TransformState::identity(),
                bounds: [0.0, 0.0, 0.0, 1.0],
                mesh: Handle::new(AssetId::new((entity % 4) as u32, 0)),
                material: Handle::new(AssetId::new((entity % 4) as u32, 0)),
                skin: None,
            });
        }
        assert_eq!(snapshot.instances.len(), count);
        assert_eq!(snapshot.batch_count(), 4);
    }
}

#[test]
fn sphere_culling_keeps_camera_facing_instances_and_rejects_far_instances() {
    let camera = Mat4::perspective_rh(60.0_f32.to_radians(), 1.0, 0.1, 10.0);
    let visible = TransformState {
        translation: Vec3::new(0.0, 0.0, -2.0),
        ..TransformState::identity()
    };
    let outside = TransformState {
        translation: Vec3::new(100.0, 0.0, -2.0),
        ..TransformState::identity()
    };
    assert!(sphere_visible(camera, visible, [0.0, 0.0, 0.0, 0.5]));
    assert!(!sphere_visible(camera, outside, [0.0, 0.0, 0.0, 0.5]));
}

#[test]
fn cpu_skinning_blends_joint_matrices() {
    let vertices = [SkinnedVertex {
        position: [0.0, 0.0, 0.0],
        joints: [0, 1, 0, 0],
        weights: [0.5, 0.5, 0.0, 0.0],
    }];
    let result = skin_vertices(
        &vertices,
        &[
            Mat4::from_translation(Vec3::X),
            Mat4::from_translation(Vec3::Y),
        ],
    );
    assert_eq!(result[0], [0.5, 0.5, 0.0]);
}

#[test]
fn preferences_and_graph_fallback_work_without_an_adapter() {
    let absent = RenderCapabilities::default();
    assert_eq!(
        RenderPreference::Auto.select(absent).unwrap(),
        RenderPath::Indirect
    );
    assert!(RenderPreference::GpuDriven.select(absent).is_err());
    let vertex_only = RenderCapabilities {
        storage_buffers: true,
        indirect_draw: true,
        compute: false,
    };
    assert_eq!(
        RenderPreference::Auto.select(vertex_only).unwrap(),
        RenderPath::Indirect
    );
    assert_eq!(
        RenderPreference::CpuIndirect.select(vertex_only).unwrap(),
        RenderPath::Indirect
    );
    let full = RenderCapabilities {
        compute: true,
        ..vertex_only
    };
    assert_eq!(
        RenderPreference::Auto.select(full).unwrap(),
        RenderPath::GpuDriven
    );
    let fallback = RenderGraph::for_path(RenderPreference::Auto.select(vertex_only).unwrap())
        .compile()
        .unwrap();
    assert_eq!(fallback.order, ["depth", "skinning", "forward"]);
    assert!(
        fallback
            .steps
            .iter()
            .all(|(_, kind)| *kind != RenderExecution::Compute)
    );
    let gpu = RenderGraph::default_forward().compile().unwrap();
    assert_eq!(gpu.steps[1], ("gpu_cull".into(), RenderExecution::Compute));
    assert_eq!(gpu.steps[3], ("forward".into(), RenderExecution::Render));
    let mut duplicate = RenderGraph::default_forward();
    duplicate.add_pass("forward", [] as [&str; 0], [] as [&str; 0]);
    assert!(duplicate.compile().is_err());
}
