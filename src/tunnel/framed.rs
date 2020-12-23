use std::convert::TryInto;
use std::mem;

use crate::tunnel::proto::*;
use crate::tunnel::Error;
use bytes::{Buf, BufMut, Bytes, BytesMut};
use futures::sink::SinkExt;
use futures::stream::TryStreamExt;
use futures::{Sink, Stream};
use tokio::io;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_util::codec::length_delimited::LengthDelimitedCodec;

fn parse_client_header(value: u64) -> Result<ClientPacket, Error> {
    let slot_val: u64 = value >> CODE_BITS_RESERVED;
    let code = (value & MAX_CODE_VALUE).try_into()?;

    let client_header = match code {
        COMMON_CODE_DATA_PLAIN => ClientHeader::Common(CommonHeader::DataPlain),
        COMMON_CODE_DATA_COMPRESSED => ClientHeader::Common(CommonHeader::DataCompressed),
        COMMON_CODE_CLOSED => ClientHeader::Common(CommonHeader::Closed),
        COMMON_CODE_PING => ClientHeader::Common(CommonHeader::Ping),
        COMMON_CODE_PONG => ClientHeader::Common(CommonHeader::Pong),
        CLIENT_CODE_ACCEPTED => ClientHeader::Accepted,
        CLIENT_CODE_REJECTED => ClientHeader::Rejected,
        code => return Err(Error::UnknownCode { code }),
    };

    Ok(ClientPacket {
        header: client_header,
        slot: slot_val.try_into()?,
    })
}

fn parse_server_header(value: u64) -> Result<ServerPacket, Error> {
    let slot_val: u64 = value >> CODE_BITS_RESERVED;
    let code = (value & MAX_CODE_VALUE).try_into()?;

    let server_header = match code {
        COMMON_CODE_DATA_PLAIN => ServerHeader::Common(CommonHeader::DataPlain),
        COMMON_CODE_DATA_COMPRESSED => ServerHeader::Common(CommonHeader::DataCompressed),
        COMMON_CODE_CLOSED => ServerHeader::Common(CommonHeader::Closed),
        COMMON_CODE_PING => ServerHeader::Common(CommonHeader::Ping),
        COMMON_CODE_PONG => ServerHeader::Common(CommonHeader::Pong),
        SERVER_CODE_CONNECT_REQUEST => ServerHeader::ConnectRequest,
        SERVER_CODE_TUNNEL_CLOSE => ServerHeader::TunnelClose,
        code => return Err(Error::UnknownCode { code }),
    };

    Ok(ServerPacket {
        header: server_header,
        slot: slot_val.try_into()?,
    })
}

fn encode_server_header(slot: Slot, header: ServerHeader) -> Result<u64, Error> {
    use crate::tunnel::proto::CommonHeader::*;
    use crate::tunnel::proto::ServerHeader::*;

    let slot_val: u64 = slot.into_inner().into();
    assert!(slot_val <= MAX_SLOT_NUM);

    let code = match header {
        Common(Ping) => COMMON_CODE_PING,
        Common(Pong) => COMMON_CODE_PONG,
        Common(DataPlain) => COMMON_CODE_DATA_PLAIN,
        Common(DataCompressed) => COMMON_CODE_DATA_COMPRESSED,
        Common(Closed) => COMMON_CODE_CLOSED,
        ConnectRequest { .. } => SERVER_CODE_CONNECT_REQUEST,
        TunnelClose { .. } => SERVER_CODE_TUNNEL_CLOSE,
    };

    let res = (slot_val << CODE_BITS_RESERVED) | u64::from(code);

    Ok(res)
}

fn encode_client_header(slot: Slot, header: ClientHeader) -> Result<u64, Error> {
    use crate::tunnel::proto::ClientHeader::*;
    use crate::tunnel::proto::CommonHeader::*;

    let slot_val: u64 = slot.into_inner().into();
    assert!(slot_val <= MAX_HEADER_CODE);

    let code = match header {
        Common(DataPlain) => COMMON_CODE_DATA_PLAIN,
        Common(DataCompressed) => COMMON_CODE_DATA_COMPRESSED,
        Common(Closed) => COMMON_CODE_CLOSED,
        Common(Ping) => COMMON_CODE_PING,
        Common(Pong) => COMMON_CODE_PONG,
        Accepted => CLIENT_CODE_ACCEPTED,
        Rejected => CLIENT_CODE_REJECTED,
    };

    let res = (slot_val << CODE_BITS_RESERVED) | u64::from(code);

    Ok(res)
}

