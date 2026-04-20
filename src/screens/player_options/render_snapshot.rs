//! Snapshot test for the player-options render output.
//!
//! Locks the `Vec<Actor>` produced by `get_actors` against a checked-in
//! baseline, so refactors in `render.rs` cannot silently change rendered
//! output. Set `UPDATE_SNAPSHOTS=1` to regenerate the baseline.

use crate::test_support::player_options_bench;

const BASELINE: &str = include_str!("snapshots/render_actors.txt");
const BASELINE_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/src/screens/player_options/snapshots/render_actors.txt",
);

fn render_snapshot() -> String {
    // The actor list contains nested Frame children and large vertex
    // arrays; pretty-printing recurses through them and easily blows
    // the default test-thread stack on Windows. Run the build +
    // formatting in a thread with a generous stack budget.
    std::thread::Builder::new()
        .stack_size(32 * 1024 * 1024)
        .spawn(|| {
            let fixture = player_options_bench::fixture();
            let actors = fixture.build(false);
            format!("{:#?}\n", actors)
        })
        .expect("failed to spawn snapshot thread")
        .join()
        .expect("snapshot thread panicked")
}

#[test]
fn render_actors_match_snapshot() {
    let actual = render_snapshot();

    if std::env::var("UPDATE_SNAPSHOTS").is_ok() {
        std::fs::write(BASELINE_PATH, &actual)
            .expect("failed to write baseline snapshot");
        return;
    }

    if actual != BASELINE {
        let actual_path = format!("{BASELINE_PATH}.actual");
        let _ = std::fs::write(&actual_path, &actual);
        panic!(
            "player_options render output diverged from baseline.\n\
             baseline: {BASELINE_PATH}\n\
             actual:   {actual_path}\n\
             To accept the new output, rerun with UPDATE_SNAPSHOTS=1.\n\
             First diverging char index: {}\n\
             baseline len: {}, actual len: {}",
            actual
                .as_bytes()
                .iter()
                .zip(BASELINE.as_bytes())
                .position(|(a, b)| a != b)
                .unwrap_or(actual.len().min(BASELINE.len())),
            BASELINE.len(),
            actual.len(),
        );
    }
}

#[test]
fn render_snapshot_is_deterministic() {
    let first = render_snapshot();
    let second = render_snapshot();
    assert_eq!(
        first, second,
        "render output is non-deterministic across two consecutive calls; \
         snapshot test cannot be trusted",
    );
}
