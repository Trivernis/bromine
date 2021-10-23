use criterion::Criterion;
use criterion::{black_box, BenchmarkId, Throughput};
use criterion::{criterion_group, criterion_main};
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

    for size in (0..32).map(|i| i * 1024) {
        group.throughput(Throughput::Bytes(size));
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
            b.to_async(&runtime).iter(|| async {
                let mut reader = black_box(create_event_bytes_reader(size as usize));
                Event::from_async_read(&mut reader).await.unwrap()
            })
        });
    }
    group.finish()
}

criterion_group!(benches, event_deserialization);
criterion_main!(benches);
