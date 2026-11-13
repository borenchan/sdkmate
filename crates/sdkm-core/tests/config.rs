use anyhow::Context;
use std::fs;
use toml::Table;

#[test]
fn test_read_toml() {
    let toml_file = "Cargo.toml";
    let config_file = fs::read_to_string(toml_file)
        .context("not found sdkm config file `config.toml` in current install dir")
        .unwrap();
    let config = config_file.parse::<Table>().unwrap();
    println!("{:?}", config)
}
