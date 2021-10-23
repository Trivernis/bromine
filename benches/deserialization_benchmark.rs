use criterion::{black_box, BenchmarkId, Throughput};
use criterion::{criterion_group, criterion_main};
use criterion::{BatchSize, Criterion};
use std::io::Cursor;

use rmp_ipc::event::Event;
use tokio::runtime::Runtime;

pub const EVENT_NAME: &str = "bench_event";

fn create_event_bytes_reader(data_size: usize) -> Cursor<Vec<u8>> {
    let bytes = Event::new(EVENT_NAME.to_string(), vec![0u8; data_size], None)
        .into_bytes()
        .unwrap();
    Cursor::new(bytes)
}

fn event_deserialization(c: &mut Criterion) {
    let runtime = Runtime::new().unwrap();
    let mut group = c.benchmark_group("event_deserialization");

    for size in (0..10)
        .step_by(2)
        .map(|i| 1024 * 2u32.pow(i as u32) as usize)
    {
        group.throughput(Throughput::Bytes(size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
            b.to_async(&runtime).iter_batched(
                || black_box(create_event_bytes_reader(size)),
                |mut reader| async move {
                    Event::from_async_read(&mut reader).await.unwrap();
                },
                BatchSize::LargeInput,
            )
        });
    }
    group.finish()
}

criterion_group!(benches, event_deserialization);
criterion_main!(benches);
