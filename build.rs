extern crate bindgen;
extern crate cc;
extern crate failure;
extern crate wx_widgets_bindgen_helpers;

use failure::Error;
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
        .filter(|s| s.len() > 0)
        .map(|s| s.to_owned())
        .collect();
}

fn get_lib_flags(wx_config_path: &str) -> Vec<String> {
    get_config_flags(wx_config_path, &["--libs", "all"])
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
    for variable in read_passlist_lines("lists/const_passlist") {
        builder = builder.whitelist_var(variable);
    }
    for type_name in read_passlist_lines("lists/types_opaquelist") {
        builder = builder.opaque_type(type_name);
    }
    for type_name in read_passlist_lines("lists/types_passlist") {
        builder = builder.whitelist_type(type_name);
    }
    for type_name in read_passlist_lines("lists/types_blocklist") {
        builder = builder.blacklist_type(type_name);
    }
    for fn_name in read_passlist_lines("lists/fn_passlist") {
        builder = builder.whitelist_function(fn_name);
    }
    return builder;
}

fn build_wrapper(flags: &mut Iterator<Item = &String>) {
    let mut builder = cc::Build::new();
    builder.cpp(true);
    builder.file("src/wrapper.cpp");
    for flag in flags {
        builder.flag(flag);
    }
    builder.compile("wrapper");
}

fn main() -> Result<(), Error> {
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());

    let out_generated_header = out_path.join("generated_wrapper.hpp");
    let out_generated_source = out_path.join("generated_wrapper.cpp");

    println!("generating c++ wrapper code");
    wx_widgets_bindgen_helpers::generate_wrapper(
        out_generated_header.to_str().unwrap(),
        out_generated_source.to_str().unwrap(),
        "wx_widgets_bindgen_helpers/xml",
    )?;
    println!("wrapper generation complete");

    // TODO(krzentner): Configure which wx instance to use.
    let wx_config_path = "wx-config";

    let lib_flags = get_lib_flags(wx_config_path);

    for lib_flag in lib_flags.iter() {
        if lib_flag.starts_with("-l") {
            println!("cargo:rustc-link-lib={}", &lib_flag[2..]);
        }
    }

    let mut clang_flags = get_clang_flags(wx_config_path);
    clang_flags.push("-I".to_owned());
    clang_flags.push(out_path.to_str().unwrap().to_owned());

    build_wrapper(&mut clang_flags.iter().chain(lib_flags.iter()));

    let builder = add_lists(
        bindgen::Builder::default()
            .header("src/wrapper.hpp")
            .generate_comments(true)
            .rust_target(bindgen::RustTarget::Nightly)
            .emit_ir_graphviz(out_path.join("bindgen_ir.dot").to_str().unwrap().to_owned())
            .clang_args(clang_flags),
    );

    let bindings = builder.generate().expect("Unable to generate bindings");

    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");
    Ok(())
}
