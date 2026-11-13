// SPDX-License-Identifier: MPL-2.0
#![cfg(windows)]
use ambrum::manifest::{AssetPlan, Platform, VersionPlan, allowed};
use serde_json::{Value, json};
fn file(path: &str) -> Value {
    json!({"path":path,"url":"https://example.invalid/artifact","size":3,"sha1":"a9993e364706816aba3e25717850c26c9cd0d89d"})
}
fn version() -> Value {
    json!({"id":"1.test","mainClass":"net.minecraft.client.main.Main","javaVersion":{"majorVersion":21},
        "downloads":{"client":file("client.jar")},"libraries":[{"downloads":{"artifact":file("a/a.jar")}}],
        "assetIndex":{"id":"assets-test","url":"https://example.invalid/index","size":3,"sha1":"a9993e364706816aba3e25717850c26c9cd0d89d"},
        "arguments":{"jvm":["-cp","${classpath}"],"game":["--gameDir","${game_directory}",{"rules":[{"action":"allow","features":{"is_demo_user":true}}],"value":"--demo"}]}})
}
#[test]
fn ordered_rules_support_versions_architecture_and_feature_defaults() {
    let p = Platform::windows_x64("10.0.26100");
    assert!(allowed(None, &p).unwrap());
    assert!(!allowed(Some(&json!([])), &p).unwrap());
    let r = json!([{"action":"allow"},{"action":"disallow","os":{"name":"windows","arch":"^(amd64|x86_64)$","version":"^10\\."}}]);
    assert!(!allowed(Some(&r), &p).unwrap());
    assert!(!allowed(Some(&json!([{"action":"allow","os":{"arch":"x86"}}])), &p).unwrap());
    assert!(
        allowed(
            Some(&json!([{"action":"allow","features":{"unknown_feature":false}}])),
            &p
        )
        .unwrap()
    );
    assert!(allowed(Some(&json!([{"action":"invalid"}])), &p).is_err());
    assert!(allowed(Some(&json!([{"action":"allow","os":{"version":"["}}])), &p).is_err());
}
#[test]
fn plan_keeps_argument_boundaries_and_applies_demo_only_when_selected() {
    let mut p = Platform::windows_x64("10.0");
    let v = version();
    let plan = VersionPlan::parse(&v, &p).unwrap();
    assert_eq!(plan.java_major, Some(21));
    assert_eq!(plan.files.len(), 3);
    assert_eq!(
        plan.classpath,
        vec!["libraries/a/a.jar", "versions/1.test/1.test.jar"]
    );
    assert!(!plan.game_arguments.contains(&"--demo".into()));
    p.features.insert("is_demo_user".into(), true);
    assert!(
        VersionPlan::parse(&v, &p)
            .unwrap()
            .game_arguments
            .contains(&"--demo".into())
    );
}
#[test]
fn inapplicable_libraries_are_skipped_and_old_native_archives_are_separate() {
    let mut v = version();
    v["libraries"] = json!([
        {"rules":[{"action":"allow","os":{"name":"osx"}}],"downloads":{"artifact":file("osx/only.jar")}},
        {"natives":{"windows":"natives-windows-${arch}"},"downloads":{"classifiers":{"natives-windows-64":file("native/win.jar")}},"extract":{"exclude":["META-INF/"]}}
    ]);
    let p = VersionPlan::parse(&v, &Platform::windows_x64("10.0")).unwrap();
    assert_eq!(p.native_archives, vec!["libraries/native/win.jar"]);
    assert_eq!(p.classpath, vec!["versions/1.test/1.test.jar"]);
    assert!(!p.files.iter().any(|f| f.path.contains("osx")));
}
#[test]
fn traversal_reserved_names_case_collision_and_conflicting_checksums_are_rejected() {
    for path in [
        "../outside.jar",
        "/root.jar",
        "C:/outside.jar",
        "a\\b.jar",
        "a//b.jar",
        "a/NUL.jar",
    ] {
        let mut v = version();
        v["libraries"][0]["downloads"]["artifact"]["path"] = json!(path);
        assert!(
            VersionPlan::parse(&v, &Platform::windows_x64("10.0")).is_err(),
            "{path}"
        );
    }
    let mut v = version();
    v["libraries"]
        .as_array_mut()
        .unwrap()
        .push(json!({"downloads":{"artifact":file("A/a.jar")}}));
    assert!(VersionPlan::parse(&v, &Platform::windows_x64("10.0")).is_err());
    v["libraries"][1]["downloads"]["artifact"]["path"] = json!("a/a.jar");
    v["libraries"][1]["downloads"]["artifact"]["size"] = json!(4);
    assert!(VersionPlan::parse(&v, &Platform::windows_x64("10.0")).is_err());
}
#[test]
fn unsupported_inheritance_and_entrypoints_are_not_reported_as_installable() {
    let mut v = version();
    v["inheritsFrom"] = json!("base");
    assert!(VersionPlan::parse(&v, &Platform::windows_x64("10.0")).is_err());
    v.as_object_mut().unwrap().remove("inheritsFrom");
    v["mainClass"] = json!("custom.Loader");
    assert!(VersionPlan::parse(&v, &Platform::windows_x64("10.0")).is_err());
}
#[test]
fn missing_java_version_stays_unknown_and_legacy_arguments_remain_individual() {
    let mut v = version();
    v.as_object_mut().unwrap().remove("javaVersion");
    v.as_object_mut().unwrap().remove("arguments");
    v["minecraftArguments"] = json!("--username ${auth_player_name} --gameDir ${game_directory}");
    let p = VersionPlan::parse(&v, &Platform::windows_x64("10.0")).unwrap();
    assert_eq!(p.java_major, None);
    assert_eq!(p.game_arguments.len(), 4);
    v["minecraftArguments"] = json!("--gameDir \"complex path\"");
    assert!(VersionPlan::parse(&v, &Platform::windows_x64("10.0")).is_err());
}
#[test]
fn assets_deduplicate_blobs_without_losing_legacy_layout_mappings() {
    let descriptor = json!({"hash":"a9993e364706816aba3e25717850c26c9cd0d89d","size":3});
    let p=AssetPlan::parse(&json!({"objects":{"a/file":descriptor,"b/file":descriptor},"virtual":true,"map_to_resources":true})).unwrap();
    assert_eq!(p.files.len(), 1);
    assert_eq!(p.mappings.len(), 2);
    assert!(p.virtual_layout && p.map_to_resources);
    assert!(AssetPlan::parse(&json!({"objects":{"../escape":descriptor}})).is_err());
    assert!(
        AssetPlan::parse(&json!({"objects":{"A/file":descriptor,"a/file":descriptor}})).is_err()
    );
}

