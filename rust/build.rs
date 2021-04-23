use std::{env::var_os, ffi::OsString, fs::create_dir_all, path::PathBuf, process::Command};

// https://gitlab.com/tspiteri/gmp-mpfr-sys/-/blob/master/build.rs

struct Environment {
    arb_branch: OsString,
    build_dir: PathBuf,
    flint_branch: OsString,
    gmp_dir: PathBuf,
    include_dir: PathBuf,
    lib_dir: PathBuf,
    makeflags: OsString,
    out_dir: PathBuf,
}

fn main() {
    if var_os("CARGO_FEATURE_ARB").is_none() {
        return;
    }

    let out_dir = PathBuf::from(var_os("OUT_DIR").unwrap());
    let env = Environment {
        arb_branch: var_os("ARB_BRANCH").unwrap_or_else(|| "master".into()),
        build_dir: out_dir.join("build"),
        flint_branch: var_os("FLINT_BRANCH").unwrap_or_else(|| "trunk".into()),
        gmp_dir: PathBuf::from(var_os("DEP_GMP_OUT_DIR").unwrap()),
        include_dir: out_dir.join("include"),
        lib_dir: out_dir.join("lib"),
        makeflags: var_os("CARGO_MAKEFLAGS").unwrap(),
        out_dir: out_dir.clone(),
    };
    create_dir_all(&env.build_dir).unwrap_or_else(|_| {
        panic!(
            "failed to create a directory: {}",
            env.build_dir.to_string_lossy()
        )
    });

    build_flint(&env);
    build_arb(&env);
    run_bindgen(&env);
    write_link_info(&env);
}

fn build_flint(env: &Environment) {
    if env.lib_dir.join("libflint.a").exists() {
        return;
    }

    let build_dir = env.build_dir.join("flint-build");

    if !build_dir.exists() {
        execute(Command::new("git").current_dir(&env.build_dir).args(&[
            "clone",
            "--branch",
            &env.flint_branch.to_str().unwrap(),
            "--depth",
            "1",
            "https://github.com/fredrik-johansson/flint2.git",
            build_dir.to_str().unwrap(),
        ]));
    }
    execute(
        Command::new("sh")
            .current_dir(&build_dir)
            .arg("-c")
            .arg(
                [
                    "./configure",
                    " --prefix=",
                    env.out_dir.to_str().unwrap(),
                    " --with-gmp=",
                    env.gmp_dir.to_str().unwrap(),
                    " --with-mpfr=",
                    env.gmp_dir.to_str().unwrap(),
                ]
                .concat(),
            )
            .env("CFLAGS", "-Wno-error"),
    );
    execute(
        Command::new("make")
            .current_dir(&build_dir)
            .env("MAKEFLAGS", &env.makeflags),
    );
    execute(
        Command::new("make")
            .current_dir(&build_dir)
            .arg("check")
            .env("MAKEFLAGS", &env.makeflags),
    );
    execute(Command::new("make").current_dir(&build_dir).arg("install"));
}

fn build_arb(env: &Environment) {
    if env.lib_dir.join("libarb.a").exists() {
        return;
    }

    let build_dir = env.build_dir.join("arb-build");
    if !build_dir.exists() {
        execute(Command::new("git").current_dir(&env.build_dir).args(&[
            "clone",
            "--branch",
            &env.arb_branch.to_str().unwrap(),
            "--depth",
            "1",
            "https://github.com/fredrik-johansson/arb.git",
            build_dir.to_str().unwrap(),
        ]));
    }
    execute(
        Command::new("sh")
            .current_dir(&build_dir)
            .arg("-c")
            .arg(
                [
                    "./configure",
                    " --prefix=",
                    env.out_dir.to_str().unwrap(),
                    " --with-gmp=",
                    env.gmp_dir.to_str().unwrap(),
                    " --with-mpfr=",
                    env.gmp_dir.to_str().unwrap(),
                    " --with-flint=",
                    env.out_dir.to_str().unwrap(),
                ]
                .concat(),
            )
            .env("CFLAGS", "-Wno-error"),
    );
    execute(
        Command::new("make")
            .current_dir(&build_dir)
            .env("MAKEFLAGS", &env.makeflags),
    );
    execute(Command::new("make").current_dir(&build_dir).arg("install"));
    execute(
        Command::new("make")
            .current_dir(&build_dir)
            .arg("check")
            .env("ARB_TEST_MULTIPLIER", "0.1")
            .env("MAKEFLAGS", &env.makeflags),
    );
}

fn run_bindgen(env: &Environment) {
    let binding_file = env.out_dir.join("arb_sys.rs");
    if binding_file.exists() {
        return;
    }
    bindgen::Builder::default()
        .header(env.include_dir.join("acb.h").to_str().unwrap())
        .header(env.include_dir.join("acb_elliptic.h").to_str().unwrap())
        .header(env.include_dir.join("arb.h").to_str().unwrap())
        .header(env.include_dir.join("arb_hypgeom.h").to_str().unwrap())
        .header(env.include_dir.join("arf.h").to_str().unwrap())
        .header(env.include_dir.join("mag.h").to_str().unwrap())
        .allowlist_function("(acb_|arb_|arf_|mag_).*")
        .clang_args(&[
            "-DACB_INLINES_C",
            "-DARB_INLINES_C",
            "-DARF_INLINES_C",
            "-DMAG_INLINES_C",
            "-I",
            env.gmp_dir.join("include").to_str().unwrap(),
        ])
        .generate()
        .expect("failed to generate bindings")
        .write_to_file(binding_file.clone())
        .unwrap_or_else(|_| {
            panic!(
                "failed to write to a file: {}",
                binding_file.to_string_lossy()
            )
        });
}

fn write_link_info(env: &Environment) {
    println!(
        "cargo:rustc-link-search=native={}",
        env.gmp_dir.join("lib").to_str().unwrap()
    );
    println!("cargo:rustc-link-lib=static=gmp");
    println!("cargo:rustc-link-lib=static=mpfr");
    println!(
        "cargo:rustc-link-search=native={}",
        env.lib_dir.to_str().unwrap()
    );
    println!("cargo:rustc-link-lib=static=flint");
    println!("cargo:rustc-link-lib=static=arb");
}

fn execute(cmd: &mut Command) {
    let status = cmd
        .status()
        .unwrap_or_else(|_| panic!("failed to run the command: {:?}", cmd));
    if !status.success() {
        if let Some(code) = status.code() {
            panic!("command exited with code {}: {:?}", code, cmd);
        } else {
            panic!("command failed: {:?}", cmd);
        }
    }
}
