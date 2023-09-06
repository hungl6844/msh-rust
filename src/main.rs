use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{Read, Write};
use std::process::{Command, exit, Stdio};

#[derive(Deserialize, Serialize)]
struct Config {
    #[serde(default)]
    server_file: String,
    #[serde(default)]
    java_path: String,
    #[serde(default)]
    arguments: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            server_file: "./server.jar".to_string(),
            java_path: "/bin/java".to_string(),
            arguments: vec!["nogui".to_string()],
        }
    }
}

fn main() {
    let mut file = File::options().write(true).create(true).read(true).open("config.toml")
        .expect("your config file was unable to be created or does not exist. please make sure you have rights to create files in the current directory.");
    let mut config_string = String::new();
    file.read_to_string(&mut config_string).unwrap();

    if config_string.is_empty() {
        let string = toml::to_string_pretty(&Config::default()).unwrap();

        file.write_all(string.as_bytes()).unwrap();
        eprintln!(
            "a default config file was created. please review the file and re-run this program"
        );
        exit(1);
    }

    let config: Config = toml::from_str(config_string.as_str()).expect("your config file is incorrectly formatted");
    let _server = Command::new(config.java_path)
        .arg("-jar")
        .arg(config.server_file)
        .args(config.arguments)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn child process");


    println!("hello world");
}
