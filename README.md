# ndef

[![Crates.io](https://img.shields.io/crates/v/ndef.svg?maxAge=2592000)](https://crates.io/crates/ndef)

`#![no_std]` Rust library to manipulate NDEF.

## Features

- alloc: enable a global allocator (use `alloc::vec::Vec` instead of `heapless::Vec`)

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
            txt: "NDEF Text from Rust!",
        }),
    );
    msg.append_record(&mut rec1).unwrap();

    // Print message raw data
    println!("message raw data: {:?}", msg.to_vec().unwrap().as_slice());
}
```
