//! A Rustc plugin that prints out the name of all items in a crate via StableMIR.

#![feature(rustc_private)]

extern crate rustc_driver;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_session;
extern crate rustc_smir;
extern crate rustc_span;
extern crate stable_mir;

use clap::Parser;
use rustc_middle::ty::TyCtxt;
use rustc_plugin::{CrateFilter, RustcPlugin, RustcPluginArgs, Utf8Path};
use rustc_smir::rustc_internal;
use serde::{Deserialize, Serialize};
use std::{borrow::Cow, env, process::Command};

// This struct is the plugin provided to the rustc_plugin framework,
// and it must be exported for use by the CLI/driver binaries.
pub struct StablePlugin;

// To parse CLI arguments, we use Clap for this example. But that
// detail is up to you.
#[derive(Parser, Serialize, Deserialize)]
pub struct StablePluginArgs {
    #[clap(last = true)]
    cargo_args: Vec<String>,
}

impl RustcPlugin for StablePlugin {
    type Args = StablePluginArgs;

    fn version(&self) -> Cow<'static, str> {
        env!("CARGO_PKG_VERSION").into()
    }

    fn driver_name(&self) -> Cow<'static, str> {
        "stable-plugin-driver".into()
    }

    // In the CLI, we ask Clap to parse arguments and also specify a CrateFilter.
    // If one of the CLI arguments was a specific file to analyze, then you
    // could provide a different filter.
    fn args(&self, _target_dir: &Utf8Path) -> RustcPluginArgs<Self::Args> {
        let args = StablePluginArgs::parse_from(env::args().skip(1));
        let filter = CrateFilter::AllCrates;
        RustcPluginArgs { args, filter }
    }

    // Pass Cargo arguments (like --feature) from the top-level CLI to Cargo.
    fn modify_cargo(&self, cargo: &mut Command, args: &Self::Args) {
        // Find the default target triplet.
        let output = Command::new("rustc")
            .arg("-vV")
            .output()
            .expect("Cannot get default rustc target");
        let stdout = String::from_utf8(output.stdout).expect("Cannot parse stdout");
        // Parse the triplet.
        let mut target = String::from("");
        for part in stdout.split("\n") {
            if part.starts_with("host: ") {
                target = part.chars().skip("host: ".len()).collect();
            }
        }
        if target.len() == 0 {
            panic!("Bad output");
        }
        // Make sure standard library is built during the compilation.
        cargo.arg("-Zbuild-std");
        cargo.arg(format!("--target={}", target));
        cargo.args(&args.cargo_args);
    }

    // In the driver, we use the Rustc API to start a compiler session
    // for the arguments given to us by rustc_plugin.
    fn run(
        self,
        mut compiler_args: Vec<String>,
        plugin_args: Self::Args,
    ) -> rustc_interface::interface::Result<()> {
        let mut callbacks = StablePluginCallbacks {
            args: plugin_args,
            result: None,
        };
        // Make sure the compiler does not throw away MIR for other crates so we can retrieve it later.
        compiler_args.push("-Zalways-encode-mir".to_string());
        let compiler = rustc_driver::RunCompiler::new(&compiler_args, &mut callbacks);
        compiler.run()
    }
}

struct StablePluginCallbacks {
    args: StablePluginArgs,
    result: Option<rustc_driver::Compilation>,
}

impl rustc_driver::Callbacks for StablePluginCallbacks {
    // At the top-level, the Rustc API uses an event-based interface for
    // accessing the compiler at different stages of compilation. In this callback,
    // all the type-checking has completed.
    fn after_analysis<'tcx>(
        &mut self,
        _compiler: &rustc_interface::interface::Compiler,
        queries: &'tcx rustc_interface::Queries<'tcx>,
    ) -> rustc_driver::Compilation {
        // We extract a key data structure, the `TyCtxt`, which is all we need
        // for our simple task of printing out item names.
        queries.global_ctxt().unwrap().enter(|tcx| {
            // We instantiate StableMIR and pass the callback into it.
            rustc_internal::run(tcx, || {
                self.result = Some(print_all_items(tcx, &self.args));
            })
            .unwrap();
            // Check the callback return value.
            if self
                .result
                .as_ref()
                .is_some_and(|val| matches!(val, rustc_driver::Compilation::Continue))
            {
                rustc_driver::Compilation::Continue
            } else {
                rustc_driver::Compilation::Stop
            }
        })
    }
}

// Analysis callback.
fn print_all_items(_tcx: TyCtxt, _args: &StablePluginArgs) -> rustc_driver::Compilation {
    for item in stable_mir::all_local_items() {
        let msg = format!("There is an item \"{:?}\" of type \"{}\"", item, item.ty());
        println!("{msg}");
    }
    rustc_driver::Compilation::Continue
}
