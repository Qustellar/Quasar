use super::*;

#[test]
fn scene_round_trip() {
    let mut scene = SceneAsset::new("test");
    scene.push_entity(SceneEntity::default());
    let text = ron::to_string(&scene).unwrap();
    let decoded: SceneAsset = ron::from_str(&text).unwrap();
    assert_eq!(decoded.schema_version, SCENE_SCHEMA_VERSION);
    assert_eq!(decoded.entities.len(), 1);
}

#[test]
fn legacy_scene_is_migrated_to_stable_ids() {
    let path =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/playground.ron");
    let scene = SceneAsset::load_ron(path).unwrap();
    assert_eq!(scene.schema_version, SCENE_SCHEMA_VERSION);
    assert_eq!(scene.entities[0].id, EntityId::new(1));
    assert_eq!(scene.entities[1].parent, Some(EntityId::new(1)));
    assert!(scene.validate_ids().is_ok());
}

#[test]
fn hierarchy_orders_descendants_even_when_children_precede_parents() {
    let mut scene = SceneAsset::new("nested");
    scene.entities = vec![
        SceneEntity {
            id: EntityId::new(1),
            parent: Some(EntityId::new(3)),
            ..SceneEntity::default()
        },
        SceneEntity {
            id: EntityId::new(2),
            ..SceneEntity::default()
        },
        SceneEntity {
            id: EntityId::new(3),
            ..SceneEntity::default()
        },
        SceneEntity {
            id: EntityId::new(4),
            parent: Some(EntityId::new(1)),
            ..SceneEntity::default()
        },
    ];
    assert_eq!(scene.hierarchy_order().unwrap(), [1, 2, 0, 3]);
}

#[test]
fn hierarchy_rejects_missing_parents_and_cycles() {
    let mut scene = SceneAsset::new("invalid");
    scene.entities.push(SceneEntity {
        id: EntityId::new(1),
        parent: Some(EntityId::new(2)),
        ..SceneEntity::default()
    });
    assert!(scene.hierarchy_order().is_err());
    scene.entities.push(SceneEntity {
        id: EntityId::new(2),
        parent: Some(EntityId::new(1)),
        ..SceneEntity::default()
    });
    assert!(scene.hierarchy_order().is_err());
}

#[test]
fn unsupported_scene_schema_is_rejected() {
    let path = std::env::temp_dir().join(format!("quasar-scene-test-{}.ron", std::process::id()));
    let mut scene = SceneAsset::new("future");
    scene.schema_version = SCENE_SCHEMA_VERSION + 1;
    assert!(matches!(
        scene.save_ron(&path),
        Err(EngineError::Unsupported(_))
    ));
    std::fs::write(&path, ron::to_string(&scene).unwrap()).unwrap();
    assert!(matches!(
        SceneAsset::load_ron(&path),
        Err(EngineError::Unsupported(_))
    ));
    std::fs::remove_file(path).unwrap();
}

#[test]
fn gltf_import_reads_static_primitive() {
    use base64::Engine;
    let mut bytes = Vec::new();
    for value in [0.0_f32, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0] {
        bytes.extend(value.to_le_bytes());
    }
    for value in [0.0_f32, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0] {
        bytes.extend(value.to_le_bytes());
    }
    for index in [0_u16, 1, 2] {
        bytes.extend(index.to_le_bytes());
    }
    let uri = format!(
        "data:application/octet-stream;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    );
    let mut document = serde_json::json!({
        "asset": {"version": "2.0"},
        "buffers": [{"uri": uri, "byteLength": 78}],
        "bufferViews": [
            {"buffer": 0, "byteOffset": 0, "byteLength": 36},
            {"buffer": 0, "byteOffset": 36, "byteLength": 36},
            {"buffer": 0, "byteOffset": 72, "byteLength": 6}
        ],
        "accessors": [
            {"bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3", "min": [0,0,0], "max": [1,1,0]},
            {"bufferView": 1, "componentType": 5126, "count": 3, "type": "VEC3"},
            {"bufferView": 2, "componentType": 5123, "count": 3, "type": "SCALAR"}
        ],
        "materials": [{"pbrMetallicRoughness": {"baseColorFactor": [0.2, 0.7, 0.4, 1.0]}}],
        "meshes": [{"primitives": [{"attributes": {"POSITION": 0, "NORMAL": 1}, "indices": 2, "material": 0}]}],
        "nodes": [{"name": "Triangle", "mesh": 0}],
        "scenes": [{"nodes": [0]}],
        "scene": 0
    });
    let path = std::env::temp_dir().join(format!("quasar-import-test-{}.gltf", std::process::id()));
    std::fs::write(&path, document.to_string()).unwrap();
    let imported = import_gltf(&path).unwrap();
    assert_eq!(imported.meshes.len(), 1);
    assert_eq!(imported.meshes[0].indices, [0, 1, 2]);
    assert_eq!(imported.scene.entities.len(), 2);
    let _ = std::fs::remove_dir_all(std::env::temp_dir().join(".quasar"));
    let (_, first_hit) = import_gltf_cached(&path).unwrap();
    let (_, second_hit) = import_gltf_cached(&path).unwrap();
    assert!(!first_hit);
    assert!(second_hit);
    document["meshes"][0]["primitives"][0]["mode"] = serde_json::json!(1);
    std::fs::write(&path, document.to_string()).unwrap();
    assert!(matches!(
        import_gltf(&path),
        Err(EngineError::Unsupported(_))
    ));
    std::fs::remove_file(path).unwrap();
    let _ = std::fs::remove_dir_all(std::env::temp_dir().join(".quasar"));
}
