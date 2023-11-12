mod structure;

use std::process::exit;
use structure::Config;
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use crate::structure::ServerboundPackets;
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut file = File::options()
        .write(true)
        .create(true)
        .read(true)
        .append(true)
        .open("config.toml")
        .await?;
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
    /*    let server = Command::new(&config.java_path)
    .arg("-jar")
    .arg(&config.server_file)
    .args(&config.arguments)
    .stdin(Stdio::inherit())
    .stdout(Stdio::inherit())
    .stderr(Stdio::inherit())
    .spawn()
    .expect("failed to spawn child process");*/

    proxy(/*&server,*/ &config).await?;

    Ok(())
}

async fn proxy(/*child: &Child,*/ config: &Config) -> Result<(), Box<dyn std::error::Error>> {
    // thread::sleep(Duration::from_millis(10000));
    // println!("out of sleep!");
    let listener =
        TcpListener::bind("127.0.0.1:".to_string() + &config.proxy_port.to_string()).await?;
    println!("listening on {}", &config.proxy_port);

    loop {
        let client = listener.accept().await;
        if let Err(e) = client {
            eprintln!("failed to accept client: {}", e);
            continue;
        }

        let (mut stream, address) = client.unwrap();
        println!("new client connected from: {}", address);

        loop {
            stream.readable().await?;

            let len = stream.read_u8().await?;
            let mut buf = vec![0; len.into()];

            stream.read_exact(&mut buf).await?;

            let packet = ServerboundPackets::try_from(buf).unwrap();
            println!("{:?}", packet);
        }
    }
}
