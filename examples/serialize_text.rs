use ndef::{Header, Message, Payload, Record, RecordType};

fn main() {
    let mut msg = Message::default();
    msg.records.push(Record {
        header: Header::default(),
        id: None,
        payload: Payload::RTD(RecordType::Text {
            enc: "en",
            txt: "NDEF Text from Rust!",
        }),
    }).unwrap();

    // Print message raw data
    println!("message raw data: {:?}", msg.to_vec().unwrap().as_slice());
}
