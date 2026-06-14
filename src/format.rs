pub fn format_size(bytes: i64, si: bool) -> String {
    let bytes_f = bytes as f64;
    let base = if si { 1000.0 } else { 1024.0 };
    let units = if si {
        &["B", "kB", "MB", "GB", "TB", "PB", "EB"]
    } else {
        &["B", "KiB", "MiB", "GiB", "TiB", "PiB", "EiB"]
    };

    if bytes < 0 {
        return "Unknown".to_string();
    }

    if bytes < base as i64 {
        return format!("{} {}", bytes, units[0]);
    }

    let mut size = bytes_f;
    let mut unit_idx = 0;

    while size >= base && unit_idx < units.len() - 1 {
        size /= base;
        unit_idx += 1;
    }

    // Ncdu formats with one decimal place if size < 100, else no decimal place
    if size < 10.0 {
        format!("{:.2} {}", size, units[unit_idx])
    } else if size < 100.0 {
        format!("{:.1} {}", size, units[unit_idx])
    } else {
        format!("{:.0} {}", size, units[unit_idx])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_size() {
        assert_eq!(format_size(500, false), "500 B");
        assert_eq!(format_size(1024, false), "1.00 KiB");
        assert_eq!(format_size(1024 * 1024, false), "1.00 MiB");
        assert_eq!(format_size(1000, true), "1.00 kB");
    }
}
