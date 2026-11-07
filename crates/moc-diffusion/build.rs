//! Embeds `weights/relief.mocd` when it exists, so builds without trained weights still
//! compile and simply fall back to the procedural detail.
use std::path::Path;

fn main() {
    println!("cargo::rustc-check-cfg=cfg(moc_relief_weights)");
    println!("cargo:rerun-if-changed=weights");
    println!("cargo:rerun-if-changed=weights/relief.mocd");
    if Path::new("weights/relief.mocd").is_file() {
        println!("cargo:rustc-cfg=moc_relief_weights");
    }
}
