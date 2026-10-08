fn main() {
    println!("cargo:rerun-if-env-changed=LUDUSAVI_VERSION");
    println!("cargo:rerun-if-env-changed=LUDUSAVI_VARIANT");

    #[cfg(target_os = "macos")]
    if std::env::var_os("CARGO_FEATURE_APP").is_some() {
        println!("cargo:rerun-if-changed=src/gui/macos_reopen.m");
        cc::Build::new()
            .file("src/gui/macos_reopen.m")
            .flag("-fobjc-arc")
            .compile("ludusavi_reopen");
        println!("cargo:rustc-link-lib=framework=AppKit");
        println!("cargo:rustc-link-lib=framework=Foundation");
    }

    #[cfg(windows)]
    {
        println!("cargo:rerun-if-changed=assets/windows/manifest.rc");
        println!("cargo:rerun-if-changed=assets/windows/manifest.xml");

        embed_resource::compile("assets/windows/manifest.rc", embed_resource::NONE)
            .manifest_required()
            .unwrap();
    }
}
