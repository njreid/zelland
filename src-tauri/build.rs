use std::env;
use std::path::{PathBuf};
use std::process::Command;

fn main() {
    // Protos
    let mut config = prost_build::Config::new();
    config.type_attribute(".", "#[derive(serde::Serialize, serde::Deserialize)]");
    config.compile_protos(&["../proto/zelland.proto"], &["../proto/"]).unwrap();

    // libghostty-vt build
    build_libghostty();

    // Tauri
    tauri_build::build();
}

fn build_libghostty() {
    let target = env::var("TARGET").unwrap();
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let ghostty_dir = manifest_dir.parent().unwrap().join("libghostty/ghostty");

    // Mapping Rust target to Zig target
    let (zig_target, android_api) = match target.as_str() {
        "aarch64-linux-android" => ("aarch64-linux-android", Some("30")),
        "armv7-linux-androideabi" => ("arm-linux-androideabi", Some("30")),
        "x86_64-linux-android" => ("x86_64-linux-android", Some("30")),
        "i686-linux-android" => ("i686-linux-android", Some("30")),
        "x86_64-unknown-linux-gnu" => ("x86_64-linux-gnu", None),
        "aarch64-unknown-linux-gnu" => ("aarch64-linux-gnu", None),
        _ => {
            if target == env::var("HOST").unwrap() {
                ("native", None)
            } else {
                panic!("Unsupported target for libghostty: {}", target);
            }
        }
    };

    let mut zig_build_target = zig_target.to_string();
    if let Some(api) = android_api {
        zig_build_target.push('.');
        zig_build_target.push_str(api);
    }

    println!("cargo:warning=Building libghostty-vt for target: {}", zig_build_target);

    // Build
    let mut cmd = Command::new("zig");
    cmd.arg("build")
        .arg("-Demit-lib-vt=true")
        .arg("-Dapp-runtime=none")
        .arg("-Doptimize=ReleaseFast")
        .arg("-Dsimd=false")
        .arg(format!("-Dtarget={}", zig_build_target))
        .arg("-p")
        .arg(&out_dir.join("libghostty-vt"))
        .current_dir(&ghostty_dir);

    let status = cmd.status().expect("Failed to run zig build");
    if !status.success() {
        panic!("Zig build failed for libghostty-vt");
    }

    let lib_dir = out_dir.join("libghostty-vt/lib");
    println!("cargo:rustc-link-search=native={}", lib_dir.display());
    println!("cargo:rustc-link-lib=static=ghostty-vt");

    if target.contains("android") {
        println!("cargo:rustc-link-lib=c++_shared");
        println!("cargo:rustc-link-lib=nativewindow");
        println!("cargo:rustc-link-lib=android");

        // Help the linker find NDK libraries for the correct target architecture.
        // zig_target already holds the NDK arch directory name (e.g. "arm-linux-androideabi").
        let api = android_api.unwrap_or("30");
        let sysroot_rel = format!(
            "toolchains/llvm/prebuilt/linux-x86_64/sysroot/usr/lib/{}/{}",
            zig_target, api
        );
        if let Ok(ndk_home) = env::var("ANDROID_NDK_HOME").or_else(|_| env::var("NDK_HOME")) {
            let sysroot_libs = PathBuf::from(ndk_home).join(&sysroot_rel);
            if sysroot_libs.exists() {
                println!("cargo:rustc-link-search=native={}", sysroot_libs.display());
            }
        }
    }

    // Bindings
    let header = ghostty_dir.join("include/ghostty/vt.h");
    let bindings = bindgen::Builder::default()
        .header(header.to_str().unwrap())
        .clang_arg(format!("-I{}", ghostty_dir.join("include").display()))
        .blocklist_type("GhosttyTerminal")
        .blocklist_type("GhosttyRenderState")
        .blocklist_type("GhosttyRenderStateRowIterator")
        .blocklist_type("GhosttyRenderStateRowCells")
        .blocklist_type("GhosttyFormatter")
        .blocklist_type("GhosttyOscParser")
        .blocklist_type("GhosttyOscCommand")
        .blocklist_type("GhosttySgrParser")
        .blocklist_type("GhosttyKeyEvent")
        .blocklist_type("GhosttyKeyEncoder")
        .blocklist_type("GhosttyMouseEvent")
        .blocklist_type("GhosttyMouseEncoder")
        .allowlist_function("ghostty_.*")
        .allowlist_type("Ghostty.*")
        .generate()
        .expect("Unable to generate bindings");

    let mut bindings_str = bindings.to_string();
    let additions = "
pub type GhosttyTerminal = *mut ::std::os::raw::c_void;
pub type GhosttyRenderState = *mut ::std::os::raw::c_void;
pub type GhosttyRenderStateRowIterator = *mut ::std::os::raw::c_void;
pub type GhosttyRenderStateRowCells = *mut ::std::os::raw::c_void;
pub type GhosttyFormatter = *mut ::std::os::raw::c_void;
pub type GhosttyOscParser = *mut ::std::os::raw::c_void;
pub type GhosttyOscCommand = *mut ::std::os::raw::c_void;
pub type GhosttySgrParser = *mut ::std::os::raw::c_void;
pub type GhosttyKeyEvent = *mut ::std::os::raw::c_void;
pub type GhosttyKeyEncoder = *mut ::std::os::raw::c_void;
pub type GhosttyMouseEvent = *mut ::std::os::raw::c_void;
pub type GhosttyMouseEncoder = *mut ::std::os::raw::c_void;
";
    bindings_str.push_str(additions);

    std::fs::write(out_dir.join("ghostty_vt_bindings.rs"), bindings_str)
        .expect("Couldn't write bindings!");
}
