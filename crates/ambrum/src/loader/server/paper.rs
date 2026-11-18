// SPDX-License-Identifier: MPL-2.0
//! Stable Paper builds from the official Fill v3 catalog.
use super::*;
fn build(value: &str) -> Result<u32> {
    let build = value.parse::<u32>().map_err(|_| "Invalid Paper build")?;
    if build == 0 || build.to_string() != value {
        return Err("Invalid Paper build".into());
    }
    Ok(build)
}
fn get(game: &str, suffix: &str, net: &impl Transport, c: &Cancellation) -> Result<Value> {
    segment(game)?;
    serde_json::from_slice(&bounded(
        net,
        &format!("https://fill.papermc.io/v3/projects/paper/versions/{game}/builds{suffix}"),
        c,
    )?)
    .map_err(|_| "Invalid Paper catalog".into())
}
pub(super) fn versions(game: &str, net: &impl Transport, c: &Cancellation) -> Result<Vec<String>> {
    let data = get(game, "", net, c)?;
    let rows = data.as_array().ok_or("Missing Paper builds")?;
    if rows.len() > 4096 {
        return Err("Paper build limit".into());
    }
    let mut builds = rows
        .iter()
        .filter(|v| v["channel"] == "STABLE")
        .map(|v| {
            let value = v["id"]
                .as_u64()
                .filter(|n| *n > 0 && *n <= u32::MAX as u64)
                .ok_or("Invalid Paper build")?;
            Ok(value as u32)
        })
        .collect::<Result<Vec<_>>>()?;
    builds.sort_unstable_by(|a, b| b.cmp(a));
    builds.dedup();
    Ok(builds.into_iter().map(|n| n.to_string()).collect())
}
pub(super) fn validate(a: &Artifact, game: &str, version: &str) -> Result<()> {
    segment(game)?;
    build(version)?;
    a.validate()?;
    if !matches!(a.algorithm, Algorithm::Sha256)
        || a.size > 256 * 1024 * 1024
        || a.url
            != format!(
                "https://fill-data.papermc.io/v1/objects/{}/paper-{game}-{version}.jar",
                a.digest
            )
    {
        return Err("Invalid Paper artifact identity".into());
    }
    Ok(())
}
pub(super) fn install(
    cache: &Cache,
    root: &Path,
    game: &str,
    choice: &Choice,
    c: &Cancellation,
) -> Result<Runtime> {
    build(&choice.version)?;
    let data = get(
        game,
        &format!("/{}", choice.version),
        &crate::download::Https,
        c,
    )?;
    if data["channel"] != "STABLE" || data["id"].as_u64() != Some(build(&choice.version)? as u64) {
        return Err("Paper build selection changed".into());
    }
    let d = &data["downloads"]["server:default"];
    let a = Artifact {
        url: d["url"].as_str().ok_or("Missing Paper URL")?.into(),
        size: d["size"].as_u64().ok_or("Missing Paper size")?,
        algorithm: Algorithm::Sha256,
        digest: d["checksums"]["sha256"]
            .as_str()
            .ok_or("Missing Paper hash")?
            .into(),
    };
    validate(&a, game, &choice.version)?;
    cache.fetch(&a, &crate::download::Https, c, |_| {})?;
    let mut source = cache.verified_file(&a, c)?;
    let mut file = options()
        .write(true)
        .create_new(true)
        .open(root.join("paper.jar"))
        .map_err(|e| e.to_string())?;
    std::io::copy(&mut source, &mut file)
        .and_then(|_| file.sync_all())
        .map_err(|e| e.to_string())?;
    let runtime = Runtime {
        choice: choice.clone(),
        game: game.into(),
        main: "paper.jar".into(),
        files: vec![File {
            path: "paper.jar".into(),
            artifact: a,
        }],
    };
    runtime.validate(game)?;
    Ok(runtime)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn paper_identity_requires_exact_build_hash_and_origin() {
        let digest = "a".repeat(64);
        let mut a = Artifact {
            url: format!("https://fill-data.papermc.io/v1/objects/{digest}/paper-26.3-249.jar"),
            size: 100,
            algorithm: Algorithm::Sha256,
            digest,
        };
        validate(&a, "26.3", "249").unwrap();
        assert!(validate(&a, "26.3", "250").is_err());
        a.url = a.url.replace("fill-data.papermc.io", "example.com");
        assert!(validate(&a, "26.3", "249").is_err());
    }
}
