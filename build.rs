use std::env;

fn main() {
    let rust_toolchain = env::var("RUSTUP_TOOLCHAIN").unwrap();
    dbg!(&rust_toolchain);
    if rust_toolchain.contains("stable") {
        // do nothing
    } else if rust_toolchain.contains("nightly") {
        //enable the 'nightly-features' feature flag
        println!("cargo:rustc-cfg=feature=\"is-nightly\"");
    } else {
        panic!(
            "Unexpected value for rustc toolchain, got {}",
            rust_toolchain
        )
    }
}
