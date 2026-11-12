// SPDX-License-Identifier: MPL-2.0
use ambrum::{
    library::{Isolation, Library},
    paths,
    task::Cancellation,
};
use serde_json::json;
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

static SEQUENCE: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "ambrum-test-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn write(&self, path: &str, text: &str) {
        let p = self.0.join(path);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, text).unwrap();
    }
    fn profile(&self, id: &str, extra: serde_json::Value) {
        let mut value = json!({"id": id, "mainClass":"net.minecraft.client.main.Main"});
        value
            .as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        self.write(&format!("versions/{id}/{id}.json"), &value.to_string());
    }
    fn scan(&self) -> ambrum::library::Scan {
        Library::open(&self.0)
            .unwrap()
            .scan(&Cancellation::default())
            .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn content_presence_does_not_prove_isolation_and_scanning_does_not_write() {
    let f = Fixture::new();
    f.profile("demo", json!({}));
    f.write("versions/demo/mods/user.jar", "original");
    f.write("saves/world/level.dat", "save");
    let before = fs::read(f.0.join("versions/demo/demo.json")).unwrap();
    let scan = f.scan();
    assert_eq!(scan.instances[0].isolation, Isolation::Unknown);
    assert_eq!(
        fs::read(f.0.join("versions/demo/demo.json")).unwrap(),
        before
    );
    assert_eq!(
        fs::read_to_string(f.0.join("saves/world/level.dat")).unwrap(),
        "save"
    );
    assert_eq!(fs::read_dir(&f.0).unwrap().count(), 2);
}

#[test]
fn pcl_and_hmcl_evidence_can_conflict_without_selecting_a_directory() {
    let f = Fixture::new();
    f.profile("demo", json!({}));
    f.write(
        "versions/demo/hmclversion.cfg",
        r#"{"usesGlobal":false,"gameDirType":0}"#,
    );
    f.write(
        "versions/demo/PCL/Setup.ini",
        "VersionArgumentIndieV2:True\n",
    );
    let row = &f.scan().instances[0];
    assert_eq!(row.isolation, Isolation::Conflict);
    assert!(row.game_dir.is_none());
}

#[test]
fn missing_hmcl_global_configuration_is_unknown() {
    let f = Fixture::new();
    f.profile("demo", json!({}));
    for data in [
        r#"{"usesGlobal":true,"gameDirType":1}"#,
        r#"{"gameDirType":1}"#,
    ] {
        f.write("versions/demo/hmclversion.cfg", data);
        assert_eq!(f.scan().instances[0].isolation, Isolation::Unknown);
    }
}

#[test]
fn official_shared_and_pcl_isolated_layouts_are_distinguished() {
    let f = Fixture::new();
    f.profile("shared", json!({}));
    f.profile("isolated", json!({}));
    f.write(
        "launcher_profiles.json",
        r#"{"profiles":{"a":{"lastVersionId":"shared","gameDir":"."}}}"#,
    );
    f.write(
        "versions/isolated/PCL/Setup.ini",
        "VersionArgumentIndie:1\n",
    );
    let scan = f.scan();
    assert_eq!(scan.instances[0].isolation, Isolation::Isolated);
    assert_eq!(scan.instances[1].isolation, Isolation::Shared);
}

#[test]
fn inheritance_cycles_and_cross_library_game_dirs_are_rejected_per_row() {
    let f = Fixture::new();
    let outside = Fixture::new();
    f.profile("a", json!({"inheritsFrom":"b"}));
    f.profile("b", json!({"inheritsFrom":"a"}));
    f.profile("outside", json!({"gameDir": outside.0}));
    f.profile("good", json!({}));
    let scan = f.scan();
    assert_eq!(
        scan.instances.iter().filter(|i| i.issue.is_some()).count(),
        3
    );
    assert_eq!(
        scan.instances.iter().filter(|i| i.issue.is_none()).count(),
        1
    );
}

#[test]
fn equal_profile_names_in_different_libraries_have_different_identity() {
    let a = Fixture::new();
    let b = Fixture::new();
    a.profile("same", json!({}));
    b.profile("same", json!({}));
    assert_ne!(a.scan().instances[0].id, b.scan().instances[0].id);
}

#[test]
fn managed_metadata_requires_matching_identity_and_ambiguous_roots_are_rejected() {
    let f = Fixture::new();
    f.write(
        "instances/test/instance.json",
        r#"{"id":"other","game":"1.21.4"}"#,
    );
    assert!(f.scan().instances[0].issue.is_some());
    f.write(
        "instances/test/instance.json",
        r#"{"id":"test","name":"测试","game":"1.21.4","loader":"fabric"}"#,
    );
    assert_eq!(f.scan().instances[0].isolation, Isolation::Isolated);
    f.profile("another", json!({}));
    assert!(Library::open(&f.0).is_err());
}

#[test]
fn cancelling_scan_returns_no_partial_success() {
    let f = Fixture::new();
    f.profile("demo", json!({}));
    let token = Cancellation::default();
    token.cancel();
    assert!(Library::open(&f.0).unwrap().scan(&token).is_err());
}

#[test]
fn unsafe_names_and_oversized_metadata_fail_closed() {
    for name in [
        "../x",
        "a/b",
        "a\\b",
        "x:stream",
        "NUL.txt",
        "COM1",
        "trailing.",
        " ",
    ] {
        assert!(paths::name(name).is_err(), "{name}");
    }
    let f = Fixture::new();
    f.write("large.json", "123456789");
    assert!(paths::text(&fs::canonicalize(&f.0).unwrap(), &f.0.join("large.json"), 4).is_err());
}

#[test]
fn java_8_and_modern_versions_keep_their_actual_architecture() {
    for (version, major) in [
        ("1.8.0_452", 8),
        ("17.0.15", 17),
        ("21.0.7+6", 21),
        ("25-ea", 25),
    ] {
        let j = ambrum::java::parse(&format!(
            "java.version = {version}\nos.arch = amd64\njava.home = C:\\Java with spaces\n"
        ))
        .unwrap();
        assert_eq!(j.major, major);
        assert_eq!(j.architecture, "amd64");
    }
    assert!(ambrum::java::parse("openjdk version 21").is_err());
}
