#[test]
fn generated_files_are_up_to_date() {
    let stale = xtask::stale_files().unwrap();
    assert!(
        stale.is_empty(),
        "stale generated files, run `cargo xtask codegen`: {stale:?}"
    );
}
