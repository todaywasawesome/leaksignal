fn main() {
    //TODO: check protoc version to set --experimental_allow_proto3_optional for < 3.15 only
    prost_build::Config::default()
        .protoc_arg("--experimental_allow_proto3_optional")
        .compile_protos(&["proto/leaksignal.proto"], &["proto"])
        .unwrap();
    prost_build::Config::default()
        .compile_protos(&["proto/grpc_service.proto"], &["proto"])
        .unwrap();
    build_data::set_GIT_COMMIT();
}
