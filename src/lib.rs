// SPDX-FileCopyrightText: © 2024 Foundation Devices, Inc. <hello@foundation.xyz>
// SPDX-License-Identifier: GPL-3.0-or-later

#![no_std]

mod error;

pub use error::{Error, Result};

#[cfg(feature = "alloc")]
extern crate alloc;
#[cfg(feature = "alloc")]
use alloc::{format, string::String, vec::Vec};
#[cfg(not(feature = "alloc"))]
use heapless::Vec;

/// Wire type name of the CBOR external record.
#[cfg(feature = "cbor")]
const CBOR_TYPE: &str = "cbor.io:cbor";

/// Longest type or ID field an NDEF record can carry, both being announced by
/// a single length octet.
#[cfg(not(feature = "alloc"))]
const MAX_FIELD_LEN: usize = u8::MAX as usize;

/// Buffer holding serialized bytes. Without `alloc` the capacity is fixed and
/// the serializer reports [`Error::BufferTooSmall`] once it is exhausted.
#[cfg(feature = "alloc")]
pub type Buffer = Vec<u8>;
#[cfg(not(feature = "alloc"))]
pub type Buffer = Vec<u8, 256>;

#[cfg(feature = "alloc")]
fn write_all<'a>(buf: &mut Buffer, data: &[u8]) -> Result<'a, ()> {
    buf.extend_from_slice(data);
    Ok(())
}
#[cfg(not(feature = "alloc"))]
fn write_all<'a>(buf: &mut Buffer, data: &[u8]) -> Result<'a, ()> {
    buf.extend_from_slice(data)
        .map_err(|_| Error::BufferTooSmall)
}

fn write_u8<'a>(buf: &mut Buffer, byte: u8) -> Result<'a, ()> {
    write_all(buf, &[byte])
}

#[derive(Clone, Debug, PartialEq)]
pub enum TypeNameFormat {
    Empty,
    NfcWellKnown,
    Media,
    AbsoluteUri,
    NfcExternal,
    Unknown,
    Unchanged,
    Reserved,
}

#[derive(Clone, Debug, Default, PartialEq)]
struct Header(u8);

impl Header {
    // fn message_begin(&self) -> bool {
    //     self.0 & 0x80 == 0x80
    // }
    fn set_message_begin(&mut self) {
        self.0 |= 0x80;
    }

    // fn message_end(&self) -> bool {
    //     self.0 & 0x40 == 0x40
    // }
    fn set_message_end(&mut self) {
        self.0 |= 0x40;
    }
    fn clr_message_end(&mut self) {
        self.0 &= !0x40;
    }

    // fn message_chunk(&self) -> bool {
    //     self.0 & 0x20 == 0x20
    // }

    fn short_record(&self) -> bool {
        self.0 & 0x10 == 0x10
    }
    fn set_short_record(&mut self) {
        self.0 |= 0x10;
    }

    fn id_length(&self) -> bool {
        self.0 & 0x08 == 0x08
    }
    fn set_id_length(&mut self) {
        self.0 |= 0x08;
    }

