pub fn score(fan_in: usize, fan_out: usize) -> u32 {
    (fan_in * 2 + fan_out) as u32
}

pub fn classify(score: u32) -> &'static str {
    match score {
        0..=4  => "LOW",
        5..=14 => "MEDIUM",
        _      => "HIGH",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn low_threshold() {
        assert_eq!(classify(score(0, 0)), "LOW");  // 0
        assert_eq!(classify(score(2, 0)), "LOW");  // 4
    }

    #[test]
    fn medium_threshold() {
        assert_eq!(classify(score(3, 0)), "MEDIUM"); // 6
        assert_eq!(classify(score(7, 0)), "MEDIUM"); // 14
    }

    #[test]
    fn high_threshold() {
        assert_eq!(classify(score(8, 0)), "HIGH");  // 16
        assert_eq!(classify(score(10, 5)), "HIGH"); // 25
    }
}
