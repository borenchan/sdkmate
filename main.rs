use cli::{SdkMateCli};

fn main() {
    let cli = SdkMateCli::parse();

    cli.run();
}
