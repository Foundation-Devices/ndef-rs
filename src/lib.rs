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
const MAX_FIELD_LEN: usize = u8::MAX as usize;

/// Longest payload a short record can carry. Above it the record switches to
/// the normal form, whose payload length is four big-endian octets.
const MAX_SHORT_PAYLOAD_LEN: usize = u8::MAX as usize;

/// Longest language code of a Text record. Its status byte spends the low six
/// bits on the length, bit 7 on the encoding and keeps bit 6 reserved.
const MAX_LANGUAGE_LEN: usize = 0x3f;
const TEXT_LANGUAGE_LEN_MASK: u8 = 0x3f;
const TEXT_RESERVED_MASK: u8 = 0x40;
const TEXT_UTF16_MASK: u8 = 0x80;

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
        self.0 &= !0x07;
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
                if enc.is_empty() || enc.len() > MAX_LANGUAGE_LEN || !enc.is_ascii() {
                    return Err(Error::InvalidLanguageCode);
                }
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
    id: Option<&'a [u8]>,
    pub payload: Payload<'a>,
}

impl<'a> Record<'a> {
    pub fn new(id: Option<&'a [u8]>, payload: Payload<'a>) -> Self {
        Self { id, payload }
    }

    /// Identifier carried by the record, if any.
    pub fn id(&self) -> Option<&'a [u8]> {
        self.id
    }

    /// Wire header of the record, given its position in the message. Every bit
    /// describes the bytes about to be written, so a payload replaced through
    /// the public field cannot leave a stale flag behind.
    fn header(&self, index: usize, count: usize) -> Header {
        let mut header = Header::default();
        header.set_type_name_format(TypeNameFormat::from(&self.payload));
        if index == 0 {
            header.set_message_begin();
        }
        if index + 1 == count {
            header.set_message_end();
        }
        if self.payload.len() <= MAX_SHORT_PAYLOAD_LEN {
            header.set_short_record();
        }
        if self.id.is_some() {
            header.set_id_length();
        }
        header
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
    pub fn append_record(&mut self, record: Record<'a>) {
        self.records.push(record);
    }

    #[cfg(not(feature = "alloc"))]
    pub fn append_record(&mut self, record: Record<'a>) -> Result<'a, ()> {
        // `push` hands the record back untouched when the message is full, so
        // a rejected append leaves the message exactly as it was.
        self.records.push(record).map_err(|_| Error::BufferTooSmall)
    }

    pub fn to_vec(&self) -> Result<'a, Buffer> {
        if self.records.is_empty() {
            return Err(Error::EmptyMessage);
        }
        let mut buf = Buffer::new();
        for (index, record) in self.records.iter().enumerate() {
            let payload_length = record.payload.len();
            let header = record.header(index, self.records.len());
            let short_record = header.short_record();
            // Header
            write_u8(&mut buf, header.0)?;
            // Type Length
            let type_length = record.payload.type_len();
            if type_length > MAX_FIELD_LEN {
                return Err(Error::FieldTooLong);
            }
            write_u8(&mut buf, type_length as u8)?;
            // Payload Length
            if short_record {
                write_u8(&mut buf, payload_length as u8)?;
            } else {
                let payload_length =
                    u32::try_from(payload_length).map_err(|_| Error::FieldTooLong)?;
                write_all(&mut buf, &payload_length.to_be_bytes())?;
            }
            // ID Length
            if let Some(id) = &record.id {
                if id.len() > MAX_FIELD_LEN {
                    return Err(Error::FieldTooLong);
                }
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
                        let status = payload_data[0];
                        if status & TEXT_RESERVED_MASK != 0 {
                            return Err(Error::InvalidTextStatus);
                        }
                        let enc_len = (status & TEXT_LANGUAGE_LEN_MASK) as usize;
                        let is_utf16 = (status & TEXT_UTF16_MASK) != 0;
                        if enc_len == 0 {
                            return Err(Error::InvalidLanguageCode);
                        }
                        if payload_data.len() < enc_len + 1 {
                            return Err(Error::SliceTooShort);
                        }
                        let enc = core::str::from_utf8(&payload_data[1..enc_len + 1])?;
                        if !enc.is_ascii() {
                            return Err(Error::InvalidLanguageCode);
                        }
                        let txt = if is_utf16 {
                            #[cfg(not(feature = "alloc"))]
                            return Err(Error::UnsupportedEncoding);
                            #[cfg(feature = "alloc")]
                            {
                                let mut utf16_bytes = &payload_data[enc_len + 1..];
                                // Ensure the byte slice has an even length (UTF-16 is 2 bytes per unit)
                                if utf16_bytes.len() % 2 != 0 {
                                    return Err(Error::UTF16OddLength(utf16_bytes.len()));
                                }
                                // A byte order mark chooses the endianness and
                                // is not part of the text. Big endian is the
                                // default when it is absent.
                                let little_endian = match utf16_bytes.get(..2) {
                                    Some([0xFF, 0xFE]) => {
                                        utf16_bytes = &utf16_bytes[2..];
                                        true
                                    }
                                    Some([0xFE, 0xFF]) => {
                                        utf16_bytes = &utf16_bytes[2..];
                                        false
                                    }
                                    _ => false,
                                };
                                // Convert the byte slice into u16 units
                                let utf16_units: Vec<u16> = utf16_bytes
                                    .chunks(2)
                                    .map(|chunk| {
                                        let unit = [chunk[0], chunk[1]];
                                        if little_endian {
                                            u16::from_le_bytes(unit)
                                        } else {
                                            u16::from_be_bytes(unit)
                                        }
                                    })
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
            records.push(Record { id, payload });
            #[cfg(not(feature = "alloc"))]
            records
                .push(Record { id, payload })
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
        let rec1 = Record::new(None, Payload::RTD(RecordType::Text { enc: "fr", txt }));
        #[cfg(feature = "alloc")]
        msg.append_record(rec1);
        #[cfg(not(feature = "alloc"))]
        msg.append_record(rec1).unwrap();
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
        let rec1 = Record::new(
            None,
            Payload::RTD(RecordType::Text {
                enc: "fr",
                txt: "UTF-16 text 🦀".to_string(),
            }),
        );
        msg.append_record(rec1);
        assert_eq!(msg, Message::try_from(raw.as_slice()).unwrap());
    }
    #[test]
    fn test_rtd_external() {
        let raw = [
            0xD4, 0x08, 0x01, 0x65, 0x78, 0x2e, 0x63, 0x6f, 0x6d, 0x3a, 0x74, 0x61,
        ];
        let mut msg = Message::default();
        let rec1 = Record::new(
            None,
            Payload::RTD(RecordType::External {
                domain: "ex.com",
                type_: "t",
                data: &[0x61],
            }),
        );
        #[cfg(feature = "alloc")]
        msg.append_record(rec1);
        #[cfg(not(feature = "alloc"))]
        msg.append_record(rec1).unwrap();
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
        let rec1 = Record::new(None, Payload::RTD(RecordType::Cbor(alloc::vec![0x61])));
        #[cfg(not(feature = "alloc"))]
        let rec1 = Record::new(None, Payload::RTD(RecordType::Cbor(&[0x61])));
        #[cfg(feature = "alloc")]
        msg.append_record(rec1);
        #[cfg(not(feature = "alloc"))]
        msg.append_record(rec1).unwrap();
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

    /// 64 characters, the first length the status byte cannot describe.
    const LONG_LANGUAGE: &str = concat!(
        "aaaaaaaaaaaaaaaa",
        "aaaaaaaaaaaaaaaa",
        "aaaaaaaaaaaaaaaa",
        "aaaaaaaaaaaaaaaa",
    );

    /// The status byte spends six bits on the language length, so a code of 32
    /// characters is valid and must not spill into the text.
    #[test]
    fn test_rtd_text_long_language() {
        assert_eq!(LONG_LANGUAGE.len(), 64);
        let enc = &LONG_LANGUAGE[..32];
        let mut raw = [0u8; 38];
        raw[0] = 0xD1;
        raw[1] = 0x01;
        raw[2] = (1 + enc.len() + 1) as u8;
        raw[3] = b'T';
        raw[4] = enc.len() as u8;
        raw[5..37].copy_from_slice(enc.as_bytes());
        raw[37] = b'x';

        let msg = Message::try_from(raw.as_slice()).unwrap();
        match &msg.records[0].payload {
            Payload::RTD(RecordType::Text { enc: parsed, txt }) => {
                assert_eq!(*parsed, enc);
                assert_eq!(&txt[..], "x");
            }
            _ => panic!("expected a text record"),
        }
        assert_eq!(msg.to_vec().unwrap().as_slice(), raw.as_slice());
    }

    /// The same text, with and without a byte order mark, decodes the same.
    #[test]
    #[cfg(feature = "alloc")]
    fn test_rtd_text_utf16_byte_order_mark() {
        let encodings: [&[u8]; 3] = [
            &[0x00, 0x41],             // big endian, no mark
            &[0xFE, 0xFF, 0x00, 0x41], // big endian mark
            &[0xFF, 0xFE, 0x41, 0x00], // little endian mark
        ];
        for encoded in encodings {
            let mut raw = alloc::vec![
                0xD1,
                0x01,
                (3 + encoded.len()) as u8,
                b'T',
                0x82,
                b'f',
                b'r'
            ];
            raw.extend_from_slice(encoded);
            let msg = Message::try_from(raw.as_slice()).unwrap();
            match &msg.records[0].payload {
                Payload::RTD(RecordType::Text { enc, txt }) => {
                    assert_eq!(*enc, "fr");
                    assert_eq!(txt, "A");
                }
                _ => panic!("expected a text record"),
            }
        }
    }

    /// A status byte or language code outside the Text definition is refused.
    #[test]
    fn test_rtd_text_invalid_language() {
        // reserved bit set
        let raw = [0xD1, 0x01, 0x04, b'T', 0x42, b'f', b'r', b'x'];
        assert_eq!(
            Message::try_from(raw.as_slice()).unwrap_err(),
            Error::InvalidTextStatus
        );
        // empty language code
        let raw = [0xD1, 0x01, 0x02, b'T', 0x00, b'x'];
        assert_eq!(
            Message::try_from(raw.as_slice()).unwrap_err(),
            Error::InvalidLanguageCode
        );
        // language code outside US-ASCII
        let raw = [0xD1, 0x01, 0x05, b'T', 0x02, 0xC3, 0xA9, b'x', b'y'];
        assert_eq!(
            Message::try_from(raw.as_slice()).unwrap_err(),
            Error::InvalidLanguageCode
        );
    }

    /// The writer cannot emit a language code the status byte cannot describe.
    #[test]
    fn test_rtd_text_language_is_validated_on_write() {
        for enc in ["", LONG_LANGUAGE, "é"] {
            let txt = "x";
            #[cfg(feature = "alloc")]
            let txt = txt.to_string();
            let mut msg = Message::default();
            let rec1 = Record::new(None, Payload::RTD(RecordType::Text { enc, txt }));
            #[cfg(feature = "alloc")]
            msg.append_record(rec1);
            #[cfg(not(feature = "alloc"))]
            msg.append_record(rec1).unwrap();
            assert_eq!(msg.to_vec().unwrap_err(), Error::InvalidLanguageCode);
        }
    }

    /// A message without a record has no wire representation.
    #[test]
    fn test_empty_message() {
        assert_eq!(
            Message::default().to_vec().unwrap_err(),
            Error::EmptyMessage
        );
    }

    /// Replacing a payload through the public field must be reflected by the
    /// header that describes it.
    #[test]
    fn test_payload_replaced_after_append() {
        let mut msg = Message::default();
        let txt = "a";
        #[cfg(feature = "alloc")]
        let txt = txt.to_string();
        let rec1 = Record::new(None, Payload::RTD(RecordType::Text { enc: "fr", txt }));
        #[cfg(feature = "alloc")]
        msg.append_record(rec1);
        #[cfg(not(feature = "alloc"))]
        msg.append_record(rec1).unwrap();

        msg.records[0].payload = Payload::RTD(RecordType::External {
            domain: "ex.com",
            type_: "t",
            data: &[0x61],
        });
        let raw = msg.to_vec().unwrap();
        assert_eq!(
            Header(raw[0]).type_name_format(),
            TypeNameFormat::NfcExternal
        );
        assert_eq!(msg, Message::try_from(raw.as_slice()).unwrap());
    }

    /// Message begin and message end follow the position of the record, so the
    /// same record can be appended more than once.
    #[test]
    fn test_record_appended_twice() {
        let mut msg = Message::default();
        let record = Record::new(
            None,
            Payload::RTD(RecordType::External {
                domain: "ex.com",
                type_: "t",
                data: &[0x61],
            }),
        );
        #[cfg(feature = "alloc")]
        {
            msg.append_record(record.clone());
            msg.append_record(record);
        }
        #[cfg(not(feature = "alloc"))]
        {
            msg.append_record(record.clone()).unwrap();
            msg.append_record(record).unwrap();
        }
        let raw = msg.to_vec().unwrap();
        // begin without end, then end without begin
        assert_eq!(raw[0] & 0xC0, 0x80);
        assert_eq!(raw[raw.len() - 12] & 0xC0, 0x40);
        assert_eq!(msg, Message::try_from(raw.as_slice()).unwrap());
    }

    /// A rejected append leaves the message untouched.
    #[test]
    #[cfg(not(feature = "alloc"))]
    fn test_append_beyond_capacity() {
        let mut msg = Message::default();
        let record = Record::new(
            None,
            Payload::RTD(RecordType::External {
                domain: "ex.com",
                type_: "t",
                data: &[0x61],
            }),
        );
        for _ in 0..8 {
            msg.append_record(record.clone()).unwrap();
        }
        let full = msg.clone();
        assert_eq!(
            msg.append_record(record).unwrap_err(),
            Error::BufferTooSmall
        );
        assert_eq!(msg, full);
        // the last record still ends the message
        let raw = msg.to_vec().unwrap();
        assert_eq!(raw[raw.len() - 12] & 0x40, 0x40);
    }

    /// A payload that no longer fits the short record form must switch to the
    /// four octet length, and still round-trip.
    #[test]
    #[cfg(feature = "alloc")]
    fn test_payload_length_encoding() {
        for length in [1usize, 254, 255, 256, 257, 1024] {
            let data = alloc::vec![0x61; length];
            let mut msg = Message::default();
            let rec1 = Record::new(
                None,
                Payload::RTD(RecordType::External {
                    domain: "ex.com",
                    type_: "t",
                    data: &data,
                }),
            );
            msg.append_record(rec1);
            let raw = msg.to_vec().unwrap();
            let short_record = raw[0] & 0x10 == 0x10;
            assert_eq!(short_record, length < 256);
            if short_record {
                assert_eq!(raw[2] as usize, length);
            } else {
                assert_eq!(
                    u32::from_be_bytes([raw[2], raw[3], raw[4], raw[5]]) as usize,
                    length
                );
            }
            assert_eq!(msg, Message::try_from(raw.as_slice()).unwrap());
        }
    }

    /// Type and ID lengths are announced by a single octet, so anything longer
    /// has to be refused rather than wrapped.
    #[test]
    #[cfg(feature = "alloc")]
    fn test_field_length_limits() {
        use alloc::string::String;

        let id = [0x00; 256];
        for (length, expected) in [(255, true), (256, false)] {
            let mut msg = Message::default();
            let rec1 = Record::new(
                Some(&id[..length]),
                Payload::RTD(RecordType::External {
                    domain: "ex.com",
                    type_: "t",
                    data: &[0x61],
                }),
            );
            msg.append_record(rec1);
            if expected {
                let raw = msg.to_vec().unwrap();
                assert_eq!(raw[3] as usize, length);
                assert_eq!(msg, Message::try_from(raw.as_slice()).unwrap());
            } else {
                assert_eq!(msg.to_vec().unwrap_err(), Error::FieldTooLong);
            }
        }

        // "<domain>:t" is one octet longer than the domain itself.
        for (length, expected) in [(253, true), (254, false)] {
            let domain = String::from_utf8(alloc::vec![b'a'; length]).unwrap();
            let mut msg = Message::default();
            let rec1 = Record::new(
                None,
                Payload::RTD(RecordType::External {
                    domain: &domain,
                    type_: "t",
                    data: &[0x61],
                }),
            );
            msg.append_record(rec1);
            if expected {
                let raw = msg.to_vec().unwrap();
                assert_eq!(raw[1] as usize, length + 2);
                assert_eq!(msg, Message::try_from(raw.as_slice()).unwrap());
            } else {
                assert_eq!(msg.to_vec().unwrap_err(), Error::FieldTooLong);
            }
        }
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
