//! Vulkan version a driver supports, from its ICD manifest: no library is
//! loaded.

use crate::util::Root;

/// Mesa and vendor manifest names by kernel driver.
fn icd_names(driver: &str) -> &'static [&'static str] {
    match driver {
        "amdgpu" | "radeon" => &["radeon_icd", "amd_icd"],
        "nvidia" => &["nvidia_icd"],
        "i915" | "xe" => &["intel_icd", "intel_hasvk_icd"],
        "nouveau" => &["nouveau_icd"],
        "virtio_gpu" | "virtio-pci" => &["virtio_icd"],
        "v3d" | "vc4" => &["broadcom_icd"],
        "msm" => &["freedreno_icd"],
        "panfrost" | "panthor" => &["panfrost_icd"],
        "asahi" => &["asahi_icd"],
        _ => &[],
    }
}

fn icd_dirs() -> Vec<String> {
    let mut dirs: Vec<String> = std::env::var("XDG_DATA_DIRS")
        .unwrap_or_default()
        .split(':')
        .filter(|d| !d.is_empty())
        .map(|d| format!("{d}/vulkan/icd.d"))
        .collect();
    dirs.extend(
        [
            "/run/opengl-driver/share/vulkan/icd.d",
            "/etc/vulkan/icd.d",
            "/usr/local/share/vulkan/icd.d",
            "/usr/share/vulkan/icd.d",
        ]
        .map(str::to_owned),
    );
    dirs
}

/// `"api_version": "1.4.354"` in a manifest.
fn api_version(manifest: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(manifest).ok()?;
    Some(v.get("ICD")?.get("api_version")?.as_str()?.to_owned())
}

pub(super) fn version(root: &Root, driver: &str) -> Option<String> {
    let names = icd_names(driver);
    icd_dirs().iter().find_map(|dir| {
        root.list(dir)
            .into_iter()
            .filter(|f| names.iter().any(|n| f.starts_with(&format!("{n}."))))
            .find_map(|f| api_version(&root.read(format!("{dir}/{f}"))?))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_manifest_version() {
        let m = r#"{"ICD": {"api_version": "1.4.354", "library_path": "x.so"}, "file_format_version": "1.0.1"}"#;
        assert_eq!(api_version(m).as_deref(), Some("1.4.354"));
        assert_eq!(icd_names("amdgpu")[0], "radeon_icd");
    }
}
