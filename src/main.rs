use ozelot::serverbound::ServerboundPacket;
use ozelot::Packet;
use ozelot::Server;
use serde::{Deserialize, Serialize};
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tokio::process::{Child, Command};
use std::process::{Stdio, exit};

#[derive(Deserialize, Serialize, Clone)]
struct Config {
    #[serde(default)]
    server_file: String,
    #[serde(default)]
    java_path: String,
    #[serde(default)]
    arguments: Vec<String>,
    #[serde(default)]
    proxy_port: u16,
    #[serde(default)]
    server_port: u16,
    #[serde(default)]
    protocol_ver: i32,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            server_file: "./server.jar".to_string(),
            java_path: "/bin/java".to_string(),
            arguments: vec!["nogui".to_string()],
            proxy_port: 25565,
            server_port: 25575,
            protocol_ver: 763,
            // this protocol version can be found in the protocol docs. it may not always be up to
            // date, so the newest versions may be difficult to use. if this field is left blank, I
            // will implement a system to pass the connect to the server directly instead of
            // refusing from the proxy.
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut file = File::options().write(true).create(true).read(true).append(true).open("config.toml").await?;
    let mut config_string = String::new();
    file.read_to_string(&mut config_string).await?;

    if config_string.is_empty() {
        let string = toml::to_string_pretty(&Config::default()).unwrap();

        file.write_all(string.as_bytes()).await?;
        file.write_all(b"# the protocol version can either be left blank to force a server start, or filled using your server's version using `https://wiki.vg/Protocol_version_numbers`. Filling the protocol version is recommended because it will allow the proxy itself to reject the connection, rather than starting the server and it having to reject the clients on the wrong versions.").await?;
        eprintln!(
            "a default config file was created. please review the file and re-run this program"
        );
        exit(1);
    }

    let config: Config =
        toml::from_str(config_string.as_str()).expect("your config file is incorrectly formatted");
    let server = Command::new(&config.java_path)
        .arg("-jar")
        .arg(&config.server_file)
        .args(&config.arguments)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("failed to spawn child process");

    proxy(&server, &config).await?;
    
    Ok(())
}

async fn proxy(_child: &Child, config: &Config) -> Result<(), Box<dyn std::error::Error>> {
    // thread::sleep(Duration::from_millis(10000));
    // println!("out of sleep!");
    let listener = TcpListener::bind("127.0.0.1:".to_string() + &config.proxy_port.to_string()).await?;
    println!("listening on {}", &config.proxy_port);
    let mut server =
        TcpStream::connect("127.0.0.1:".to_string() + &config.server_port.to_string()).await?;

    loop {
        let client = listener.accept().await;
        if let Err(e) = client {
            eprintln!("failed to accept client: {}", e);
            continue;
        }

        let (stream, address) = client.unwrap();
        println!("new client connected from: {}", address);

        let mut client = Server::from_tcpstream(stream.into_std()?).unwrap();
        let client_packets = client.read().unwrap();

        for packet in client_packets {
            match packet {
                ServerboundPacket::Handshake(ref p) => {
                    if &config.protocol_ver == p.get_protocol_version() {
                        server.write_all(p.to_u8().unwrap().as_slice()).await?;
                    } else {
                        client.close().unwrap();
                        continue;
                    }
                    dbg!(&p);
                }
                _ => {
                    server
                        .write_all(packet.to_u8().unwrap().as_slice())
                        .await?;
                    dbg!(packet);
                }
            }
        }
    }
}
