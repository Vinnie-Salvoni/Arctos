pub fn parabola_samples(points: usize) -> Vec<(i32, i32)> {
    if points < 2 {
        return Vec::new();
    }

    let last = (points - 1) as i64;
    (0..points)
        .map(|index| {
            let x = -100 + (200 * index as i64 / last) as i32;
            (x, x * x / 100)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::parabola_samples;

    #[test]
    fn returns_evenly_spaced_symmetric_samples() {
        assert_eq!(
            parabola_samples(5),
            vec![(-100, 100), (-50, 25), (0, 0), (50, 25), (100, 100)]
        );
    }

    #[test]
    fn rejects_sample_counts_that_cannot_cover_a_range() {
        assert!(parabola_samples(0).is_empty());
        assert!(parabola_samples(1).is_empty());
    }
}
