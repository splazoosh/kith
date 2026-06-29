//! Build script: runs `tauri_build::build()` to parse `tauri.conf.json`,
//! generate the ACL/capability schemas under `gen/`, and embed the Windows
//! resource (icon + version metadata) for the `kith-tauri` binary.

fn main() {
    tauri_build::build();
}
