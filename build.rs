extern crate bindgen;

use std::env;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::Command;

fn get_config_flags(wx_config_path: &str, args: &[&str]) -> Vec<String> {
    let wx_config_output = Command::new(wx_config_path)
        .args(args)
        .output()
        .expect("failed to execute process");
    let clang_flags = String::from_utf8(wx_config_output.stdout).expect("invalid utf8");
    return clang_flags
        .trim()
        .split(' ')
        .map(|s| s.to_owned())
        .collect();
}

fn get_lib_flags(wx_config_path: &str) -> Vec<String> {
    get_config_flags(wx_config_path, &["--libs"])
}

fn get_clang_flags(wx_config_path: &str) -> Vec<String> {
    get_config_flags(wx_config_path, &["--cppflags"])
}

fn read_passlist_lines(path: &str) -> Vec<String> {
    let file = File::open(path).expect("could not open passlist");
    let mut output = Vec::with_capacity(8192);
    for line in BufReader::new(file).lines() {
        let line = line.expect("passlist should have lines");
        let line = line.trim();
        if !line.starts_with('#') && line.len() > 0 {
            output.push(line.to_owned());
        }
    }
    return output;
}

fn add_lists(mut builder: bindgen::Builder) -> bindgen::Builder {
    for variable in read_passlist_lines("const_passlist") {
        builder = builder.whitelist_var(variable);
    }
    for type_name in read_passlist_lines("types_opaquelist") {
        builder = builder.opaque_type(type_name);
    }
    for type_name in read_passlist_lines("types_passlist") {
        builder = builder.whitelist_type(type_name);
    }
    for type_name in read_passlist_lines("types_blocklist") {
        builder = builder.blacklist_type(type_name);
    }
    for fn_name in read_passlist_lines("fn_passlist") {
        builder = builder.whitelist_function(fn_name);
    }
    return builder;
}

fn main() {
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    // TODO(krzentner): Configure which wx instance to use.
    let wx_config_path = "wx-config";

    for lib_flag in get_lib_flags(wx_config_path) {
        if lib_flag.starts_with("-l") {
            println!("cargo:rustc-link-lib={}", &lib_flag[2..]);
        }
    }

    let clang_flags = get_clang_flags(wx_config_path);

    let builder = add_lists(
        bindgen::Builder::default()
            .header("wrapper.hpp")
            .generate_comments(true)
            .rust_target(bindgen::RustTarget::Nightly)
            .emit_ir_graphviz(out_path.join("bindgen_ir.dot").to_str().unwrap().to_owned())
            .clang_args(clang_flags),
    );

    let bindings = builder.generate().expect("Unable to generate bindings");

    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");
}
