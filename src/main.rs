extern crate unindent;
extern crate rustc_version;
extern crate clap;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate toml;

use std::fs::File;
use std::io::prelude::*;
use std::process::{Command, Stdio};

use clap::{App, SubCommand};

#[derive(Debug, Deserialize)]
struct CargoMetadataDocker {
    repo: String,
    expose: u32,
}

#[derive(Debug, Deserialize)]
struct CargoMetadata {
    docker: CargoMetadataDocker,
}

#[derive(Debug, Deserialize)]
struct CargoPackage {
    name: String,
    version: String,
    metadata: CargoMetadata,
}

#[derive(Debug, Deserialize)]
struct CargoToml {
    package: CargoPackage,
}

fn get_cargo_package() -> CargoPackage {
    let mut cargo_toml = File::open("Cargo.toml").expect("can't read Cargo.toml");
    let mut contents = String::new();
    cargo_toml.read_to_string(&mut contents).expect(
        "can't read Cargo.toml",
    );
    let cargo_config = toml::from_str::<CargoToml>(&contents).expect("can't parse Cargo.toml");
    cargo_config.package
}

fn generate() {
    let cargo_package = get_cargo_package();

    let rust_version = rustc_version::version()
        .expect("can't get rustc version")
        .to_string();
    let dockerfile_content = unindent::unindent(&format!(
        "
        FROM jimmycuadra/rust:{0} AS builder
        WORKDIR /app
        ADD . /app/
        RUN cargo build --release

        FROM debian
        WORKDIR /
        COPY --from=builder /app/target/release/{1} /{1}-{2}
        EXPOSE {3}
        CMD [\"./{1}-{2}\"]
        ",
        rust_version,
        cargo_package.name,
        cargo_package.version,
        cargo_package.metadata.docker.expose,
    ));

    let mut dockerfile = File::create("Dockerfile").expect("can't create Dockerfile");
    dockerfile
        .write_fmt(format_args!("{}", dockerfile_content))
        .expect("can't write Dockerfile");
    println!("write Dockerfile");

    let mut dockerignore = File::create(".dockerignore").expect("can't create .dockerignore");
    let dockerignore_content = unindent::unindent(
        "
        .git
        */.git
        */*/.git

        .gitignore
        */.gitignore
        */*/.gitignore
        
        target
        */target
        */*/target

        Dockerfile
        .dockerignore
    ",
    );
    dockerignore
        .write_fmt(format_args!("{}", dockerignore_content))
        .expect("can't write .dockerignore");
    println!("write .dockerignore");
}

fn get_docker_tag() -> String {
    let cargo_package = get_cargo_package();
    format!(
        "{}:{}",
        cargo_package.metadata.docker.repo,
        cargo_package.version
    )
}

fn run_docker_command(modify: &Fn(&mut Command) -> &Command) -> () {
    let mut cmd = Command::new("docker");
    modify(&mut cmd);

    let exit_status = cmd.stdin(Stdio::inherit())
        .stderr(Stdio::inherit())
        .stdout(Stdio::inherit())
        .status()
        .expect("can't start docker process");

    if !exit_status.success() {
        std::process::exit(1);
    }
}

fn build(cmd: &mut Command) -> &Command {
    cmd.arg("build").arg("-t").arg(get_docker_tag()).arg(".")
}

fn push(cmd: &mut Command) -> &Command {
    cmd.arg("push").arg(get_docker_tag())
}

fn main() {
    let matches = App::new("cargo dock")
        .version(env!("CARGO_PKG_VERSION"))
        .subcommand(
            SubCommand::with_name("dock")
                .subcommand(SubCommand::with_name("generate").about(
                    "Generate Dockerfile and .dockerignore",
                ))
                .subcommand(SubCommand::with_name("build").about("Build docker image"))
                .subcommand(SubCommand::with_name("push").about("Push docker image")),
        )
        .get_matches();

    let command = matches.subcommand.and_then(|c| c.matches.subcommand);

    match command {
        Some(command) => {
            if command.name == "generate" {
                generate()
            } else if command.name == "build" {
                run_docker_command(&build)
            } else if command.name == "push" {
                run_docker_command(&push)
            }
        }
        None => println!("{}", matches.usage.unwrap()),
    }
}