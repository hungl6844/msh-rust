use std::{error, fmt::Debug, io};
use tokio::io::AsyncReadExt;

const SEGMENT_BITS: u8 = 0x7F;
const CONTINUE_BIT: u8 = 0x80;

#[derive(Debug, Clone)]
pub enum ServerboundPackets {
    Handshake {
        id: u8,
        // this should be a varint, but i'll work on that later
        protocol_ver: i32,
        address: String,
        port: u16,
        next_state: State, // TODO: varint enum, write_varint
    },
    StatusRequest {
        id: u8,
    },
    PingRequest {
        id: u8,
        payload: i64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[repr(u8)]
pub enum State {
    Listening = 0,
    Status = 1,
    Login = 2,
}

pub async fn parse<R>(value: &mut R) -> Result<ServerboundPackets, Box<dyn error::Error>>
where
    R: AsyncReadExt + std::marker::Unpin + Debug + ?Sized,
{
    let id = value.read_u8().await?;

    if !(0..53).contains(&id) {
        return Err("the packet has an invalid packet id".into());
    }

    match id {
        0 => {
            let mut vec_buf = vec![];
            value.read_to_end(&mut vec_buf).await?;

            let mut buffer = vec_buf.as_slice();

            if buffer.is_empty() {
                return Ok(ServerboundPackets::StatusRequest { id: 0 });
            }

            let protocol_ver = read_varint(&mut buffer).await?;

            let mut str_buf = vec![u8::default(); buffer.read_u8().await?.into()];
            buffer.read_exact(&mut str_buf).await?;

            let address = String::from_utf8(str_buf)?;

            let port = (buffer.read_u8().await? as u16) << 8 | (buffer.read_u8().await? as u16);

            let next_state = match buffer.read_u8().await? {
                1 => State::Status,
                2 => State::Login,
                _ => {
                    return Err("state enum incorrect".into());
                }
            };

            let size = buffer.len();
            if size != 0 {
                return Err((format!(
                    "invalid packet: size was {size}, the vector still had {buffer:?}"
                ))
                .into());
            }

            Ok(ServerboundPackets::Handshake {
                id,
                protocol_ver,
                address,
                port,
                next_state,
            })
        }

        1 => {
            let payload = ((value.read_u8().await? as u64) << 56
                | (value.read_u8().await? as u64) << 48
                | (value.read_u8().await? as u64) << 40
                | (value.read_u8().await? as u64) << 32
                | (value.read_u8().await? as u64) << 24
                | (value.read_u8().await? as u64) << 16
                | (value.read_u8().await? as u64) << 8
                | (value.read_u8().await? as u64)) as i64;

            Ok(ServerboundPackets::PingRequest { id, payload })
        }

        _ => {
            // this is a scenario that I don't want to think about because I might have to
            // redesign this entire project over it
            Err("this packet is unimplemented".into())
        }
    }
}

pub async fn to_bytes(value: ServerboundPackets) -> Vec<u8> {
    let mut byte_array: Vec<u8> = vec![];

    match value {
        ServerboundPackets::Handshake {
            id,
            protocol_ver,
            address,
            port,
            next_state,
        } => {
            // id + protocol_ver + address length + address + port byte 1 + port byte 2 + next state
            // protocol_ver may not always work, due to the lack of varint support right now
            byte_array.push(1 + 1 + 1 + address.len() as u8 + 1 + 1 + 1);
            byte_array.push(id);
            byte_array.push(protocol_ver as u8); // this will always work, however I need to work on
                                                 // a write_varint implementation
            byte_array.push(address.len() as u8);
            byte_array.append(&mut address.as_bytes().to_owned());
            byte_array.push((port >> 8) as u8);
            byte_array.push((port & 0xff) as u8);
            byte_array.push(next_state.into());
        }

        ServerboundPackets::PingRequest { id, payload } => {
            // id + payload (payload is always 8 bytes)
            byte_array.push(1 + 8);
            byte_array.push(id);
            let mut paylod_arr: Vec<u8> = payload.to_be_bytes().into();
            byte_array.append(&mut paylod_arr);
        }

        ServerboundPackets::StatusRequest { id } => {
            byte_array.push(1);
            byte_array.push(id);
        }
    };

    byte_array
}

impl From<State> for u8 {
    fn from(value: State) -> Self {
        value as u8
    }
}

pub async fn read_varint<R>(reader: &mut R) -> Result<i32, Box<dyn error::Error>>
where
    R: tokio::io::AsyncReadExt + std::marker::Unpin,
{
    let mut varint: i32 = 0;
    let mut pos = 0;
    loop {
        let byte = reader.read_u8().await?;
        varint |= ((SEGMENT_BITS & byte) as i32) << (pos);

        if (byte & CONTINUE_BIT) == 0 {
            break;
        }

        pos += 7;

        if pos >= 32 {
            return Err("varint too long: packet is most likely invalid".into());
        }
    }

    Ok(varint)
}

pub fn write_varint<W>(w: &mut W, mut val: i64) -> Result<usize, io::Error>
where
    W: ?Sized + io::Write,
{
    let mut bytes_written = 0;
    loop {
        let mut byte = val as u8;
        val >>= 6;
        let done = val == 0 || val == -1;
        if done {
            byte &= !CONTINUE_BIT;
        } else {
            val >>= 1;
            byte |= CONTINUE_BIT;
        }

        let buf = [byte];
        w.write_all(&buf)?;
        bytes_written += 1;

        if done {
            return Ok(bytes_written);
        }
    }
}
