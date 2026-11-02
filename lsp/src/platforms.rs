/// Platform names accepted by Unity assembly definitions.
///
/// Unity has renamed some platforms over time, so the list keeps both the
/// legacy and current spellings. Unknown names produce a warning, not an error.
pub const PLATFORMS: &[&str] = &[
    "Android",
    "CloudRendering",
    "Editor",
    "EmbeddedLinux",
    "GameCoreScarlett",
    "GameCoreXboxOne",
    "iOS",
    "LinuxHeadless",
    "LinuxStandalone64",
    "Lumin",
    "macOSStandalone",
    "PS4",
    "PS5",
    "QNX",
    "Stadia",
    "StandaloneLinux64",
    "StandaloneOSX",
    "StandaloneWindows",
    "StandaloneWindows64",
    "Switch",
    "Switch2",
    "tvOS",
    "VisionOS",
    "WebGL",
    "WebGPU",
    "WindowsStandalone32",
    "WindowsStandalone64",
    "WSAPlayer",
    "XboxOne",
    "XboxSeriesX",
];

pub fn is_known(platform: &str) -> bool {
    PLATFORMS.iter().any(|known| *known == platform)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn includes_current_and_legacy_unity_names() {
        for platform in [
            "Editor",
            "Android",
            "iOS",
            "WebGL",
            "macOSStandalone",
            "LinuxStandalone64",
            "StandaloneOSX",
            "CloudRendering",
        ] {
            assert!(is_known(platform), "missing {platform}");
        }
        assert!(!is_known("Andriod"));
        assert_eq!(PLATFORMS.len(), PLATFORMS.iter().collect::<std::collections::HashSet<_>>().len());
    }
}
