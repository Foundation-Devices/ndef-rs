<!--
SPDX-FileCopyrightText: © 2024 Foundation Devices, Inc. <hello@foundation.xyz>
SPDX-License-Identifier: GPL-3.0-or-later
-->

# ndef

[![Crates.io](https://img.shields.io/crates/v/ndef.svg?maxAge=2592000)](https://crates.io/crates/ndef)

`#![no_std]` Rust library to manipulate NDEF.

## Features

- alloc: enable a global allocator
    - allow more than 8 records per message
    - allow more than 256 bytes of payload size
    - allow to decode UTF-16 RTD Text record (always encoded in UTF-8)
    - allow to encode RTD external record
- cbor: add minicbor dependency
    - provide a custom cbor RTD external record
    - provide a convenient record payload from cbor encodable type (need `alloc` too)

## Example Usage
### Cargo.toml

    [dependencies]
    ndef = "0.1.0"
    

### main.rs  
```rust
use ndef::{Message, Payload, Record, RecordType};

fn main() {
    let mut msg = Message::default();
    let mut rec1 = Record::new(
        None,
        Payload::RTD(RecordType::Text {
            enc: "en",
            txt: "NDEF Text from Rust🦀!",
        }),
    );
    msg.append_record(&mut rec1).unwrap();

    // Print message raw data
    println!("message raw data: {:?}", msg.to_vec().unwrap().as_slice());
}
```