#[inline]
fn length_delimited(
    io: impl AsyncRead + AsyncWrite,
) -> impl Stream<Item = io::Result<BytesMut>> + Sink<Bytes, Error = io::Error> {
    LengthDelimitedCodec::builder()
        .length_field_offset(0)
        .length_field_length(mem::size_of::<u16>())
        .length_adjustment(HEADER_BYTES.try_into().unwrap())
        .new_framed(io)
}

#[inline]
pub fn server_framed(
    io: impl AsyncRead + AsyncWrite + Send + 'static,
) -> impl Stream<Item = Result<(ClientPacket, Vec<u8>), Error>>
       + Sink<(ServerPacket, Vec<u8>), Error = Error>
       + Send
       + 'static {
    length_delimited(io)
        .err_into()
        .and_then(|mut bytes| async move {
            let mut header = bytes.split_to(HEADER_BYTES);
            Ok((
                parse_client_header(header.get_uint(HEADER_BYTES))?,
                bytes.to_vec(),
            ))
        })
        .with(|(packet, bytes): (ServerPacket, Vec<u8>)| async move {
            let mut out = BytesMut::with_capacity(bytes.len() + HEADER_BYTES);
            let header_bytes = encode_server_header(packet.slot, packet.header)?;

            out.put_uint(header_bytes, 3);
            out.put_slice(bytes.as_slice());

            Ok(out.freeze())
        })
}

#[inline]
pub fn client_framed(
    io: impl AsyncRead + AsyncWrite + Send + 'static,
) -> impl Stream<Item = Result<(ServerPacket, Vec<u8>), Error>>
       + Sink<(ClientPacket, Vec<u8>), Error = Error>
       + Send
       + 'static {
    length_delimited(io)
        .err_into()
        .and_then(|mut bytes| async move {
            let mut header = bytes.split_to(HEADER_BYTES);
            Ok((parse_server_header(header.get_uint(3))?, bytes.to_vec()))
        })
        .with(|(packet, bytes): (ClientPacket, Vec<u8>)| async move {
            let mut out = BytesMut::with_capacity(bytes.len() + HEADER_BYTES);
            let header_bytes = encode_client_header(packet.slot, packet.header)?;

            out.put_uint(header_bytes, 3);
            out.put_slice(bytes.as_slice());

            Ok(out.freeze())
        })
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::tunnel::proto::CommonHeader;

    #[test]
    fn test_server_headers() {
        let server_headers = vec![
            ServerHeader::ConnectRequest,
            ServerHeader::Common(CommonHeader::DataPlain),
            ServerHeader::Common(CommonHeader::Closed),
        ];

        for server_header in server_headers.into_iter() {
            let slot = 23u64.try_into().unwrap();
            let server_header_bytes = encode_server_header(slot, server_header).unwrap();
            assert!(server_header_bytes <= MAX_HEADER_CODE);
            let ServerPacket {
                header: parsed_server_header,
                slot: parsed_slot,
            } = parse_server_header(server_header_bytes).unwrap();

            assert_eq!(parsed_slot, slot);
            assert_eq!(parsed_server_header, server_header);
        }
    }

    #[test]
    fn test_client_headers() {
        let client_headers = vec![
            ClientHeader::Accepted,
            ClientHeader::Rejected,
            ClientHeader::Common(CommonHeader::DataPlain),
            ClientHeader::Common(CommonHeader::Closed),
        ];

        for client_header in client_headers.into_iter() {
            let slot = 93182u64.try_into().unwrap();
            let client_header_bytes = encode_client_header(slot, client_header).unwrap();
            assert!(client_header_bytes <= MAX_HEADER_CODE);
            let ClientPacket {
                header: parsed_client_header,
                slot: parsed_slot,
            } = parse_client_header(client_header_bytes).unwrap();

            assert_eq!(parsed_slot, slot);
            assert_eq!(parsed_client_header, client_header);
        }
    }
}
