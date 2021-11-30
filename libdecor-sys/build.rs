use pkg_config::Config;

fn main() {
    if std::env::var_os("CARGO_FEATURE_DLOPEN").is_some() {
        // Do not link to anything
        return;
    }

    Config::new().probe("libdecor-0").unwrap();
}
