fn main() {
    env_logger::init();
    rustc_plugin::cli_main(stable_plugin::StablePlugin);
}
