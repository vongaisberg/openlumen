//! Build script for RP2350 ArtNet Node

fn main() {
    // Link arguments for RP2350
    println!("cargo:rustc-link-arg-bins=--nmagic");
    println!("cargo:rustc-link-arg-bins=-Tlink.x");
    println!("cargo:rustc-link-arg-bins=-Tdefmt.x");
    
    // Re-run if memory.x changes
    println!("cargo:rerun-if-changed=memory.x");
}
