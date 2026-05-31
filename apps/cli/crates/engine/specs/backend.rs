//! Backend defaulting rules shared by planners and presentation actions.

use crate::platform::PlatformId;
use crate::specs::item::ItemKind;

/// Returns the default backend for an explicit `auto` backend selection.
pub fn default_backend(kind: ItemKind, platform: PlatformId) -> &'static str {
    match kind {
        ItemKind::Tool => "homebrew",
        ItemKind::Package => match platform {
            PlatformId::Macos => "homebrew",
            PlatformId::Linux => "apt",
            PlatformId::Windows => "winget",
        },
        ItemKind::App => match platform {
            PlatformId::Macos => "homebrew-cask",
            PlatformId::Linux => "flatpak",
            PlatformId::Windows => "winget",
        },
    }
}

/// Converts a literal `auto` backend into the platform/type default.
pub fn normalize_auto_backend(
    kind: ItemKind,
    backend: Option<String>,
    platform: PlatformId,
) -> Option<String> {
    match backend.as_deref() {
        Some("auto") => Some(default_backend(kind, platform).to_string()),
        _ => backend,
    }
}

/// Returns the item kind implied by an unclassified, type-specific backend.
pub fn infer_item_kind_from_backend(backend: &str) -> Option<ItemKind> {
    match backend {
        "rustup" | "mise" | "asdf" | "aqua" | "npm" | "pnpm" | "yarn" | "cargo" | "go" | "pipx" => {
            Some(ItemKind::Tool)
        }
        "homebrew" | "brew" | "apt" | "apt-get" | "dnf" | "pacman" | "nix" => {
            Some(ItemKind::Package)
        }
        "homebrew-cask" | "brew-cask" | "cask" | "flatpak" | "snap" | "mas" => Some(ItemKind::App),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_auto_backend_by_item_kind_and_platform() {
        assert_eq!(
            normalize_auto_backend(
                ItemKind::Package,
                Some("auto".to_string()),
                PlatformId::Linux
            )
            .as_deref(),
            Some("apt")
        );
        assert_eq!(
            normalize_auto_backend(ItemKind::App, Some("auto".to_string()), PlatformId::Macos)
                .as_deref(),
            Some("homebrew-cask")
        );
    }

    #[test]
    fn preserves_explicit_backend_and_absent_backend() {
        assert_eq!(
            normalize_auto_backend(ItemKind::Tool, Some("mise".to_string()), PlatformId::Linux)
                .as_deref(),
            Some("mise")
        );
        assert_eq!(
            normalize_auto_backend(ItemKind::Tool, None, PlatformId::Linux),
            None
        );
    }

    #[test]
    fn infers_item_kind_from_type_specific_backend() {
        assert_eq!(infer_item_kind_from_backend("rustup"), Some(ItemKind::Tool));
        assert_eq!(
            infer_item_kind_from_backend("apt-get"),
            Some(ItemKind::Package)
        );
        assert_eq!(infer_item_kind_from_backend("cask"), Some(ItemKind::App));
        assert_eq!(infer_item_kind_from_backend("unknown"), None);
    }
}
