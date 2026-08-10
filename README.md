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
- cbor: provide a custom cbor RTD external record
- dcbor: add the `dcbor` dependency (implies `cbor` and `alloc`)
    - provide a convenient record payload from cbor encodable type

## Compatibility

Without a feature or with `alloc` and `cbor`, the crate is `#![no_std]`, has no
dependency that reaches `std`, and builds on the minimum supported Rust version
declared in `Cargo.toml`. Only fixed-capacity mode (`alloc` disabled) is free of
a global allocator.

| feature set          | `no_std` target | MSRV      |
| -------------------- | --------------- | --------- |
| *(none)*             | yes             | 1.75      |
| `alloc`              | yes, with a global allocator | 1.75 |
| `cbor`               | yes             | 1.75      |
| `alloc,cbor`         | yes, with a global allocator | 1.75 |
| `dcbor`              | no, needs `std` | 1.85      |

`dcbor` is kept behind its own feature because the `dcbor` crate is published
with edition 2024 and enables the default features of `chrono` and `hex`, both
of which require `std`. Enabling it therefore raises the effective minimum Rust
version and rules out bare-metal targets.

## Example Usage
### Cargo.toml

    [dependencies]
    ndef = { version = "0.5.0", features = ["alloc"] }
    

### main.rs  
```rust
use ndef::{Message, Payload, Record, RecordType};

fn main() {
    let mut msg = Message::default();
    let rec1 = Record::new(
        None,
        Payload::RTD(RecordType::Text {
            enc: "en",
            txt: "NDEF Text from Rust🦀!".to_string(),
        }),
    );
    msg.append_record(rec1);

    // Print message raw data
    println!("message raw data: {:?}", msg.to_vec().unwrap().as_slice());
}
```

Without `alloc`, `txt` is a `&str` and `append_record` reports that the
message is full, so the same program ends with
`msg.append_record(rec1).unwrap();`.
