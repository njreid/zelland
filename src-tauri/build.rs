fn main() {
    prost_build::compile_protos(&["proto/zelland.proto"], &["proto/"]).unwrap();
    tauri_build::build();
}