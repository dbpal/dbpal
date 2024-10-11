fn main() {
    println!("cargo:rerun-if-changed={}", "proto/app.proto");
    tonic_build::compile_protos("proto/app.proto").unwrap();

    tonic_build::configure()
        .out_dir("./proto_generated")
        .compile(&["proto/app.proto"], &["proto"])
        .unwrap();
}
