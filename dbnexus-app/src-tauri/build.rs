fn main() {
    // Generate TypeScript bindings before build
    // tauri_typegen::BuildSystem::generate_at_build_time()
    //     .expect("Failed to generate TypeScript bindings");

    // tauri_typegen::BuildSystem::new(true, true)
    //     .run_generation()
    //     .unwrap();

    tauri_build::build();
}
