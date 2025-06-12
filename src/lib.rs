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

    #[cfg(feature = "alloc")]
    fn to_vec(&self) -> Vec<u8> {
        match self {
            RecordType::Text { enc, txt } => {
                let mut data = Vec::new();
                // force utf-8 encoding here
                data.push(enc.len() as u8);
                data.extend_from_slice(enc.as_bytes());
                data.extend_from_slice(txt.as_bytes());
                data
            }
            RecordType::External { data, .. } => data.to_vec(),
            #[cfg(feature = "cbor")]
            RecordType::Cbor(data) => data.clone(),
        }
    }
    #[cfg(not(feature = "alloc"))]
    fn to_vec(&self) -> Result<Vec<u8, 256>> {
        match self {
            RecordType::Text { enc, txt } => {
                let mut data = Vec::new();
                // force utf-8 encoding here
                data.push(enc.len() as u8)
                    .map_err(|_| Error::BufferTooSmall)?;
                data.extend_from_slice(enc.as_bytes())
                    .map_err(|_| Error::BufferTooSmall)?;
                data.extend_from_slice(txt.as_bytes())
                    .map_err(|_| Error::BufferTooSmall)?;
                Ok(data)
            }
            RecordType::External { data, .. } => {
                Vec::from_slice(data).map_err(|_| Error::BufferTooSmall)
            }
            #[cfg(feature = "cbor")]
            RecordType::Cbor(data) => Vec::from_slice(data).map_err(|_| Error::BufferTooSmall),
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

    #[cfg(feature = "alloc")]
    fn to_vec(&self) -> Vec<u8> {
        match self {
            Payload::RTD(rtd) => rtd.to_vec(),
        }
    }
    #[cfg(not(feature = "alloc"))]
    fn to_vec(&self) -> Result<Vec<u8, 256>> {
        match self {
            Payload::RTD(rtd) => rtd.to_vec(),
        }
    }
    #[cfg(all(feature = "alloc", feature = "cbor"))]
    pub fn from_cbor_encodable<T>(x: &T) -> Self
    where
        T: minicbor::Encode<()>,
    {
        Payload::RTD(RecordType::Cbor(minicbor::to_vec(x).unwrap()))
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
                RecordType::Cbor(_) => "cbor.io:cbor".to_string(),
            },
        }
    }
    #[cfg(not(feature = "alloc"))]
    pub fn get_type(&self) -> &'a str {
        match &self.payload {
            Payload::RTD(rtd) => match rtd {
                RecordType::Text { .. } => "T",
                RecordType::External {
                    domain: _,
                    type_: _,
                    ..
                } => unimplemented!("can't concat without alloc"),
                #[cfg(feature = "cbor")]
                RecordType::Cbor(_) => "cbor.io:cbor",
            },
        }
    }

    #[cfg(feature = "alloc")]
    pub fn payload(&self) -> Vec<u8> {
        self.payload.to_vec()
    }
    #[cfg(not(feature = "alloc"))]
    pub fn payload(&self) -> Result<Vec<u8, 256>> {
        self.payload.to_vec()
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
    pub fn append_record(&mut self, record: &mut Record<'a>) -> Result<()> {
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

    #[cfg(feature = "alloc")]
    pub fn to_vec(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        for record in &self.records {
            let type_ = record.get_type();
            let payload_data = record.payload();
            // Header
            buf.push(record.header.0);
            // Type Length
            buf.push(type_.len() as u8);
            // Payload Length
            buf.push(payload_data.len() as u8);
            // ID Length
            if let Some(id) = &record.id {
                buf.push(id.len() as u8);
            }
            // Type
            buf.extend_from_slice(type_.as_bytes());
            // ID
            if let Some(id) = &record.id {
                buf.extend_from_slice(id);
            }
            // Payload
            buf.extend_from_slice(payload_data.as_slice());
        }
        buf
    }

    #[cfg(not(feature = "alloc"))]
    pub fn to_vec(&self) -> Result<Vec<u8, 256>> {
        let mut buf = Vec::new();
        for record in &self.records {
            let type_ = record.get_type();
            let payload_data = record.payload()?;
            // Header
            buf.push(record.header.0)
                .map_err(|_| Error::BufferTooSmall)?;
            // Type Length
            buf.push(type_.len() as u8)
                .map_err(|_| Error::BufferTooSmall)?;
            // Payload Length
            buf.push(payload_data.len() as u8)
                .map_err(|_| Error::BufferTooSmall)?;
            // ID Length
            if let Some(id) = &record.id {
                buf.push(id.len() as u8)
                    .map_err(|_| Error::BufferTooSmall)?;
            }
            // Type
            buf.extend_from_slice(type_.as_bytes())
                .map_err(|_| Error::BufferTooSmall)?;
            // ID
            if let Some(id) = &record.id {
                buf.extend_from_slice(id)
                    .map_err(|_| Error::BufferTooSmall)?;
            }
            // Payload
            buf.extend_from_slice(payload_data.as_slice())
                .map_err(|_| Error::BufferTooSmall)?;
        }
        Ok(buf)
    }
}

impl<'a> TryFrom<&'a [u8]> for Message<'a> {
    type Error = Error<'a>;

    fn try_from(slice: &'a [u8]) -> Result<Self> {
        if slice.is_empty() {
            return Err(Error::SliceTooShort);
        }
        let mut records = Vec::new();
        let mut offset = 0;
        macro_rules! checked_add_offset {
            ($inc:expr) => {{
                if offset + $inc > slice.len() {
                    return Err(Error::SliceTooShort);
                }
                offset += $inc;
            }};
        }
        while offset < slice.len() {
            // Header
            checked_add_offset!(1);
            let header = Header(slice[offset - 1]);
            // Type Length
            checked_add_offset!(1);
            let type_length = slice[offset - 1] as usize;
            // Payload Length
            let payload_length = if header.short_record() {
                checked_add_offset!(1);
                slice[offset - 1] as usize
            } else {
                checked_add_offset!(4);
                u32::from_be_bytes(slice[offset - 4..offset].try_into().unwrap()) as usize
            };
            // ID Length
            let id_length = if header.id_length() {
                checked_add_offset!(1);
                slice[offset - 1] as usize
            } else {
                0
            };
            // Type
            checked_add_offset!(type_length);
            let type_ = core::str::from_utf8(&slice[offset - type_length..offset])?;
            // ID
            let id = if header.id_length() {
                checked_add_offset!(id_length);
                Some(&slice[offset - id_length..offset])
            } else {
                None
            };
            // Payload
            checked_add_offset!(payload_length);
            let payload_data = &slice[offset - payload_length..offset];
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
                            unimplemented!("UTF-16 decoding is not supported yet");
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
                    "cbor.io:cbor" => Payload::RTD(RecordType::Cbor(payload_data)),
                    #[cfg(all(feature = "cbor", feature = "alloc"))]
                    "cbor.io:cbor" => Payload::RTD(RecordType::Cbor(payload_data.to_vec())),
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
        #[cfg(feature = "alloc")]
        assert_eq!(&raw, msg.to_vec().as_slice());
        #[cfg(not(feature = "alloc"))]
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
        #[cfg(feature = "alloc")]
        assert_eq!(&raw, msg.to_vec().as_slice());
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
        #[cfg(feature = "alloc")]
        assert_eq!(&raw, msg.to_vec().as_slice());
        #[cfg(not(feature = "alloc"))]
        assert_eq!(&raw, msg.to_vec().unwrap().as_slice());
    }
}
