use std::collections::HashMap;
use std::fs;
use std::io::Read;

use sha2::{Digest, Sha256};
use zed_extension_api::{
    self as zed, settings::LspSettings, Architecture, DownloadedFileType, LanguageServerId,
    LanguageServerInstallationStatus, Os, Result,
};

const LSP_NAME: &str = "unity-asmdef";
const LSP_BINARY: &str = "unity-asmdef-lsp";
const LSP_REPO: &str = "Dust-Games-Studio/Zed-Unity-Extension";
const LSP_TAG: &str = "unity-asmdef-lsp-v0.2.1";
const CHECKSUMS_ASSET: &str = "SHA256SUMS";
// Consumed by the UXML installer in Task 2.
#[allow(dead_code)]
const UXML_LSP_NAME: &str = "unity-uxml";
// Consumed by the UXML installer in Task 2.
#[allow(dead_code)]
const LEM_MINX_REPO: &str = "redhat-developer/vscode-xml";
// Consumed by the UXML installer in Task 2.
#[allow(dead_code)]
const LEM_MINX_TAG: &str = "0.29.3";
const LEM_MINX_SHA256: &[(&str, &str)] = &[
    (
        "lemminx-win32",
        "25f571c0f07d0ad76be60a86c1b864d46cf7bf94773dc21e5d5da646478aac79",
    ),
    (
        "lemminx-osx-x86_64",
        "e13b4c135f56f906a7450bc9f64f49718bf743c5c4df8003a5610e1fb8772a55",
    ),
    (
        "lemminx-osx-aarch_64",
        "c5e9e2aee768eb69d041e9541cd0568a432ebd51c7fe9c1d7b7a1130b759475a",
    ),
    (
        "lemminx-linux-x86_64",
        "84cbbe3c31eb158799327d5f68a0dc876e81c431da596747b92eecffe956b3f2",
    ),
    (
        "lemminx-linux-aarch_64",
        "e706d10fbda4fa354952edb604800d067518401521d4d1e491b8bee7cf4cbcdb",
    ),
];

struct UnityExtension {
    cached_binary_path: Option<String>,
}

impl UnityExtension {
    fn language_server_binary_path(
        &mut self,
        language_server_id: &LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<String> {
        if let Some(path) = worktree.which(LSP_BINARY) {
            return Ok(path);
        }

        if let Some(path) = &self.cached_binary_path {
            if is_file(path) {
                return Ok(path.clone());
            }
        }

        let path = install_binary(language_server_id).map_err(|error| {
            format!(
                "failed to install {LSP_BINARY} from {LSP_REPO} {LSP_TAG}: {error}\n\
                 Install it yourself with `cargo install --path lsp` and put it on PATH, or point \
                 `lsp.{LSP_NAME}.binary.path` at the binary in your Zed settings."
            )
        })?;

        self.cached_binary_path = Some(path.clone());
        Ok(path)
    }
}

impl zed::Extension for UnityExtension {
    fn new() -> Self {
        Self {
            cached_binary_path: None,
        }
    }

