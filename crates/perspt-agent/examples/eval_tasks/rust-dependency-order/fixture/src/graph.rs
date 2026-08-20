use std::collections::BTreeMap;

pub fn topo_order(edges: &[(String, String)]) -> Option<Vec<String>> {
    let _ = edges;
    let _unused: BTreeMap<String, u32> = BTreeMap::new();
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn edge(a: &str, b: &str) -> (String, String) { (a.into(), b.into()) }
    #[test]
    fn orders_dependencies_first() {
        let order = topo_order(&[edge("a", "b"), edge("b", "c")]).unwrap();
        assert_eq!(order, vec!["a", "b", "c"]);
    }
}
