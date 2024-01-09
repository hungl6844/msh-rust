use std::{
    error,
    fs::File,
    io::{Read, Write},
};

use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Clone)]
#[serde(default)]
pub struct Config {
    pub server_file: String,
    pub java_path: String,
    pub arguments: Vec<String>,
    pub proxy_port: u16,
    pub server_port: u16,
    pub protocol_ver: i32,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            server_file: "./server.jar".to_string(),
            java_path: "/bin/java".to_string(),
            arguments: vec!["nogui".to_string()],
            proxy_port: 25565,
            server_port: 25575,
            protocol_ver: 763, // this protocol version can be found in the protocol docs. it may not always be up to
                               // date, so the newest versions may be difficult to use. if this field is left blank, I
                               // will implement a system to pass the connect to the server directly instead of
                               // refusing from the proxy.
        }
    }
}

impl Config {
    pub fn try_new(path: &str) -> Result<Config, Box<dyn error::Error>> {
        let mut file = File::options()
            .read(true)
            .write(true)
            .create(true)
            .open(path)?;

        let mut config_str = "".to_string();
        file.read_to_string(&mut config_str)?;

        if config_str.is_empty() {
            file.write_all(toml::to_string(&Config::default())?.as_bytes())?;
            file.write_all("# the protocol version can either be left blank to force a server start, or filled using your server's version using `https://wiki.vg/Protocol_version_numbers`. Filling the protocol version is recommended because it will allow the proxy itself to reject the connection, rather than starting the server and it having to reject the clients on the wrong versions.".as_bytes())?;
            return Err("your config file did not exist. a default config file has been created. please review the fields and make any changes nessecary.".into());
        }

        Ok(toml::from_str(&config_str)?)
    }
}
