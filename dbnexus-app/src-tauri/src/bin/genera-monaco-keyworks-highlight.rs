use dbnexus_db::{db::monaco_editor_keyword_regexp, redis_completion::REDIS_COMMANDS};
use std::{io::Write, path::Path};

fn compose(name: &str, content: &str) -> String {
    return format!("const {} = {};", name, content);
}
fn main() {
    let dir = env!("CARGO_MANIFEST_DIR");
    let filepath = Path::new(&dir)
        .to_path_buf()
        .join("../src/comp/keywords.ts");

    let mut ops = std::fs::OpenOptions::new();
    ops.write(true);
    ops.truncate(true);
    ops.create(true);
    let mut file = ops.open(filepath).unwrap();

    // redis
    {
        let content = monaco_editor_keyword_regexp(REDIS_COMMANDS);
        file.write(compose("redisKeywords", &content).as_bytes())
            .unwrap();
    }
    // TODO add more
}