#[test]
fn both_argument_formats_enforce_limits_and_reject_nul() {
    let platform = Platform::windows_x64("10.0");
    let mut v = version();
    v["arguments"]["game"] = json!(["bad\u{0}argument"]);
    assert!(VersionPlan::parse(&v, &platform).is_err());
    v["arguments"]["game"] = json!(["x".repeat(32769)]);
    assert!(VersionPlan::parse(&v, &platform).is_err());
    v.as_object_mut().unwrap().remove("arguments");
    v["minecraftArguments"] = json!("x ".repeat(4097));
    assert!(VersionPlan::parse(&v, &platform).is_err());
    v["minecraftArguments"] = json!("x".repeat(32769));
    assert!(VersionPlan::parse(&v, &platform).is_err());
}

#[test]
fn asset_aliases_must_agree_on_size_and_os_version_is_not_guessed() {
    let hash = "a9993e364706816aba3e25717850c26c9cd0d89d";
    assert!(
        AssetPlan::parse(&json!({"objects": {
            "one": {"hash": hash, "size": 3}, "two": {"hash": hash, "size": 4}
        }}))
        .is_err()
    );
    let platform = Platform::windows_x64("");
    assert!(
        allowed(
            Some(&json!([{"action":"allow","os":{"name":"windows","version":"^10"}}])),
            &platform
        )
        .is_err()
    );
    assert!(
        !allowed(
            Some(&json!([{"action":"allow","os":{"name":"osx","version":"^10"}}])),
            &platform
        )
        .unwrap()
    );
}
