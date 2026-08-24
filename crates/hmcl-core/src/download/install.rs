//! Version installation: download the version JSON, client jar, libraries
//! and assets of a vanilla Minecraft version.
//!
//! Port of HMCL's `download.game.GameInstallTask` (vanilla subset).

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::Mutex;

use crate::download::file::{DownloadProgress, download_file};
use crate::download::version_list::RemoteVersion;
use crate::game::{AssetIndex, GameVersion};

/// Install status reported to the UI.
#[derive(Debug, Clone)]
pub struct InstallStatus {
    pub message: String,
    pub fraction: f32,
    pub done_bytes: u64,
    pub total_bytes: u64,
}

impl InstallStatus {
    fn new(progress: &DownloadProgress, message: String) -> Self {
        Self {
            message,
            fraction: progress.fraction(),
            done_bytes: progress.done(),
            total_bytes: progress.total(),
        }
    }
}

/// Fetch the fully resolved version manifest (inheritance folded).
pub async fn fetch_resolved_version(
    client: &reqwest::Client,
    version: &RemoteVersion,
) -> anyhow::Result<GameVersion> {
    let mut current = fetch_version_json(client, &version.url).await?;
    while let Some(parent_id) = current.inherits_from.clone() {
        let parent_url = format!(
            "https://piston-meta.mojang.com/v1/packages/{}/{}.json",
            parent_id, parent_id
        );
        let parent = fetch_version_json(client, &parent_url).await?;
        current = current.merge_with(&parent);
    }
    Ok(current)
}

async fn fetch_version_json(client: &reqwest::Client, url: &str) -> anyhow::Result<GameVersion> {
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("failed to GET {url}: {e}"))?;
    if !response.status().is_success() {
        anyhow::bail!("failed to fetch {url}: HTTP {}", response.status());
    }
    let text = response.text().await?;
    Ok(serde_json::from_str(&text)?)
}

/// The shared install progress, polled by the UI.
pub struct InstallTask {
    pub status: Arc<Mutex<Option<InstallStatus>>>,
    pub finished: Arc<Mutex<Option<Result<(), String>>>>,
}

impl InstallTask {
    pub fn poll_status(&self) -> Option<InstallStatus> {
        self.status.lock().unwrap().clone()
    }

    pub fn poll_result(&self) -> Option<Result<(), String>> {
        self.finished.lock().unwrap().take()
    }
}

/// Start installing `version` into `game_dir` on the background runtime.
pub fn spawn_install(version: RemoteVersion, game_dir: PathBuf) -> InstallTask {
    let status = Arc::new(Mutex::new(None));
    let finished = Arc::new(Mutex::new(None));
    let task = InstallTask {
        status: status.clone(),
        finished: finished.clone(),
    };
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("failed to start install runtime");
        let result =
            runtime.block_on(async move { install_version(&version, &game_dir, &status).await });
        *finished.lock().unwrap() = Some(result.map_err(|e| format!("{e:#}")));
    });
    task
}

/// Download and install a vanilla version.
pub async fn install_version(
    version: &RemoteVersion,
    game_dir: &Path,
    status: &Mutex<Option<InstallStatus>>,
) -> anyhow::Result<()> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()?;
    let progress = DownloadProgress::new();

    let report = |progress: &DownloadProgress, message: String| {
        *status.lock().unwrap() = Some(InstallStatus::new(progress, message));
    };

    let versions_dir = game_dir.join("versions").join(&version.id);
    let libraries_dir = game_dir.join("libraries");
    let assets_dir = game_dir.join("assets");
    std::fs::create_dir_all(&versions_dir)?;

    // 1. Version JSON.
    report(&progress, "下载版本元数据".to_owned());
    let resolved = fetch_resolved_version(&client, version).await?;
    let json_path = versions_dir.join(format!("{}.json", version.id));
    std::fs::write(&json_path, serde_json::to_string_pretty(&resolved)?)?;

    // 2. Client jar.
    let client_info = resolved.client_download();
    let jar_path = versions_dir.join(format!("{}.jar", version.id));
    progress.add_total(client_info.size.unwrap_or(0));
    report(&progress, "下载客户端".to_owned());
    download_file(
        &client,
        &client_info.url,
        &jar_path,
        client_info.sha1.as_deref(),
        client_info.size,
        &progress,
    )
    .await?;

    // 3. Libraries.
    let os = crate::game::rules::current_os();
    let mut library_total = 0u64;
    let mut library_downloads = Vec::new();
    for library in &resolved.libraries {
        if !library.applies_to_current_platform() {
            continue;
        }
        if let Some(download) = library.artifact_download() {
            library_total += download.size.unwrap_or(0);
            library_downloads.push(download);
        }
        if let Some(download) = library.native_download(os.name()) {
            library_total += download.size.unwrap_or(0);
            library_downloads.push(download);
        }
    }
    progress.add_total(library_total);
    for (index, download) in library_downloads.iter().enumerate() {
        let dest = libraries_dir.join(download.path.clone().unwrap_or_default());
        if index % 16 == 0 {
            report(&progress, "下载依赖库".to_owned());
        }
        download_file(
            &client,
            &download.url,
            &dest,
            download.sha1.as_deref(),
            download.size,
            &progress,
        )
        .await?;
    }

    // 4. Asset index.
    let asset_index_info = resolved.asset_index_info();
    let index_path = assets_dir
        .join("indexes")
        .join(format!("{}.json", asset_index_info.id));
    if asset_index_info.sha1.is_some() {
        progress.add_total(asset_index_info.size.unwrap_or(0));
    }
    report(&progress, "下载资源索引".to_owned());
    download_file(
        &client,
        &asset_index_info.url,
        &index_path,
        asset_index_info.sha1.as_deref(),
        asset_index_info.size,
        &progress,
    )
    .await?;
    let index_text = std::fs::read_to_string(&index_path)?;
    let asset_index: AssetIndex = serde_json::from_str(&index_text)?;

    // 5. Asset objects.
    let assets_total: u64 = asset_index.objects.values().map(|object| object.size).sum();
    progress.add_total(assets_total);
    let objects_dir = assets_dir.join("objects");
    for (index, object) in asset_index.objects.values().enumerate() {
        let dest = objects_dir.join(object.object_path());
        if index % 256 == 0 {
            report(&progress, "下载游戏资源".to_owned());
        }
        download_file(
            &client,
            &format!(
                "https://resources.download.minecraft.net/{}",
                object.object_path()
            ),
            &dest,
            None,
            Some(object.size),
            &progress,
        )
        .await?;
    }

    report(&progress, "安装完成".to_owned());
    Ok(())
}
