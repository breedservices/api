fn main() {
    let m = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let raw = std::env::var("BAPI_LIB").unwrap_or_else(|_| m.join("../../cpp/build").to_string_lossy().into_owned());
    let root = std::fs::canonicalize(&raw).map(|p| p.to_string_lossy().into_owned()).unwrap_or(raw);
    println!("cargo:rustc-link-search=native={root}");
    println!("cargo:rustc-link-lib=dylib=bimg");
    println!("cargo:rustc-link-arg=-Wl,-rpath,$ORIGIN");
    println!("cargo:rustc-link-arg=-Wl,-rpath,{root}");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=BAPI_LIB");
}
