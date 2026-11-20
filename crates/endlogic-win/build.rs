//! Links the app icon into `endlogic-win.exe` on Windows with the MSVC
//! linker. The resource file is made by `examples/icon.rs` and kept in the
//! repository, so no resource compiler is needed here.

fn main() {
    println!("cargo:rerun-if-changed=res/endlogic.res");
    let target = std::env::var("TARGET").unwrap_or_default();
    if target.ends_with("-windows-msvc") {
        let res = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("res")
            .join("endlogic.res");
        println!("cargo:rustc-link-arg-bin=endlogic-win={}", res.display());
    }
}
