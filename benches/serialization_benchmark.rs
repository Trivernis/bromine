use bromine::event::Event;
use criterion::{
    black_box, criterion_group, criterion_main, BatchSize, BenchmarkId, Criterion, Throughput,
};

pub const EVENT_NAME: &str = "bench_event";

fn create_event(data_size: usize) -> Event {
    Event::initiator(None, EVENT_NAME.to_string(), vec![0u8; data_size])
}

fn event_serialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("event_serialization");

    for size in (0..10)
        .step_by(2)
        .map(|i| 1024 * 2u32.pow(i as u32) as usize)
    {
        group.throughput(Throughput::Bytes(size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
            b.iter_batched(
                || black_box(create_event(size)),
                |event| event.into_bytes().unwrap(),
                BatchSize::LargeInput,
            )
        });
    }
    group.finish();
}

criterion_group!(benches, event_serialization);
criterion_main!(benches);