    fn language_server_command(
        &mut self,
        language_server_id: &LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<zed::Command> {
        let binary = LspSettings::for_worktree(LSP_NAME, worktree)
            .ok()
            .and_then(|settings| settings.binary);
        let (path, args, env) = match binary {
            Some(binary) => (binary.path, binary.arguments, binary.env),
            None => (None, None, None),
        };
        let command = match path {
            Some(path) => path,
            None => self.language_server_binary_path(language_server_id, worktree)?,
        };

        Ok(zed::Command {
            command,
            args: args.unwrap_or_default(),
            env: env.unwrap_or_default().into_iter().collect(),
        })
    }
}

fn install_binary(language_server_id: &LanguageServerId) -> Result<String> {
    let (platform, architecture) = zed::current_platform();
    let asset = asset_name(platform, architecture)?;
    let version_dir = LSP_TAG.to_string();
    let binary_path = format!("{version_dir}/{asset}");

    if is_file(&binary_path) {
        return Ok(binary_path);
    }

    zed::set_language_server_installation_status(
        language_server_id,
        &LanguageServerInstallationStatus::CheckingForUpdate,
    );

    let release = zed::github_release_by_tag_name(LSP_REPO, LSP_TAG)?;
    let binary_asset = release
        .assets
        .iter()
        .find(|candidate| candidate.name == asset)
        .ok_or_else(|| format!("release {LSP_TAG} has no asset named {asset}"))?;
    let checksums_asset = release
        .assets
        .iter()
        .find(|candidate| candidate.name == CHECKSUMS_ASSET)
        .ok_or_else(|| format!("release {LSP_TAG} has no {CHECKSUMS_ASSET} asset"))?;

    zed::set_language_server_installation_status(
        language_server_id,
        &LanguageServerInstallationStatus::Downloading,
    );

    let download_dir = format!("{version_dir}.download");
    fs::remove_dir_all(&download_dir).ok();
    fs::create_dir_all(&download_dir)
        .map_err(|error| format!("failed to create {download_dir}: {error}"))?;

    let result = download_and_verify(
        &binary_asset.download_url,
        &checksums_asset.download_url,
        asset,
        &download_dir,
        &version_dir,
    );
    if result.is_err() {
        fs::remove_dir_all(&download_dir).ok();
    }
    result?;

    Ok(binary_path)
}

fn download_and_verify(
    binary_url: &str,
    checksums_url: &str,
    asset: &str,
    download_dir: &str,
    version_dir: &str,
) -> Result<()> {
    let downloaded_binary = format!("{download_dir}/{asset}");
    let downloaded_sums = format!("{download_dir}/{CHECKSUMS_ASSET}");

    zed::download_file(
        binary_url,
        &downloaded_binary,
        DownloadedFileType::Uncompressed,
    )
    .map_err(|error| format!("failed to download {asset}: {error}"))?;
    zed::download_file(
        checksums_url,
        &downloaded_sums,
        DownloadedFileType::Uncompressed,
    )
    .map_err(|error| format!("failed to download {CHECKSUMS_ASSET}: {error}"))?;

    let checksums = fs::read_to_string(&downloaded_sums)
        .map_err(|error| format!("failed to read {downloaded_sums}: {error}"))?;
    let expected = parse_sha256sums(&checksums)
        .remove(asset)
        .ok_or_else(|| format!("{CHECKSUMS_ASSET} does not contain {asset}"))?;
    let actual = sha256_file(&downloaded_binary)?;
    if !actual.eq_ignore_ascii_case(&expected) {
        return Err(format!(
            "checksum mismatch for {asset}: expected {expected}, got {actual}"
        ));
    }

    zed::make_file_executable(&downloaded_binary)?;
    fs::remove_dir_all(version_dir).ok();
    fs::rename(download_dir, version_dir)
        .map_err(|error| format!("failed to move {download_dir} to {version_dir}: {error}"))?;
    Ok(())
}

fn asset_name(platform: Os, architecture: Architecture) -> Result<&'static str> {
    match (platform, architecture) {
        (Os::Linux, Architecture::X8664) => Ok("unity-asmdef-lsp-linux-x86_64"),
        (Os::Linux, Architecture::Aarch64) => Ok("unity-asmdef-lsp-linux-aarch64"),
        (Os::Mac, Architecture::X8664) => Ok("unity-asmdef-lsp-macos-x86_64"),
        (Os::Mac, Architecture::Aarch64) => Ok("unity-asmdef-lsp-macos-aarch64"),
        // There is no native Windows arm64 build; x86_64 runs under emulation.
        (Os::Windows, Architecture::X8664 | Architecture::Aarch64) => {
            Ok("unity-asmdef-lsp-windows-x86_64.exe")
        }
        (_, Architecture::X86) => Err("unsupported architecture: 32-bit x86".to_string()),
    }
}

// Consumed by the UXML installer in Task 2.
#[allow(dead_code)]
fn lemminx_asset(platform: Os, architecture: Architecture) -> Result<&'static str> {
    match (platform, architecture) {
        (Os::Linux, Architecture::X8664) => Ok("lemminx-linux-x86_64"),
        (Os::Linux, Architecture::Aarch64) => Ok("lemminx-linux-aarch_64"),
        (Os::Mac, Architecture::X8664) => Ok("lemminx-osx-x86_64"),
        (Os::Mac, Architecture::Aarch64) => Ok("lemminx-osx-aarch_64"),
        (Os::Windows, Architecture::X8664 | Architecture::Aarch64) => Ok("lemminx-win32"),
        (_, Architecture::X86) => Err("unsupported architecture: 32-bit x86".to_string()),
    }
}

// Consumed by the UXML installer in Task 2.
#[allow(dead_code)]
fn lemminx_sha256(asset: &str) -> Result<&'static str> {
    LEM_MINX_SHA256
        .iter()
        .find(|(name, _)| *name == asset)
        .map(|(_, hash)| *hash)
        .ok_or_else(|| format!("no pinned checksum for {asset}"))
}

