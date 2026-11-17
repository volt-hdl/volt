//! Kanıt dalı (#318): `evidence_step`'in mutantlarından biri timeout olur.

#[test]
fn evidence_step_reaches_twelve() {
    let mut x = 0u32;
    while x < 10 {
        x += volt_span::evidence_step();
    }
    assert_eq!(x, 12);
}
