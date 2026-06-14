pub fn natural_compare(a: &str, b: &str) -> std::cmp::Ordering {
    natord::compare(a, b)
}
