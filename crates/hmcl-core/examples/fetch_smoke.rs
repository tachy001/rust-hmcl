//! Smoke test: fetch and resolve real version manifests from Mojang.
use hmcl_core::download::version_list::RemoteVersion;
use hmcl_core::download::fetch_resolved_version;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let client = reqwest::Client::new();
    let manifest = hmcl_core::download::fetch_version_manifest().await?;
    println!("latest release: {}", manifest.latest.release);

    let pick = |id: &str| -> RemoteVersion {
        manifest
            .versions
            .iter()
            .find(|v| v.id == id)
            .unwrap()
            .clone()
    };

    for version_id in ["1.21.11", "1.7.10", "1.12.2"] {
        let version = pick(version_id);
        let resolved = fetch_resolved_version(&client, &version).await?;
        let client_info = resolved.client_download();
        println!(
            "{}: main={:?} java={} libraries={} assetIndex={:?} jar_sha1={:?}",
            version.id,
            resolved.main_class,
            resolved.java_major_version(),
            resolved.libraries.len(),
            resolved.asset_index_info().id,
            client_info.sha1,
        );
        let os = hmcl_core::game::rules::current_os();
        let applicable = resolved
            .libraries
            .iter()
            .filter(|l| l.applies_to_current_platform())
            .count();
        let natives = resolved
            .libraries
            .iter()
            .filter(|l| l.native_download(os.name()).is_some())
            .count();
        println!("  applicable libraries: {applicable}, with natives: {natives}");
    }
    Ok(())
}
