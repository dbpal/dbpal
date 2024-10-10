fn main() {
    println!("cargo:rerun-if-changed={}", "proto/app.proto");
    tonic_build::compile_protos("proto/app.proto").unwrap();
}
