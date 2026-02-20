fn main() {
    // Tauri必須
    tauri_build::build();

    // macOS: OpenGL フレームワークと Syphon フレームワークをリンク
    // #[link] 属性だけでは bin のリンク時に不十分なため build.rs でも指定
    #[cfg(target_os = "macos")]
    {
        println!("cargo:rustc-link-lib=framework=OpenGL");

        // Syphon.framework を強制的にリンク
        // -needed_framework を使用することで、リンカーのデッドコード除去を回避
        println!("cargo:rustc-link-arg=-Wl,-needed_framework,Syphon");
    }

    // Windows: Spout2 SDK の FFI バインディングを生成
    #[cfg(target_os = "windows")]
    generate_spout_bindings();
}

#[cfg(target_os = "windows")]
fn generate_spout_bindings() {
    use std::path::PathBuf;

    let bindings_dir = PathBuf::from("bindings/spout2");

    // Spout2 SDK の .lib をリンク
    // SDK は https://github.com/leadedge/Spout2 から取得してください
    println!("cargo:rustc-link-search=native={}", bindings_dir.display());
    println!("cargo:rustc-link-lib=static=Spout2");
    println!("cargo:rerun-if-changed=bindings/spout2/SpoutLibrary.h");

    // bindgen でヘッダからRustバインディングを生成
    if bindings_dir.join("SpoutLibrary.h").exists() {
        let bindings = bindgen::Builder::default()
            .header(bindings_dir.join("SpoutLibrary.h").to_str().unwrap())
            .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
            .allowlist_function("GetSpout")
            .allowlist_function("Spout.*")
            .allowlist_type("SPOUTHANDLE")
            .generate()
            .expect("Spout2 bindgen failed");

        let out_path = PathBuf::from(std::env::var("OUT_DIR").unwrap());
        bindings
            .write_to_file(out_path.join("spout_bindings.rs"))
            .expect("Failed to write spout_bindings.rs");
    } else {
        eprintln!("cargo:warning=Spout2 SDK not found at bindings/spout2/. Spout output disabled.");
    }
}
