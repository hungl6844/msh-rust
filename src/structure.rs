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

#[derive(Debug)]
pub enum ServerboundPackets {
    Handshake {
        id: u8,
        // this should be a varint, but i'll work on that later
        protocol_ver: u8,
        address: String,
        port: u16,
        next_state: State,
    },
    PingRequest {
        id: u8,
        payload: i64,
    },
}

#[derive(Debug)]
#[repr(u8)]
pub enum State {
    Status = 1,
    Login = 2,
}

impl TryFrom<Vec<u8>> for ServerboundPackets {
    type Error = &'static str;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        let Some(id) = value.first() else {
            return Err("couldn't read the first item in vector: is it empty?");
        };
        if !(0..53).contains(id) {
            return Err("the packet has an invalid packet id");
        }

        match id {
            0 => {
                let mut i = 1;

                let protocol_ver = value[i];
                i += 1;

                let str_len = value[i];
                i += 1;

                let address = String::from_utf8(value[i..(str_len as usize + i)].into()).unwrap();

                i += str_len as usize;

                let port = (value[i] as u16) << 8 | (value[i + 1] as u16);
                i += 2;

                let next_state = match value[i] {
                    1 => State::Status,
                    2 => State::Login,
                    _ => {
                        return Err("state enum incorrect");
                    }
                };

                if i + 1 != value.len() {
                    return Err("invalid packet");
                }

                Ok(ServerboundPackets::Handshake {
                    id: *id,
                    protocol_ver,
                    address,
                    port,
                    next_state,
                })
            }

            1 => {
                let payload = ((value[1] as u64) << 56
                    | (value[2] as u64) << 48
                    | (value[3] as u64) << 40
                    | (value[4] as u64) << 32
                    | (value[5] as u64) << 24
                    | (value[6] as u64) << 16
                    | (value[7] as u64) << 8
                    | (value[8] as u64)) as i64;

                Ok(ServerboundPackets::PingRequest { id: *id, payload })
            }

            _ => {
                // this is a scenario that I don't want to think about because I might have to
                // redesign this entire project over it
                Err("this packet is unimplemented")
            }
        }
    }
}

impl From<ServerboundPackets> for Vec<u8> {
    fn from(value: ServerboundPackets) -> Vec<u8> {
        let mut byte_array: Vec<u8> = vec![];

        match value {
            ServerboundPackets::Handshake {
                id,
                protocol_ver,
                address,
                port,
                next_state,
            } => {
                byte_array.push(id);
                byte_array.push(protocol_ver);
                byte_array.push(address.len() as u8);
                byte_array.append(&mut address.as_bytes().to_owned());
                byte_array.push((port >> 8) as u8);
                byte_array.push((port & 0xff) as u8);
                byte_array.push(next_state.into());
            }

            ServerboundPackets::PingRequest { id, payload } => {
                byte_array.push(id);
                let mut paylod_arr: Vec<u8> = payload.to_be_bytes().into();
                byte_array.append(&mut paylod_arr);
            }
        };

        byte_array
    }
}

impl From<State> for u8 {
    fn from(value: State) -> Self {
        value as u8
    }
}