    fn type_name_format(&self) -> TypeNameFormat {
        match self.0 & 0x07 {
            0 => TypeNameFormat::Empty,
            1 => TypeNameFormat::NfcWellKnown,
            2 => TypeNameFormat::Media,
            3 => TypeNameFormat::AbsoluteUri,
            4 => TypeNameFormat::NfcExternal,
            5 => TypeNameFormat::Unknown,
            6 => TypeNameFormat::Unchanged,
            7 => TypeNameFormat::Reserved,
            _ => unreachable!(),
        }
    }
    fn set_type_name_format(&mut self, tnf: TypeNameFormat) {
        self.0 &= !0x70;
        self.0 |= match tnf {
            TypeNameFormat::Empty => 0x00,
            TypeNameFormat::NfcWellKnown => 0x01,
            TypeNameFormat::Media => 0x02,
            TypeNameFormat::AbsoluteUri => 0x03,
            TypeNameFormat::NfcExternal => 0x04,
            TypeNameFormat::Unknown => 0x05,
            TypeNameFormat::Unchanged => 0x06,
            TypeNameFormat::Reserved => 0x07,
        };
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum RecordType<'a> {
    #[cfg(not(feature = "alloc"))]
    Text { enc: &'a str, txt: &'a str },
    #[cfg(feature = "alloc")]
    Text { enc: &'a str, txt: String },
    External {
        domain: &'a str,
        type_: &'a str,
        data: &'a [u8],
    },
    #[cfg(all(feature = "cbor", not(feature = "alloc")))]
    Cbor(&'a [u8]),
    #[cfg(all(feature = "cbor", feature = "alloc"))]
    Cbor(Vec<u8>),
}

impl<'a> RecordType<'a> {
    fn len(&self) -> usize {
        match self {
            RecordType::Text { enc, txt } => 1 + enc.len() + txt.len(),
            RecordType::External { data, .. } => data.len(),
            #[cfg(feature = "cbor")]
            RecordType::Cbor(data) => data.len(),
        }
    }

    fn write(&self, buf: &mut Buffer) -> Result<'a, ()> {
        match self {
            RecordType::Text { enc, txt } => {
                // force utf-8 encoding here
                write_u8(buf, enc.len() as u8)?;
                write_all(buf, enc.as_bytes())?;
                write_all(buf, txt.as_bytes())
            }
            RecordType::External { data, .. } => write_all(buf, data),
            #[cfg(feature = "cbor")]
            RecordType::Cbor(data) => write_all(buf, data),
        }
    }

    /// Length of the wire type name.
    fn type_len(&self) -> usize {
        match self {
            RecordType::Text { .. } => 1,
            RecordType::External { domain, type_, .. } => domain.len() + 1 + type_.len(),
            #[cfg(feature = "cbor")]
            RecordType::Cbor(_) => CBOR_TYPE.len(),
        }
    }

    fn write_type(&self, buf: &mut Buffer) -> Result<'a, ()> {
        match self {
            RecordType::Text { .. } => write_all(buf, b"T"),
            RecordType::External { domain, type_, .. } => {
                write_all(buf, domain.as_bytes())?;
                write_u8(buf, b':')?;
                write_all(buf, type_.as_bytes())
            }
            #[cfg(feature = "cbor")]
            RecordType::Cbor(_) => write_all(buf, CBOR_TYPE.as_bytes()),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Payload<'a> {
    RTD(RecordType<'a>),
}

impl<'a> From<&Payload<'a>> for TypeNameFormat {
    fn from(pl: &Payload<'a>) -> TypeNameFormat {
        match pl {
            Payload::RTD(RecordType::External { .. }) => TypeNameFormat::NfcExternal,
            #[cfg(feature = "cbor")]
            Payload::RTD(RecordType::Cbor(_)) => TypeNameFormat::NfcExternal,
            Payload::RTD(_) => TypeNameFormat::NfcWellKnown,
        }
    }
}

impl<'a> Payload<'a> {
    fn len(&self) -> usize {
        match self {
            Payload::RTD(rtd) => rtd.len(),
        }
    }

    fn write(&self, buf: &mut Buffer) -> Result<'a, ()> {
        match self {
            Payload::RTD(rtd) => rtd.write(buf),
        }
    }

    fn type_len(&self) -> usize {
        match self {
            Payload::RTD(rtd) => rtd.type_len(),
        }
    }

    fn write_type(&self, buf: &mut Buffer) -> Result<'a, ()> {
        match self {
            Payload::RTD(rtd) => rtd.write_type(buf),
        }
    }

    #[cfg(feature = "dcbor")]
    pub fn from_cbor_encodable<T>(x: &T) -> Self
    where
        T: dcbor::CBOREncodable,
    {
        Payload::RTD(RecordType::Cbor(x.to_cbor_data()))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Record<'a> {
    header: Header,
    id: Option<&'a [u8]>,
    pub payload: Payload<'a>,
}

impl<'a> Record<'a> {
    pub fn new(id: Option<&'a [u8]>, payload: Payload<'a>) -> Self {
        let mut header = Header::default();
        header.set_type_name_format(TypeNameFormat::from(&payload));
        if id.is_some() {
            header.set_id_length();
        }
        if payload.len() < 256 {
            header.set_short_record();
        }
        Self {
            header,
            id,
            payload,
        }
    }

    #[cfg(feature = "cbor")]
    pub fn is_type_cbor(&self) -> bool {
        matches!(&self.payload, Payload::RTD(RecordType::Cbor(_)))
    }

    #[cfg(feature = "alloc")]
    pub fn get_type(&self) -> String {
        use alloc::string::ToString;

        match &self.payload {
            Payload::RTD(rtd) => match rtd {
                RecordType::Text { .. } => "T".to_string(),
                RecordType::External { domain, type_, .. } => format!("{domain}:{type_}"),
                #[cfg(feature = "cbor")]
                RecordType::Cbor(_) => CBOR_TYPE.to_string(),
            },
        }
    }
    #[cfg(not(feature = "alloc"))]
    pub fn get_type(&self) -> Result<'a, heapless::String<MAX_FIELD_LEN>> {
        let mut type_name = heapless::String::new();
        match &self.payload {
            Payload::RTD(rtd) => match rtd {
                RecordType::Text { .. } => type_name.push_str("T"),
                RecordType::External { domain, type_, .. } => type_name
                    .push_str(domain)
                    .and_then(|()| type_name.push(':'))
                    .and_then(|()| type_name.push_str(type_)),
                #[cfg(feature = "cbor")]
                RecordType::Cbor(_) => type_name.push_str(CBOR_TYPE),
            },
        }
        .map_err(|_| Error::BufferTooSmall)?;
        Ok(type_name)
    }

    /// Encoded payload of the record.
    pub fn payload(&self) -> Result<'a, Buffer> {
        let mut buf = Buffer::new();
        self.payload.write(&mut buf)?;
        Ok(buf)
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Message<'a> {
    #[cfg(feature = "alloc")]
    pub records: Vec<Record<'a>>,
    #[cfg(not(feature = "alloc"))]
    pub records: Vec<Record<'a>, 8>,
}

impl<'a> Message<'a> {
    #[cfg(feature = "alloc")]
    pub fn append_record(&mut self, record: &mut Record<'a>) {
        if self.records.is_empty() {
            record.header.set_message_begin();
        } else {
            self.records.last_mut().unwrap().header.clr_message_end();
        }
        record.header.set_message_end();
        self.records.push(record.clone());
    }

    #[cfg(not(feature = "alloc"))]
    pub fn append_record(&mut self, record: &mut Record<'a>) -> Result<'_, ()> {
        if self.records.is_empty() {
            record.header.set_message_begin();
        } else {
            self.records.last_mut().unwrap().header.clr_message_end();
        }
        record.header.set_message_end();
        self.records
            .push(record.clone())
            .map_err(|_| Error::BufferTooSmall)
    }

    pub fn to_vec(&self) -> Result<'a, Buffer> {
        let mut buf = Buffer::new();
        for record in &self.records {
            // Header
            write_u8(&mut buf, record.header.0)?;
            // Type Length
            write_u8(&mut buf, record.payload.type_len() as u8)?;
            // Payload Length
            write_u8(&mut buf, record.payload.len() as u8)?;
            // ID Length
            if let Some(id) = &record.id {
                write_u8(&mut buf, id.len() as u8)?;
            }
            // Type
            record.payload.write_type(&mut buf)?;
            // ID
            if let Some(id) = &record.id {
                write_all(&mut buf, id)?;
            }
            // Payload
            record.payload.write(&mut buf)?;
        }
        Ok(buf)
    }
}

impl<'a> TryFrom<&'a [u8]> for Message<'a> {
    type Error = Error<'a>;

    fn try_from(slice: &'a [u8]) -> Result<'a, Self> {
        if slice.is_empty() {
            return Err(Error::SliceTooShort);
        }
        let mut records = Vec::new();
        let mut offset = 0;
        // Consumes the next `$len` bytes. `offset` is never advanced past
        // `slice.len()`, so the remaining length is computed by subtraction
        // instead of adding an encoded length to `offset`: a length field read
        // from the input can be as large as `u32::MAX` and would otherwise
        // overflow the addition on a 32-bit or smaller pointer width.
        macro_rules! take {
            ($len:expr) => {{
                let len = $len;
                if len > slice.len() - offset {
                    return Err(Error::SliceTooShort);
                }
                let start = offset;
                offset += len;
                &slice[start..offset]
            }};
        }
        while offset < slice.len() {
            // Header
            let header = Header(take!(1)[0]);
            // Type Length
            let type_length = take!(1)[0] as usize;
            // Payload Length
            let payload_length = if header.short_record() {
                take!(1)[0] as usize
            } else {
                let bytes = take!(4);
                let length = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
                usize::try_from(length).map_err(|_| Error::SliceTooShort)?
            };
            // ID Length
            let id_length = if header.id_length() {
                take!(1)[0] as usize
            } else {
                0
            };
            // Type
            let type_ = core::str::from_utf8(take!(type_length))?;
            // ID
            let id = if header.id_length() {
                Some(take!(id_length))
            } else {
                None
            };
            // Payload
            let payload_data = take!(payload_length);
            let payload = match header.type_name_format() {
                TypeNameFormat::NfcWellKnown => Payload::RTD(match type_ {
                    "T" => {
                        if payload_data.is_empty() {
                            return Err(Error::SliceTooShort);
                        }
                        let enc_len = (payload_data[0] & 0x1f) as usize;
                        let is_utf16 = (payload_data[0] & 0x80) != 0;
                        if payload_data.len() < enc_len + 1 {
                            return Err(Error::SliceTooShort);
                        }
                        let enc = core::str::from_utf8(&payload_data[1..enc_len + 1])?;
                        let txt = if is_utf16 {
                            #[cfg(not(feature = "alloc"))]
                            return Err(Error::UnsupportedEncoding);
                            #[cfg(feature = "alloc")]
                            {
                                let utf16_bytes = &payload_data[enc_len + 1..];
                                // Ensure the byte slice has an even length (UTF-16 is 2 bytes per unit)
                                if utf16_bytes.len() % 2 != 0 {
                                    return Err(Error::UTF16OddLength(utf16_bytes.len()));
                                }
                                // Convert the byte slice into u16 units
                                let utf16_units: Vec<u16> = utf16_bytes
                                    .chunks(2)
                                    .map(|chunk| u16::from_be_bytes(chunk.try_into().unwrap()))
                                    .collect();
                                String::from_utf16(&utf16_units).map_err(|_| Error::UTF16Decode)?
                            }
                        } else {
                            #[cfg(not(feature = "alloc"))]
                            {
                                core::str::from_utf8(&payload_data[enc_len + 1..])?
                            }
                            #[cfg(feature = "alloc")]
                            String::from_utf8(payload_data[enc_len + 1..].to_vec())?
                        };
                        RecordType::Text { enc, txt }
                    }
                    t => return Err(Error::UnsupportedRecordType(t)),
                }),
                TypeNameFormat::NfcExternal => match type_ {
                    #[cfg(all(feature = "cbor", not(feature = "alloc")))]
                    CBOR_TYPE => Payload::RTD(RecordType::Cbor(payload_data)),
                    #[cfg(all(feature = "cbor", feature = "alloc"))]
                    CBOR_TYPE => Payload::RTD(RecordType::Cbor(payload_data.to_vec())),
                    _ => {
                        if let Some(index) = type_.find(':') {
                            let domain = &type_[..index];
                            let type_ = &type_[index + 1..];
                            Payload::RTD(RecordType::External {
                                domain,
                                type_,
                                data: payload_data,
                            })
                        } else {
                            return Err(Error::InvalidExternalType(type_));
                        }
                    }
                },
                tnf => return Err(Error::UnsupportedTypeNameFormat(tnf)),
            };
            #[cfg(feature = "alloc")]
            records.push(Record {
                header,
                id,
                payload,
            });
            #[cfg(not(feature = "alloc"))]
            records
                .push(Record {
                    header,
                    id,
                    payload,
                })
                .map_err(|_| Error::SliceTooShort)?;
        }
        Ok(Message { records })
    }
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "alloc")]
    use alloc::string::ToString;

