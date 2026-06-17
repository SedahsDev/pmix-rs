//! Benchmarks for data serialization hot paths.
//!
//! Measures throughput of buffer creation, data pack/unpack, and
//! byte object operations.
//!
//! Run with:
//! ```bash
//! cargo bench --bench data_serialization_bench
//! ```

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use pmix::data_serialization::*;

// ─────────────────────────────────────────────────────────────────────────────
// Buffer lifecycle benchmarks
// ─────────────────────────────────────────────────────────────────────────────

fn bench_buffer_create_release(c: &mut Criterion) {
    c.bench_function("buffer_create_release", |b| {
        b.iter(|| {
            let mut buf = data_buffer_create().unwrap();
            data_buffer_release(&mut buf);
        })
    });
}

// ─────────────────────────────────────────────────────────────────────────────
// ByteObject benchmarks
// ─────────────────────────────────────────────────────────────────────────────

fn bench_byteobject_from_vec_small(c: &mut Criterion) {
    c.bench_function("byteobject_from_vec_64B", |b| {
        let data = vec![0xABu8; 64];
        b.iter(|| {
            let _obj = PmixByteObject::from(black_box(data.clone()));
        })
    });
}

fn bench_byteobject_from_vec_large(c: &mut Criterion) {
    c.bench_function("byteobject_from_vec_4KB", |b| {
        let data = vec![0xABu8; 4096];
        b.iter(|| {
            let _obj = PmixByteObject::from(black_box(data.clone()));
        })
    });
}

fn bench_byteobject_as_slice(c: &mut Criterion) {
    c.bench_function("byteobject_as_slice_256B", |b| {
        let obj = PmixByteObject::from(vec![0xABu8; 256]);
        b.iter(|| {
            black_box(obj.as_slice());
        })
    });
}

// ─────────────────────────────────────────────────────────────────────────────
// Load/Unload benchmarks (no PMIx_Init needed)
// ─────────────────────────────────────────────────────────────────────────────

fn bench_load_unload_small(c: &mut Criterion) {
    c.bench_function("load_unload_64B", |b| {
        let payload = PmixByteObject::from(vec![0xABu8; 64]);
        b.iter(|| {
            let mut buf = data_buffer_create().unwrap();
            data_load(&buf, &payload).unwrap();
            let _out = data_unload(&buf).unwrap();
            data_buffer_release(&mut buf);
        })
    });
}

fn bench_load_unload_large(c: &mut Criterion) {
    c.bench_function("load_unload_4KB", |b| {
        let payload = PmixByteObject::from(vec![0xABu8; 4096]);
        b.iter(|| {
            let mut buf = data_buffer_create().unwrap();
            data_load(&buf, &payload).unwrap();
            let _out = data_unload(&buf).unwrap();
            data_buffer_release(&mut buf);
        })
    });
}

fn bench_load_unload_64KB(c: &mut Criterion) {
    c.bench_function("load_unload_64KB", |b| {
        let payload = PmixByteObject::from(vec![0xABu8; 65536]);
        b.iter(|| {
            let mut buf = data_buffer_create().unwrap();
            data_load(&buf, &payload).unwrap();
            let _out = data_unload(&buf).unwrap();
            data_buffer_release(&mut buf);
        })
    });
}

// ─────────────────────────────────────────────────────────────────────────────
// Buffer growth benchmark — simulates packing many values via load
// ─────────────────────────────────────────────────────────────────────────────

fn bench_buffer_grow(c: &mut Criterion) {
    c.bench_function("buffer_grow_100x64B", |b| {
        b.iter(|| {
            let mut buf = data_buffer_create().unwrap();
            for _ in 0..100 {
                let payload = PmixByteObject::from(vec![0xABu8; 64]);
                data_load(&buf, &payload).unwrap();
            }
            data_buffer_release(&mut buf);
        })
    });
}

// ─────────────────────────────────────────────────────────────────────────────
// Buffer bytes_used/bytes_allocated queries
// ─────────────────────────────────────────────────────────────────────────────

fn bench_buffer_queries(c: &mut Criterion) {
    c.bench_function("buffer_bytes_used_query", |b| {
        let payload = PmixByteObject::from(vec![0xABu8; 256]);
        let mut buf = data_buffer_create().unwrap();
        data_load(&buf, &payload).unwrap();
        b.iter(|| {
            black_box(buf.bytes_used());
            black_box(buf.bytes_allocated());
        });
        data_buffer_release(&mut buf);
    });
}

// ─────────────────────────────────────────────────────────────────────────────
// Criteria
// ─────────────────────────────────────────────────────────────────────────────

fn benchmark_suite(c: &mut Criterion) {
    bench_buffer_create_release(c);
    bench_byteobject_from_vec_small(c);
    bench_byteobject_from_vec_large(c);
    bench_byteobject_as_slice(c);
    bench_load_unload_small(c);
    bench_load_unload_large(c);
    bench_load_unload_64KB(c);
    bench_buffer_grow(c);
    bench_buffer_queries(c);
}

criterion_group!(benches, benchmark_suite);
criterion_main!(benches);
