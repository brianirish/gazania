//! Human-readable byte counts in the style of `df -h`.

const UNITS: [&str; 6] = ["B", "K", "M", "G", "T", "P"];

pub fn human_size(bytes: u64) -> String {
    if bytes < 1024 {
        return format!("{bytes}B");
    }
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if value < 10.0 {
        format!("{value:.1}{}", UNITS[unit])
    } else {
        format!("{value:.0}{}", UNITS[unit])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const G: u64 = 1024 * 1024 * 1024;

    #[test]
    fn bytes_below_one_k_print_as_bytes() {
        assert_eq!(human_size(0), "0B");
        assert_eq!(human_size(1023), "1023B");
    }

    #[test]
    fn one_decimal_below_ten_units() {
        assert_eq!(human_size(1024), "1.0K");
        assert_eq!(human_size(1536), "1.5K");
        assert_eq!(human_size(2 * G), "2.0G");
        assert_eq!(human_size((1.8 * G as f64) as u64), "1.8G");
    }

    #[test]
    fn integer_from_ten_units_up() {
        assert_eq!(human_size(475 * G), "475G");
        assert_eq!(human_size(219 * 1024 * 1024), "219M");
        assert_eq!(human_size(447 * G + G / 10), "447G");
    }
}