    use super::*;

    #[test]
    fn test_rtd_text_utf8() {
        let raw = [
            0xD1, 0x01, 0x12, 0x54, 0x02, 0x66, 0x72, 0x55, 0x54, 0x46, 0x2D, 0x38, 0x20, 0x74,
            0x65, 0x78, 0x74, 0x20, 0xf0, 0x9f, 0xa6, 0x80,
        ];
        let mut msg = Message::default();
        let txt = "UTF-8 text 🦀";
        #[cfg(feature = "alloc")]
        let txt = txt.to_string();
        let mut rec1 = Record::new(None, Payload::RTD(RecordType::Text { enc: "fr", txt }));
        #[cfg(feature = "alloc")]
        msg.append_record(&mut rec1);
        #[cfg(not(feature = "alloc"))]
        msg.append_record(&mut rec1).unwrap();
        assert_eq!(msg, Message::try_from(raw.as_slice()).unwrap());
        assert_eq!(&raw, msg.to_vec().unwrap().as_slice());
    }
    #[test]
    #[cfg(feature = "alloc")]
    fn test_rtd_text_utf16() {
        let raw = [
            0xD1, 0x01, 0x1F, 0x54, 0x82, 0x66, 0x72, 0x00, 0x55, 0x00, 0x54, 0x00, 0x46, 0x00,
            0x2D, 0x00, 0x31, 0x00, 0x36, 0x00, 0x20, 0x00, 0x74, 0x00, 0x65, 0x00, 0x78, 0x00,
            0x74, 0x00, 0x20, 0xd8, 0x3e, 0xdd, 0x80,
        ];
        let mut msg = Message::default();
        let mut rec1 = Record::new(
            None,
            Payload::RTD(RecordType::Text {
                enc: "fr",
                txt: "UTF-16 text 🦀".to_string(),
            }),
        );
        msg.append_record(&mut rec1);
        assert_eq!(msg, Message::try_from(raw.as_slice()).unwrap());
    }
    #[test]
    fn test_rtd_external() {
        let raw = [
            0xD4, 0x08, 0x01, 0x65, 0x78, 0x2e, 0x63, 0x6f, 0x6d, 0x3a, 0x74, 0x61,
        ];
        let mut msg = Message::default();
        let mut rec1 = Record::new(
            None,
            Payload::RTD(RecordType::External {
                domain: "ex.com",
                type_: "t",
                data: &[0x61],
            }),
        );
        #[cfg(feature = "alloc")]
        msg.append_record(&mut rec1);
        #[cfg(not(feature = "alloc"))]
        msg.append_record(&mut rec1).unwrap();
        assert_eq!(msg, Message::try_from(raw.as_slice()).unwrap());
        assert_eq!(&raw, msg.to_vec().unwrap().as_slice());
    }
    #[test]
    #[cfg(feature = "cbor")]
    fn test_cbor() {
        let raw = [
            0xD4, 0x0c, 0x01, 0x63, 0x62, 0x6f, 0x72, 0x2e, 0x69, 0x6f, 0x3a, 0x63, 0x62, 0x6f,
            0x72, 0x61,
        ];
        let mut msg = Message::default();
        #[cfg(feature = "alloc")]
        let mut rec1 = Record::new(None, Payload::RTD(RecordType::Cbor(alloc::vec![0x61])));
        #[cfg(not(feature = "alloc"))]
        let mut rec1 = Record::new(None, Payload::RTD(RecordType::Cbor(&[0x61])));
        #[cfg(feature = "alloc")]
        msg.append_record(&mut rec1);
        #[cfg(not(feature = "alloc"))]
        msg.append_record(&mut rec1).unwrap();
        assert_eq!(msg, Message::try_from(raw.as_slice()).unwrap());
        assert_eq!(&raw, msg.to_vec().unwrap().as_slice());
    }

