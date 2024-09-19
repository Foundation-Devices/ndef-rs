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
    #[cfg(feature = "alloc")]
    msg.append_record(&mut rec1);
    #[cfg(not(feature = "alloc"))]
    msg.append_record(&mut rec1).unwrap();

    // Print message raw data
    println!("message raw data: {:?}", msg.to_vec().unwrap().as_slice());
}
