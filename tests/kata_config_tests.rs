#[path = "../build_support/ca_config.rs"]
mod ca_config;
#[path = "../build_support/kata_config.rs"]
mod kata_config;

#[test]
fn repository_kata_file_is_valid() {
    let names = kata_config::parse(include_str!("../kata.yaml")).unwrap();
    assert!(!names.is_empty());
}

#[test]
fn repository_ca_bundle_is_valid() {
    let normalized = ca_config::parse_pem_bundle(include_str!("../certs/ca-roots.pem")).unwrap();
    assert!(normalized.contains("-----BEGIN CERTIFICATE-----"));
}
