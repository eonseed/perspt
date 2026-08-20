mod catalog;

pub fn lookup(key: &str) -> Option<u64> {
    catalog::ENTRIES.iter().find_map(|(candidate, value)| (*candidate == key).then_some(*value))
}
