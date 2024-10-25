use criterion::{black_box, criterion_group, criterion_main, Criterion};
use mcts::{play_mcts, Goal, NimState, Player};
use rand::{rngs::StdRng, SeedableRng};

#[path = "../tests/mcts.rs"]
mod mcts;

fn benchmark_mcts(c: &mut Criterion) {
    c.bench_function("mcts_nim", |b| {
        b.iter(|| {
            let initial_state = NimState::new([30, 40, 50], Player::Player1);
            let mut rng = StdRng::seed_from_u64(12345);
            play_mcts(
                black_box(initial_state),
                black_box(1000),
                black_box(Goal::TakeLast),
                black_box(&mut rng),
            );
        })
    });
}

criterion_group!(benches, benchmark_mcts);
criterion_main!(benches);
