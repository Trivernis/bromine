use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use rmp_ipc::event::Event;

pub const EVENT_NAME: &str = "bench_event";

fn create_event(data_size: usize) -> Event {
    Event::new(EVENT_NAME.to_string(), vec![0u8; data_size], None)
}

pub fn event_serialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("event_serialization");
    for size in (0..10).map(|i| i * 1024) {
        group.throughput(Throughput::Bytes(size));
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
            b.iter(|| black_box(create_event(size as usize)).into_bytes())
        });
    }
    group.finish();
}

criterion_group!(benches, event_serialization);
criterion_main!(benches);
