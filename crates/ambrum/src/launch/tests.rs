// SPDX-License-Identifier: MPL-2.0
use super::*;
use crate::{
    auth::{Profile, Secret},
    registry::new_id,
    transaction::Destination,
};
use std::{io::Write, time::Instant};
struct Fixture(std::path::PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("ambrum-launch-{}", new_id().unwrap()));
        fs::create_dir(&root).unwrap();
        Self(root)
    }
    fn installed(&self) -> std::path::PathBuf {
        let cancel = Cancellation::default();
        let mut stage = Destination::open(&self.0)
            .unwrap()
            .begin("game with spaces")
            .unwrap();
        stage
            .write("libraries/test.jar", &mut b"payload".as_slice(), 7, &cancel)
            .unwrap();
        stage
            .write("versions/test/test.json", &mut b"{}".as_slice(), 2, &cancel)
            .unwrap();
        stage.seal(&cancel).unwrap();
        stage.commit(&cancel).unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
#[test]
fn installed_session_pins_payload_allows_gameplay_files_and_rejects_conflicts() {
    let fixture = Fixture::new();
    let root = fixture.installed();
    let cancel = Cancellation::default();
    fs::create_dir(root.join("saves")).unwrap();
    fs::write(root.join("options.txt"), b"game settings").unwrap();
    {
        let installed = Installed::open(&root, &cancel).unwrap();
        installed.check_game_paths(&cancel).unwrap();
        assert!(Installed::open(&root, &cancel).is_err());
        assert!(fs::write(root.join("libraries/test.jar"), b"changed").is_err());
        fs::write(root.join("options.txt"), b"updated").unwrap();
        assert!(fs::rename(&root, fixture.0.join("moved")).is_err());
    }
    assert!(Installed::open(&root, &cancel).is_ok());
    fs::write(root.join("libraries/test.jar"), b"changed").unwrap();
    assert!(
        Installed::open(&root, &cancel)
            .err()
            .unwrap()
            .contains("checksum")
    );
}
#[test]
fn moved_copied_or_linked_instances_are_not_silently_adopted() {
    let fixture = Fixture::new();
    let root = fixture.installed();
    let cancel = Cancellation::default();
    fs::hard_link(root.join("options-missing"), root.join("unused")).unwrap_err();
    fs::write(fixture.0.join("outside"), b"untouched").unwrap();
    fs::hard_link(fixture.0.join("outside"), root.join("options.txt")).unwrap();
    assert!(
        Installed::open(&root, &cancel)
            .unwrap()
            .check_game_paths(&cancel)
            .is_err()
    );
    assert_eq!(fs::read(fixture.0.join("outside")).unwrap(), b"untouched");
    let moved = fixture.0.join("moved");
    fs::rename(&root, &moved).unwrap();
    assert!(Installed::open(&moved, &cancel).is_err());
}

#[test]
fn unknown_game_directory_is_rejected_without_creating_a_session_lock() {
    let fixture = Fixture::new();
    fs::write(fixture.0.join("options.txt"), b"original settings").unwrap();
    assert!(Installed::open(&fixture.0, &Cancellation::default()).is_err());
    assert!(!fixture.0.join(".amberl-session.lock").exists());
    assert_eq!(fs::read_dir(&fixture.0).unwrap().count(), 1);
}
#[test]
fn placeholders_preserve_argument_boundaries_and_reject_unknowns() {
    let values = BTreeMap::from([
        ("path", r"C:\a b\中文\x".into()),
        ("literal", "${not-expanded}".into()),
    ]);
    assert_eq!(
        expand("-Dpath=${path}", &values).unwrap(),
        r"-Dpath=C:\a b\中文\x"
    );
    assert_eq!(expand("${literal}", &values).unwrap(), "${not-expanded}");
    assert!(expand("${missing}", &values).is_err());
    assert!(expand("${path", &values).is_err());
    assert!(expand("a\0b", &values).is_err());
    assert_eq!(java_path(Path::new(r"\\?\C:\a b")).unwrap(), r"C:\a b");
    assert_eq!(
        java_path(Path::new(r"\\?\UNC\server\share")).unwrap(),
        r"\\server\share"
    );
}
#[test]
fn explicit_java_major_architecture_and_memory_are_required() {
    let mut j = java::Java {
        version: "25.0.4".into(),
        major: 25,
        architecture: "amd64".into(),
        home: "chosen".into(),
    };
    assert!(java::compatible(&j, Some(25)).is_ok());
    assert!(java::compatible(&j, Some(21)).is_err());
    assert!(java::compatible(&j, None).is_err());
    j.architecture = "aarch64".into();
    assert!(java::compatible(&j, Some(25)).is_err());
    for n in [0, 511, 65537] {
        assert!(memory(n).is_err());
    }
    assert!(memory(4096).is_ok());
}
#[test]
fn launch_arguments_are_separate_and_expired_identity_never_starts() {
    let fixture = Fixture::new();
    let root = fixture.installed();
    let file = fixture.0.join("java.exe");
    fs::File::create(&file)
        .unwrap()
        .write_all(b"test only")
        .unwrap();
    let prepared = Prepared {
        installed: Installed::open(&root, &Cancellation::default()).unwrap(),
        plan: VersionPlan {
            version: "test".into(),
            java_major: Some(25),
            main_class: "net.minecraft.client.main.Main".into(),
            files: vec![],
            classpath: vec!["libraries/test.jar".into()],
            native_archives: vec![],
            game_arguments: vec![
                "--username".into(),
                "${auth_player_name}".into(),
                "--accessToken".into(),
                "${auth_access_token}".into(),
                "--gameDir".into(),
                "${game_directory}".into(),
            ],
            jvm_arguments: vec!["-cp".into(), "${classpath}".into()],
            logging_argument: None,
            asset_index: "test".into(),
        },
        metadata: serde_json::json!({"type":"release"}),
        report: Preflight {
            version: "test".into(),
            game_directory: root.clone(),
            java_executable: file.clone(),
            java: java::Java {
                version: "25".into(),
                major: 25,
                architecture: "amd64".into(),
                home: "test".into(),
            },
            max_memory_mib: 2048,
            resources_verified: true,
        },
        _java_parent: DirectoryGuard::open(&fixture.0).unwrap(),
        _java: fs::File::open(file).unwrap(),
    };
    let mut identity = Identity {
        profile: Profile {
            id: "0".repeat(32),
            name: "TestPlayer".into(),
        },
        token: Secret::new("synthetic-secret".into()).unwrap(),
        xuid: "123".into(),
        expires: Instant::now() + Duration::from_secs(60),
    };
    let args = arguments(&prepared, &identity, &root.join("natives")).unwrap();
    assert_eq!(args[0], "-Xmx2048M");
    assert_eq!(args[2], "-cp");
    assert_eq!(args[8], "synthetic-secret");
    assert_eq!(args.len(), 11);
    assert!(args[10].ends_with("game with spaces"));
    identity.expires = Instant::now();
    assert!(arguments(&prepared, &identity, &root).is_err());
}