fn parse_sha256sums(contents: &str) -> HashMap<String, String> {
    let mut checksums = HashMap::new();
    for line in contents.lines() {
        let mut fields = line.split_whitespace();
        let Some(hash) = fields.next() else {
            continue;
        };
        let Some(file) = fields.next() else {
            continue;
        };
        if hash.len() != 64 || !hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            continue;
        }
        let file = file.trim_start_matches('*');
        if file.is_empty() {
            continue;
        }
        checksums.insert(file.to_string(), hash.to_ascii_lowercase());
    }
    checksums
}

fn sha256_file(path: &str) -> Result<String> {
    let mut file =
        fs::File::open(path).map_err(|error| format!("failed to open {path}: {error}"))?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0; 1 << 16];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|error| format!("failed to read {path}: {error}"))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

fn is_file(path: &str) -> bool {
    fs::metadata(path).is_ok_and(|metadata| metadata.is_file())
}

zed::register_extension!(UnityExtension);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_supported_platforms_to_release_assets() {
        assert_eq!(
            asset_name(Os::Linux, Architecture::X8664).unwrap(),
            "unity-asmdef-lsp-linux-x86_64"
        );
        assert_eq!(
            asset_name(Os::Linux, Architecture::Aarch64).unwrap(),
            "unity-asmdef-lsp-linux-aarch64"
        );
        assert_eq!(
            asset_name(Os::Mac, Architecture::X8664).unwrap(),
            "unity-asmdef-lsp-macos-x86_64"
        );
        assert_eq!(
            asset_name(Os::Mac, Architecture::Aarch64).unwrap(),
            "unity-asmdef-lsp-macos-aarch64"
        );
        assert_eq!(
            asset_name(Os::Windows, Architecture::X8664).unwrap(),
            "unity-asmdef-lsp-windows-x86_64.exe"
        );
        assert!(asset_name(Os::Windows, Architecture::Aarch64).is_ok());
        assert!(asset_name(Os::Linux, Architecture::X86).is_err());
    }

    #[test]
    fn parses_sha256sums_with_binary_and_text_markers() {
        let sums = "\
ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad  unity-asmdef-lsp-linux-x86_64
c3ab8ff13720e8ad9047dd39466b3c8974e592c2fa383d4a3960714caef0c4f2 *unity-asmdef-lsp-macos-aarch64
";
        let parsed = parse_sha256sums(sums);
        assert_eq!(
            parsed
                .get("unity-asmdef-lsp-linux-x86_64")
                .map(String::as_str),
            Some("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad")
        );
        assert_eq!(
            parsed
                .get("unity-asmdef-lsp-macos-aarch64")
                .map(String::as_str),
            Some("c3ab8ff13720e8ad9047dd39466b3c8974e592c2fa383d4a3960714caef0c4f2")
        );
    }

    #[test]
    fn rejects_malformed_sha256sums() {
        let parsed = parse_sha256sums("not-a-hash  file\n");
        assert!(parsed.is_empty());
    }

    #[test]
    fn maps_lemminx_assets_for_supported_platforms() {
        assert_eq!(
            lemminx_asset(Os::Linux, Architecture::X8664).unwrap(),
            "lemminx-linux-x86_64"
        );
        assert_eq!(
            lemminx_asset(Os::Linux, Architecture::Aarch64).unwrap(),
            "lemminx-linux-aarch_64"
        );
        assert_eq!(
            lemminx_asset(Os::Mac, Architecture::X8664).unwrap(),
            "lemminx-osx-x86_64"
        );
        assert_eq!(
            lemminx_asset(Os::Mac, Architecture::Aarch64).unwrap(),
            "lemminx-osx-aarch_64"
        );
        assert_eq!(
            lemminx_asset(Os::Windows, Architecture::X8664).unwrap(),
            "lemminx-win32"
        );
        assert_eq!(
            lemminx_asset(Os::Windows, Architecture::Aarch64).unwrap(),
            "lemminx-win32"
        );
        assert!(lemminx_asset(Os::Linux, Architecture::X86).is_err());
    }

    #[test]
    fn records_a_checksum_for_every_lemminx_asset() {
        for asset in [
            "lemminx-linux-x86_64",
            "lemminx-linux-aarch_64",
            "lemminx-osx-x86_64",
            "lemminx-osx-aarch_64",
            "lemminx-win32",
        ] {
            let hash = lemminx_sha256(asset).unwrap();
            assert_eq!(hash.len(), 64, "{asset}");
            assert!(hash.bytes().all(|byte| byte.is_ascii_hexdigit()), "{asset}");
        }
        assert!(lemminx_sha256("lemminx-unknown").is_err());
    }
}
