use std::{env, io::Write, path::Path};

use dbnexus_db::language_server::{CompletionKind, TokenModifier};

fn main() {
    let exe = env::current_exe().unwrap();

    let dir = env!("CARGO_MANIFEST_DIR");
    let filepath = Path::new(&dir).to_path_buf().join("../src/comp/legend.ts");

    let mut ops = std::fs::OpenOptions::new();
    ops.write(true);
    ops.truncate(true);
    ops.create(true);
    let mut file = ops.open(filepath).unwrap();

    file.write(format!("// generate by {:?}\n", exe).as_bytes())
        .unwrap();

    {
        let x = enum_iterator::all::<CompletionKind>().collect::<Vec<_>>();
        let last = x.len() - 1;
        let mut s = "const tokenTypes = [".to_string();
        for e in x.iter().enumerate() {
            s.push_str(format!("'{}'", e.1).as_str());
            if e.0 != last {
                s.push_str(",");
            }
        }
        s.push_str("];\n");
        file.write_all(s.as_bytes()).unwrap();
    }

    {
        let x = enum_iterator::all::<TokenModifier>().collect::<Vec<_>>();
        let last = x.len() - 1;
        let mut s = "const tokenModifiers = [".to_string();

        for e in x.iter().enumerate() {
            s.push_str(format!("'{}'", e.1).as_str());
            if e.0 != last {
                s.push_str(",");
            }
        }
        s.push_str("];\n");
        file.write_all(s.as_bytes()).unwrap();
    }

    file.write("export {tokenModifiers , tokenTypes} \n".as_bytes())
        .unwrap();
}
