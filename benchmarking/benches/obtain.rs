use calendar::{
    date::Date,
    obtain::{EventSourceStd, NanoSerde, ObtainArguments, events_with_lanes},
};
use criterion::{Criterion, criterion_group, criterion_main};
use std::hint::black_box;

fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("events_with_lanes", |b| {
        b.iter(|| {
            let agenda_source = EventSourceStd;
            let json_parser = NanoSerde;
            let bin = "/home/antlord/Nest/personal/khal/current/.venv/bin/khal";
            let from = Date::try_new(2025, 12, 1).unwrap();
            let arguments = ObtainArguments {
                from: &from,
                duration_days: 7,
                backend_bin_path: bin,
            };
            events_with_lanes(&agenda_source, &json_parser, &arguments)
        })
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
