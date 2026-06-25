//! Round 8 — P9: data_serialization.rs module via prte-beast daemon.
//!
//! Uses server_init for data_serialization testing. Single consolidated test.
//! Uses daemon_lock for serialization.
//!
//! Run:
//!   cargo test --test daemon_data_serialization_via_daemon -- --include-ignored --test-threads=1

mod daemon_helper;

use pmix::data_serialization::{
    PmixByteObject, PmixDataBuffer, data_buffer_create, data_buffer_release, data_compress,
    data_copy, data_copy_payload, data_decompress, data_embed, data_load, data_pack, data_print,
    data_unload, data_unpack,
};
use pmix::{InfoBuilder, PmixDataType, PmixError, PmixStatus};

// ─────────────────────────────────────────────────────────────────────────────
// Daemon tests — single consolidated test using server_init/server_finalize.
// ─────────────────────────────────────────────────────────────────────────────

/// Full data_serialization workflow: create → pack → unpack → unload → load → copy → embed → compress → decompress
#[test]
#[ignore = "daemon isolation"]
fn test_data_serialization_all_ffi_operations() {
    let _lock = daemon_helper::daemon_lock().expect("daemon lock");
    let _ = daemon_helper::get_tool_handle().expect("shared tool handle (daemon available)");

    let module = pmix::server::PmixServerModule::default();
    let info = InfoBuilder::new().build();
    let handle = pmix::server::server_init(Some(&module), &info).expect("server_init");

    // ── data_buffer_create ──
    let mut buf = data_buffer_create().expect("create buffer");

    // ── data_pack (pack an i32) ──
    let val: i32 = 42;
    let _ = data_pack::<i32>(None, &buf, &val, 1, PmixDataType::Int);

    // ── data_unpack ──
    let mut unpacked: i32 = 0;
    let mut count: i32 = 1;
    let _ = data_unpack::<i32>(None, &buf, &mut unpacked, &mut count, PmixDataType::Int);

    // ── data_unload ──
    let _ = data_unload(&buf);

    // ── data_load ──
    let byte_obj = PmixByteObject::new();
    let _ = data_load(&buf, &byte_obj);

    // ── data_copy_payload ──
    let mut buf2 = data_buffer_create().expect("create buffer 2");
    let _ = data_copy_payload(&buf2, &buf);

    // ── data_embed ──
    let _ = data_embed(&buf, None);

    // ── data_compress / data_decompress ──
    let test_data = b"hello pmix serialization";
    let compress_result = data_compress(test_data);
    let _ = compress_result;
    if let Ok(compressed) = compress_result {
        let _ = data_decompress(&compressed);
    }

    // ── data_print ──
    let _ = data_print::<i32>(&val, Some("val:"), PmixDataType::Int);

    // ── data_copy ──
    let _ = data_copy::<i32>(&val, PmixDataType::Int);

    // ── data_buffer_release ──
    data_buffer_release(&mut buf);
    data_buffer_release(&mut buf2);

    let _ = pmix::server::server_finalize(handle);
}