    /// A UTF-16 Text record is selected by one bit of the wire status byte, so
    /// a configuration that cannot decode it must still report an error.
    #[test]
    #[cfg(not(feature = "alloc"))]
    fn test_rtd_text_utf16_unsupported() {
        let raw = [
            0xD1, 0x01, 0x1F, 0x54, 0x82, 0x66, 0x72, 0x00, 0x55, 0x00, 0x54, 0x00, 0x46, 0x00,
            0x2D, 0x00, 0x31, 0x00, 0x36, 0x00, 0x20, 0x00, 0x74, 0x00, 0x65, 0x00, 0x78, 0x00,
            0x74, 0x00, 0x20, 0xd8, 0x3e, 0xdd, 0x80,
        ];
        assert_eq!(
            Message::try_from(raw.as_slice()).unwrap_err(),
            Error::UnsupportedEncoding
        );
    }

    /// The external type name is assembled straight into the output, so it
    /// needs no allocator.
    #[test]
    fn test_rtd_external_type_name() {
        let record = Record::new(
            None,
            Payload::RTD(RecordType::External {
                domain: "ex.com",
                type_: "t",
                data: &[0x61],
            }),
        );
        #[cfg(feature = "alloc")]
        assert_eq!(record.get_type(), "ex.com:t");
        #[cfg(not(feature = "alloc"))]
        assert_eq!(record.get_type().unwrap().as_str(), "ex.com:t");
        assert_eq!(record.payload.type_len(), "ex.com:t".len());
    }

