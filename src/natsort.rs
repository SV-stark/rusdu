pub fn natural_compare(a: &str, b: &str) -> std::cmp::Ordering {
    natord::compare(a, b)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cmp::Ordering;

    #[test]
    fn test_natural_compare() {
        assert_eq!(natural_compare("file2.txt", "file10.txt"), Ordering::Less);
        assert_eq!(natural_compare("file10.txt", "file2.txt"), Ordering::Greater);
        assert_eq!(natural_compare("img1.png", "img1.png"), Ordering::Equal);
    }
}
