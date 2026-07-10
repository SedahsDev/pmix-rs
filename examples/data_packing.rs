//! Sophisticated data packing / unpacking example.
//!
//! Demonstrates packing heterogeneous data (i32 + string + bytes) into a
//! PMIx data buffer, unloading for "transport", loading into a new buffer,
//! and unpacking the values.
//!
//! This is more sophisticated than basic put/get because it shows:
//! - Multiple data types in one buffer
//! - Buffer unload/load (for sending over the wire or storing)
//! - Proper count handling for unpack
//! - Roundtrip verification
//!
//! Run with: cargo run --example data_packing

use pmix::{PmixDataType, data_serialization::*};

fn main() {
    println!("PMIx sophisticated data packing example");

    // === Sender side ===
    let mut buf = data_buffer_create().expect("failed to create data buffer");

    // Pack an i32
    let int_val: i32 = 42;
    let packed_int =
        data_pack(None, &buf, &int_val, 1, PmixDataType::Int32).expect("failed to pack i32");
    println!("Packed {} i32 value(s)", packed_int);

    // Pack a string (as PMIX_STRING)
    let msg = "hello from packed data";
    let packed_str =
        data_pack(None, &buf, &msg, 1, PmixDataType::String).expect("failed to pack string");
    println!("Packed {} string value(s)", packed_str);

    // Pack some raw bytes (as array of uint8)
    let bytes: Vec<u8> = vec![0xde, 0xad, 0xbe, 0xef];
    let packed_bytes = data_pack(None, &buf, &bytes, bytes.len() as i32, PmixDataType::Uint8)
        .expect("failed to pack bytes");
    println!("Packed {} byte(s)", packed_bytes);

    // Unload the buffer into a byte object (simulates transport / storage)
    let payload = data_unload(&buf).expect("failed to unload buffer");
    println!("Unloaded buffer -> {} bytes payload", payload.size());

    // Release the original buffer
    data_buffer_release(&mut buf);

    // === Receiver side (simulating another process or later time) ===
    let mut buf2 = data_buffer_create().expect("failed to create receiver buffer");
    data_load(&buf2, &payload).expect("failed to load payload into buffer");

    // Unpack the i32 (count starts as max we expect)
    let mut out_int: i32 = 0;
    let mut count: i32 = 1;
    let unpacked_int = data_unpack(None, &buf2, &mut out_int, &mut count, PmixDataType::Int32)
        .expect("failed to unpack i32");
    println!("Unpacked {} i32: {}", unpacked_int, out_int);

    // Unpack the string
    let mut out_str: String = String::new();
    let mut count: i32 = 1;
    let unpacked_str = data_unpack(None, &buf2, &mut out_str, &mut count, PmixDataType::String)
        .expect("failed to unpack string");
    println!("Unpacked {} string: {}", unpacked_str, out_str);

    // Unpack the bytes (we know how many)
    let mut out_bytes: Vec<u8> = vec![0u8; 4];
    let mut count: i32 = 4;
    let unpacked_bytes = data_unpack(None, &buf2, &mut out_bytes, &mut count, PmixDataType::Uint8)
        .expect("failed to unpack bytes");
    println!("Unpacked {} bytes: {:02x?}", unpacked_bytes, out_bytes);

    // Verify roundtrip
    assert_eq!(out_int, 42);
    assert_eq!(out_str, "hello from packed data");
    assert_eq!(out_bytes, vec![0xde, 0xad, 0xbe, 0xef]);
    println!("Roundtrip verification successful!");

    // Clean up
    data_buffer_release(&mut buf2);

    println!("Data packing example completed successfully");
}
