mod structure;

use structure::{config::Config, protocol::parse};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use crate::structure::protocol::{self, ServerboundPackets, StatusJson, Version, Players, Sample, Description, write_varint};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /*    let server = Command::new(&config.java_path)
    .arg("-jar")
    .arg(&config.server_file)
    .args(&config.arguments)
    .stdin(Stdio::inherit())
    .stdout(Stdio::inherit())
    .stderr(Stdio::inherit())
    .spawn()
    .expect("failed to spawn child process");*/

    proxy(/*&server,*/ &Config::try_new("config.toml")?).await?;
    Ok(())
}

async fn proxy(/*child: &Child,*/ config: &Config) -> Result<(), Box<dyn std::error::Error>> {
    // thread::sleep(Duration::from_millis(10000));
    // println!("out of sleep!");
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

                        favicon: "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAEAAAABACAQAAAAAYLlVAAAAPElEQVR42u3OMQEAAAgDIJfc6BpjDyQgt1MVAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBgXbgARTAX8ECcrkoAAAAAElFTkSuQmCC".to_string(),
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
