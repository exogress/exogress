use std::convert::TryInto;
use std::mem;

use crate::tunnel::*;
use crate::Error;
use bytes::{Buf, BufMut, Bytes, BytesMut};
use futures::sink::SinkExt;
use futures::stream::TryStreamExt;
use futures::{Sink, Stream};
use tokio::io;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_util::codec::length_delimited::LengthDelimitedCodec;

fn parse_client_header(value: u64) -> Result<ClientPacket, Error> {
    let slot_val: u64 = value >> CODE_BITS_RESERVED;
    let code = (value & CODE_MASK).try_into()?;

    let client_header = match code {
        COMMON_CODE_DATA => ClientHeader::Common(CommonHeader::Data),
        COMMON_CODE_CLOSED => ClientHeader::Common(CommonHeader::Closed),
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
    let code = (value & CODE_MASK).try_into()?;

    let server_header = match code {
        COMMON_CODE_DATA => ServerHeader::Common(CommonHeader::Data),
        COMMON_CODE_CLOSED => ServerHeader::Common(CommonHeader::Closed),
        SERVER_CODE_CONNECT_REQUEST => ServerHeader::ConnectRequest,
        code => return Err(Error::UnknownCode { code }),
    };

    Ok(ServerPacket {
        header: server_header,
        slot: slot_val.try_into()?,
    })
}

fn encode_server_header(slot: Slot, header: ServerHeader) -> Result<u64, Error> {
    use crate::tunnel::CommonHeader::*;
    use crate::tunnel::ServerHeader::*;

    let slot_val: u64 = slot.into_inner().into();
    assert!(slot_val <= MAX_SLOT_NUM);

    let code = match header {
        Common(Data) => COMMON_CODE_DATA,
        Common(Closed) => COMMON_CODE_CLOSED,
        ConnectRequest { .. } => SERVER_CODE_CONNECT_REQUEST,
    };

    let res = (slot_val << CODE_BITS_RESERVED) | u64::from(code);

    Ok(res)
}

fn encode_client_header(slot: Slot, header: ClientHeader) -> Result<u64, Error> {
    use crate::tunnel::ClientHeader::*;
    use crate::tunnel::CommonHeader::*;

    let slot_val: u64 = slot.into_inner().into();
    assert!(slot_val <= MAX_HEADER_CODE);

    let code = match header {
        Common(Data) => COMMON_CODE_DATA,
        Common(Closed) => COMMON_CODE_CLOSED,
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
) -> impl Stream<Item = Result<(ClientPacket, BytesMut), Error>>
       + Sink<(ServerPacket, Bytes), Error = Error>
       + Send
       + 'static {
    length_delimited(io)
        .err_into()
        .and_then(|mut bytes| async move {
            let mut header = bytes.split_to(HEADER_BYTES);
            Ok((parse_client_header(header.get_uint(HEADER_BYTES))?, bytes))
        })
        .with(|(packet, bytes): (ServerPacket, Bytes)| async move {
            let mut out = BytesMut::with_capacity(bytes.len() + HEADER_BYTES);
            let header_bytes = encode_server_header(packet.slot, packet.header)?;

            out.put_uint(header_bytes, 3);
            out.put(bytes);

            Ok(out.freeze())
        })
}

#[inline]
pub fn client_framed(
    io: impl AsyncRead + AsyncWrite + Send + 'static,
) -> impl Stream<Item = Result<(ServerPacket, BytesMut), Error>>
       + Sink<(ClientPacket, Bytes), Error = Error>
       + Send
       + 'static {
    length_delimited(io)
        .err_into()
        .and_then(|mut bytes| async move {
            let mut header = bytes.split_to(HEADER_BYTES);
            Ok((parse_server_header(header.get_uint(3))?, bytes))
        })
        .with(|(packet, bytes): (ClientPacket, Bytes)| async move {
            let mut out = BytesMut::with_capacity(bytes.len() + HEADER_BYTES);
            let header_bytes = encode_client_header(packet.slot, packet.header)?;

            out.put_uint(header_bytes, 3);
            out.put(bytes);
            Ok(out.freeze())
        })
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::tunnel::CommonHeader;

    #[test]
    fn test_server_headers() {
        let server_headers = vec![
            ServerHeader::ConnectRequest,
            ServerHeader::Common(CommonHeader::Data),
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
            ClientHeader::Common(CommonHeader::Data),
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