    /// A normal record announcing the largest encodable payload must be
    /// rejected rather than overflow the running offset.
    #[test]
    fn test_parse_maximum_payload_length() {
        for length in [
            [0xFF, 0xFF, 0xFF, 0xFF],
            [0xFF, 0xFF, 0xFF, 0xFE],
            [0x80, 0x00, 0x00, 0x00],
            [0x00, 0x00, 0x01, 0x00],
        ] {
            let raw = [
                0xC1, 0x01, length[0], length[1], length[2], length[3], b'T', 0x02, b'f', b'r',
            ];
            assert_eq!(
                Message::try_from(raw.as_slice()).unwrap_err(),
                Error::SliceTooShort
            );
        }
    }

    /// Every truncation of a valid message must be a recoverable error.
    #[test]
    fn test_parse_truncated() {
        let raw = [
            0xD1, 0x01, 0x12, 0x54, 0x02, 0x66, 0x72, 0x55, 0x54, 0x46, 0x2D, 0x38, 0x20, 0x74,
            0x65, 0x78, 0x74, 0x20, 0xf0, 0x9f, 0xa6, 0x80,
        ];
        for length in 0..raw.len() {
            assert!(Message::try_from(&raw[..length]).is_err());
        }
        assert!(Message::try_from(raw.as_slice()).is_ok());
    }

    /// Arbitrary input must never panic, whatever the header claims.
    #[test]
    fn test_parse_arbitrary_input() {
        let mut state = 0x1234_5678u32;
        let mut next = || {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            state
        };
        let mut buf = [0u8; 64];
        for _ in 0..4096 {
            let len = (next() % (buf.len() as u32 + 1)) as usize;
            for byte in buf.iter_mut().take(len) {
                *byte = next() as u8;
            }
            let _ = Message::try_from(&buf[..len]);
        }
    }
}
