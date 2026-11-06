#[path = "../build_support/kata_config.rs"]
mod kata_config;

#[test]
fn repository_kata_file_is_valid() {
    let names = kata_config::parse(include_str!("../kata.yaml")).unwrap();
    assert!(!names.is_empty());
}
