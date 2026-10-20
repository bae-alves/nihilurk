use super::*;

#[test]
fn a_kind_shows_up_at_most_once_a_run() {
    for last in [8, 12, 26] {
        let floors = Floors::new(last);
        for seed in 0..500 {
            let kinds: Vec<_> = (1..=last)
                .filter_map(|d| roll_special_level(seed, d, false, floors))
                .collect();
            let unique: HashSet<_> = kinds.iter().map(|k| *k as u8).collect();
            assert_eq!(unique.len(), kinds.len(), "seed {seed}, last {last}");
        }
    }
}
