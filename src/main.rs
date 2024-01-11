mod structure;

use std::io::Cursor;

use base64::Engine;
use base64::prelude::BASE64_STANDARD;
use image::ImageOutputFormat;
use structure::{config::Config, protocol::parse};
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::process::{Command, Child};
use tokio::select;

use crate::structure::protocol::{self, ServerboundPackets, StatusJson, Version, Players, Description, write_varint, State};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::try_new("config.toml")?;
    let server = Command::new(&config.java_path)
    .arg("-jar")
    .arg(&config.server_file)
    .args(&config.arguments)
    .spawn()
    .expect("failed to spawn child process");

    proxy(&server, config).await?;
    Ok(())
}

#[allow(unreachable_patterns)] async fn proxy(child: &Child, config: Config) -> Result<(), Box<dyn std::error::Error>> {
    let listener =
        TcpListener::bind("127.0.0.1:".to_string() + &config.proxy_port.to_string()).await?;
    println!("listening on {}", &config.proxy_port);

    'conn: loop {
        let client = listener.accept().await;
        if let Err(e) = client {
            eprintln!("failed to accept client: {}", e);
            continue;
        }
        let (mut stream, address) = client.unwrap();
        println!("new client connected from: {}", address);

        let mut status = protocol::State::Listening;

        loop {
            stream.readable().await?;

            let mut length = [0_u8; 1];
            if stream.read(&mut length).await? == 0 {
                println!("connection closed, waiting for next client");
                stream.shutdown().await?;
                continue 'conn;
            };

            let mut buf = Vec::with_capacity(length[0].into());
            stream.read_buf(&mut buf).await?;

            let packet = parse(&mut buf.as_slice()).await?;
            println!("{:?}", packet.clone());

            match packet {
                ServerboundPackets::Handshake { next_state, .. } => {
                    if next_state == State::Login {
                        tokio::spawn(async move {
                            let mut server = TcpStream::connect("localhost:".to_string() + &config.server_port.to_string()).await?;
                            server.write_all(buf.as_slice()).await?;

                            tokio::spawn(async move {
                                let (mut client_read, mut client_write) = tokio::io::split(stream);
                                let (mut server_read, mut server_write) = tokio::io::split(server);
                                
                                tokio::join!(
                                    async {
                                        loop {
                                            let length = match client_read.read_u8().await {
                                                Ok(l) => l,
                                                Err(_) => break
                                            };

                                            let mut buf: Vec<u8> = ;
                                        }
                                    }
                                );
                            });

                            Ok::<(), tokio::io::Error>(())
                        });
                        continue 'conn;
                    }

                    status = next_state;
                }

                ServerboundPackets::PingRequest { .. } => {
                    if status != protocol::State::Status {
                        eprintln!("wrong state! state was {status:?}, should have been Status");
                        stream.shutdown().await?;
                        continue 'conn;
                    }

                    stream.writable().await?;
                    let packet_bytes: Vec<u8> = protocol::to_bytes(packet).await;
                    stream.write_all(packet_bytes.as_slice()).await?;
                }

                ServerboundPackets::StatusRequest { .. } => {
                    let img = image::open("favicon.png")?;
                    let mut image_bytes: Cursor<Vec<u8>> = Cursor::new(vec![]);
                    img.write_to(&mut image_bytes, ImageOutputFormat::Png)?;

                    let mut buf = vec![0_u8];
                    let mut json_bytes = serde_json::ser::to_vec_pretty(&StatusJson {
                        version: Version {
                            name: "1.19.4".to_string(),
                            protocol: 762
                        },

                        players: Players {
                            max: 100,
                            online: 0,
                            sample: None
                        },

                        description: Description {
                            text: "hello world".to_string()
                        },

                        favicon: "data:image/png;base64,".to_string() + BASE64_STANDARD.encode(image_bytes.into_inner()).as_str(),
                        enforces_secure_chat: true,
                        previews_chat: true
                    })?;
                    println!("{}", String::from_utf8(json_bytes.clone())?);
                    let mut json_len: Vec<u8> = vec![];
                    write_varint(&mut json_len, json_bytes.len() as i64)?;
                    buf.append(&mut json_len);
                    buf.append(&mut json_bytes);
                    println!("{buf:?}");
                    stream.writable().await?;
                    let mut varint: Vec<u8> = vec![];
                    write_varint(&mut varint, buf.len() as i64)?;
                    stream.write_all(&varint).await?;
                    stream.write_all(buf.as_slice()).await?;
                }

                _ => {
                    return Err("uninplemented packet".into());
                }
            }
        }
    }
}
