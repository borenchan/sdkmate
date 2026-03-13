

#[cfg(test)]
mod tests {
    use std::fs;
    use anyhow::Context;
    use toml::Table;
    use sdkcore::manager::config::SdkmConfig;

    #[test]
    fn test_read_toml() {
        let toml_file = "Cargo.toml";
        let config_file = fs::read_to_string(toml_file).context("not found sdkm config file `config.toml` in current install dir").unwrap();
        let config =config_file.parse::<Table>().unwrap();
        println!("{:?}",config)
    }

}