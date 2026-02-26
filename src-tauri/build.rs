fn main() {
    tauri_build::try_build(
        tauri_build::Attributes::new()
            .codegen(tauri_build::CodegenContext::new()),
    )
    .expect("tauri build failed");
}
