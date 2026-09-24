use super::*;

#[test]
fn handles_are_stable_for_same_path() {
    let mut server = AssetServer::new();
    let a = server.load::<String>("assets/a.txt");
    let b = server.load::<String>("assets/a.txt");
    assert_eq!(a.id, b.id);
}

#[test]
fn reload_marks_asset_loading() {
    let mut server = AssetServer::new();
    let handle = server.load::<String>("assets/a.txt");
    server.mark_loaded(handle).unwrap();
    assert!(server.reload_path("assets/a.txt"));
    assert_eq!(server.state(handle), None);
    let current = server.load::<String>("assets/a.txt");
    assert_eq!(server.state(current), Some(LoadState::Loading));
}

#[test]
fn asynchronous_bytes_reach_loaded_state() {
    let path = std::env::temp_dir().join(format!("quasar-asset-test-{}.bin", std::process::id()));
    std::fs::write(&path, b"quasar").unwrap();
    let mut server = AssetServer::new();
    let handle = server.load_bytes_async(&path);
    for _ in 0..100 {
        server.poll_async();
        if server.state(handle) == Some(LoadState::Loaded) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert_eq!(server.bytes(handle).unwrap().0, b"quasar");
    std::fs::remove_file(path).unwrap();
}

#[test]
fn reloading_bytes_discards_old_generation_and_bounds_duplicate_loads() {
    let path = std::env::temp_dir().join(format!(
        "quasar-asset-reload-test-{}.bin",
        std::process::id()
    ));
    std::fs::write(&path, b"first").unwrap();
    let mut server = AssetServer::new();
    let old = server.load_bytes_async(&path);
    assert_eq!(server.load_bytes_async(&path).id, old.id);
    assert_eq!(server.in_flight.len(), 1);
    for _ in 0..100 {
        server.poll_async();
        if server.state(old) == Some(LoadState::Loaded) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert_eq!(server.bytes(old).unwrap().0, b"first");
    std::fs::write(&path, b"second").unwrap();
    assert!(server.reload_path(&path));
    assert!(server.bytes(old).is_none());
    assert_eq!(server.bytes.len(), 0);
    let current = server.load_bytes_async(&path);
    assert_ne!(current.id, old.id);
    for _ in 0..100 {
        server.poll_async();
        if server.state(current) == Some(LoadState::Loaded) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert_eq!(server.bytes(current).unwrap().0, b"second");
    assert_eq!(server.bytes.len(), 1);
    std::fs::remove_file(path).unwrap();
}
