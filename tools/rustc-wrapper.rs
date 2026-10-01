use std::{env, process::{Command, exit}};
fn main() {
    let mut args = env::args_os();
    let _self = args.next();
    let rustc = args.next().expect("rustc path");
    let status = Command::new(rustc).arg("-Zunstable-options").args(args).status().expect("run rustc");
    exit(status.code().unwrap_or(1));
}

