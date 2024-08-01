fn main() {
    env_logger::init();
    rustc_plugin::driver_main(stable_plugin::StablePlugin);
}
