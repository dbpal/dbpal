use std::io::Write;
use std::path::Path;

fn main() {
    tauri_build::build();

    // write_ts_debug_var();
}

#[allow(unused)]
fn write_ts_debug_var() {
    #[allow(unused)]
    let debug = false;
    #[cfg(debug_assertions)]
    #[allow(unused)]
    let debug = true;
    let mut ops = std::fs::OpenOptions::new();
    let dir = env!("CARGO_MANIFEST_DIR");
    let filepath = Path::new(&dir).to_path_buf().join("../src/comp/debug.ts");

    let mut file = ops
        .write(true)
        .create(true)
        .truncate(true)
        .open(filepath.as_path())
        .unwrap();
    file.write_all("// generate by rust don't edit\n".as_bytes())
        .unwrap();
    file.write_all(format!("const debug = {};\n", debug).as_bytes())
        .unwrap();
    file.write_all("export { debug }\n".as_bytes()).unwrap();
}
