// SPDX-FileCopyrightText: © 2024 Foundation Devices, Inc. <hello@foundation.xyz>
// SPDX-License-Identifier: GPL-3.0-or-later

#![no_std]

mod error;

pub use error::{Error, Result};

#[cfg(feature = "alloc")]
extern crate alloc;
#[cfg(feature = "alloc")]
use alloc::vec::Vec;
#[cfg(not(feature = "alloc"))]
use heapless::Vec;

#[derive(Debug, PartialEq)]
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

#[derive(Debug, Default, PartialEq)]
pub struct Header(u8);

impl Header {
    pub fn message_begin(&self) -> bool {
        self.0 & 0x80 == 0x80
    }

    pub fn message_end(&self) -> bool {
        self.0 & 0x40 == 0x40
    }

    pub fn message_chunk(&self) -> bool {
        self.0 & 0x20 == 0x20
    }

    pub fn short_record(&self) -> bool {
        self.0 & 0x10 == 0x10
    }

    pub fn id_length(&self) -> bool {
        self.0 & 0x08 == 0x08
    }

    pub fn type_name_format(&self) -> TypeNameFormat {
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
}

#[derive(Debug, PartialEq)]
pub enum RecordType<'a> {
    Text { enc: &'a str, txt: &'a str },
}

#[derive(Debug, PartialEq)]
pub enum Payload<'a> {
    RTD(RecordType<'a>),
}

#[derive(Debug, PartialEq)]
pub struct Record<'a> {
    pub header: Header,
    pub id: Option<&'a [u8]>,
    pub payload: Payload<'a>,
}

#[derive(Debug, Default, PartialEq)]
pub struct Message<'a> {
    #[cfg(feature = "alloc")]
    pub records: Vec<Record<'a>>,
    #[cfg(not(feature = "alloc"))]
    pub records: Vec<Record<'a>, 8>,
}

impl<'a> Message<'a> {
    #[cfg(feature = "alloc")]
    pub fn to_vec(&self) -> Result<Vec<u8>> {
        let mut buf = Vec::new();
        for record in &self.records {
            let (type_, payload_data) = match &record.payload {
                Payload::RTD(rtd) => match rtd {
                    RecordType::Text { enc, txt } => {
                        let mut data: Vec<u8> = Vec::new();
                        data.push(enc.len() as u8);
                        data.extend_from_slice(enc.as_bytes());
                        data.extend_from_slice(txt.as_bytes());
                        ("T", data)
                    }
                },
            };
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
        Ok(buf)
    }

    #[cfg(not(feature = "alloc"))]
    pub fn to_vec(&self) -> Result<Vec<u8, 256>> {
        let mut buf = Vec::new();
        for record in &self.records {
            let (type_, payload_data) = match &record.payload {
                Payload::RTD(rtd) => match rtd {
                    RecordType::Text { enc, txt } => {
                        let mut data: Vec<u8, 128> = Vec::new();
                        data.push(enc.len() as u8)
                            .map_err(|_| Error::BufferTooSmall)?;
                        data.extend_from_slice(enc.as_bytes())
                            .map_err(|_| Error::BufferTooSmall)?;
                        data.extend_from_slice(txt.as_bytes())
                            .map_err(|_| Error::BufferTooSmall)?;
                        ("T", data)
                    }
                },
            };
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
                        let enc_len = payload_data[0] as usize;
                        if payload_data.len() < enc_len + 1 {
                            return Err(Error::SliceTooShort);
                        }
                        let enc = core::str::from_utf8(&payload_data[1..enc_len + 1])?;
                        let txt = core::str::from_utf8(&payload_data[enc_len + 1..])?;
                        RecordType::Text { enc, txt }
                    }
                    t => return Err(Error::UnsupportedRecordType(t)),
                }),
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
    use super::*;

    #[test]
    fn test_parse() {
        //                              T       f    r    T   h    t
        let raw = [209, 1, 6, 84, 2, 102, 114, 84, 104, 116];
        let mut msg = Message::default();
        let rec1 = Record {
            header: Header(209),
            id: None,
            payload: Payload::RTD(RecordType::Text {
                enc: "fr",
                txt: "Tht",
            }),
        };
        #[cfg(feature = "alloc")]
        msg.records.push(rec1);
        #[cfg(not(feature = "alloc"))]
        msg.records.push(rec1).unwrap();
        assert_eq!(msg, Message::try_from(raw.as_slice()).unwrap());
        assert_eq!(&raw, msg.to_vec().unwrap().as_slice());
    }
}
