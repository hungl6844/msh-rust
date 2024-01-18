mod structure;

use std::io::Cursor;
use tokio::sync::mpsc;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::time::Duration;

use base64::prelude::BASE64_STANDARD;
use base64::Engine;
use image::ImageOutputFormat;
use structure::{config::Config, protocol::parse};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::process::{Child, Command};

use crate::structure::protocol::{
    self, write_varint, Description, Players, ServerboundPackets, State, StatusJson, Version,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::try_new("config.toml")?;
    let server = Command::new(&config.java_path)
        .arg("-jar")
        .arg(&config.server_file)
        .args(&config.arguments)
        .spawn()
        .expect("failed to spawn child process");

    proxy(server, config).await?;
    Ok(())
}

#[allow(unreachable_patterns)]
async fn proxy(child: Child, config: Config) -> Result<(), Box<dyn std::error::Error>> {
    let listener =
        TcpListener::bind("127.0.0.1:".to_string() + &config.proxy_port.to_string()).await?;
    println!("listening on {}", &config.proxy_port);

    let (tx, rx) = mpsc::unbounded_channel();
    tokio::spawn(handle_suspend(
        child,
        rx,
        Duration::from_secs(config.suspend_timeout),
    ));

    'conn: loop {
        let stream = listener.accept().await;
        if let Err(e) = stream {
            eprintln!("failed to accept client: {}", e);
            continue;
        }
        let (mut client, address) = stream.unwrap();
        println!("new client connected from: {}", address);

        let mut status = protocol::State::Listening;

        loop {
            client.readable().await?;

            let mut length = [0_u8; 1];
            if client.read(&mut length).await? == 0 {
                println!("connection closed, waiting for next client");
                client.shutdown().await?;
                continue 'conn;
            };

            let mut buf = Vec::with_capacity(length[0].into());
            client.read_buf(&mut buf).await?;

            let packet = parse(&mut buf.as_slice()).await?;
            println!("{:?}", packet.clone());

            match packet {
                ServerboundPackets::Handshake { next_state, .. } => {
                    if next_state == State::Login {
                        let mut server = TcpStream::connect(
                            "localhost:".to_string() + &config.server_port.to_string(),
                        )
                        .await?;
                        server.write_all(buf.as_slice()).await?;

                        let sender = tx.clone();
                        tokio::spawn(async move {
                            let _ = sender.send(true);
                            let _ = tokio::io::copy_bidirectional(&mut server, &mut client).await;
                            let _ = sender.send(false);
                        });

                        continue 'conn;
                    }

                    status = next_state;
                }

                ServerboundPackets::PingRequest { .. } => {
                    if status != protocol::State::Status {
                        eprintln!("wrong state! state was {status:?}, should have been Status");
                        client.shutdown().await?;
                        continue 'conn;
                    }

                    client.writable().await?;
                    let packet_bytes: Vec<u8> = protocol::to_bytes(packet).await;
                    client.write_all(packet_bytes.as_slice()).await?;
                }

                ServerboundPackets::StatusRequest { .. } => {
                    let img = image::open("favicon.png")?;
                    let mut image_bytes: Cursor<Vec<u8>> = Cursor::new(vec![]);
                    img.write_to(&mut image_bytes, ImageOutputFormat::Png)?;

                    let mut buf = vec![0_u8];
                    let mut json_bytes = serde_json::ser::to_vec_pretty(&StatusJson {
                        version: Version {
                            name: "1.19.4".to_string(),
                            protocol: 762,
                        },

                        players: Players {
                            max: 100,
                            online: 0,
                            sample: None,
                        },

                        description: Description {
                            text: "hello world".to_string(),
                        },

                        favicon: "data:image/png;base64,".to_string()
                            + BASE64_STANDARD.encode(image_bytes.into_inner()).as_str(),
                        enforces_secure_chat: true,
                        previews_chat: true,
                    })?;
                    println!("{}", String::from_utf8(json_bytes.clone())?);
                    let mut json_len: Vec<u8> = vec![];
                    write_varint(&mut json_len, json_bytes.len() as i64)?;
                    buf.append(&mut json_len);
                    buf.append(&mut json_bytes);
                    println!("{buf:?}");
                    client.writable().await?;
                    let mut varint: Vec<u8> = vec![];
                    write_varint(&mut varint, buf.len() as i64)?;
                    client.write_all(&varint).await?;
                    client.write_all(buf.as_slice()).await?;
                }

                _ => {
                    return Err("uninplemented packet".into());
                }
            }
        }
    }
}

async fn handle_suspend(_child: Child, mut rx: UnboundedReceiver<bool>, interval: Duration) {
    let mut players_online = 0;
    loop {
        let result = rx.recv().await.unwrap();
        if result {
            players_online += 1;
        } else {
            players_online -= 1;
        }

        tokio::time::sleep(interval).await;

        if players_online == 0 {
            // sleep thread here
        }
    }
}
