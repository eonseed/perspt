fn edge(a: &str, b: &str) -> (String, String) { (a.into(), b.into()) }

#[test]
fn cycles_return_none() {
    assert!(t::graph::topo_order(&[edge("a", "b"), edge("b", "a")]).is_none());
}

#[test]
fn lexicographic_among_ready() {
    let order = t::graph::topo_order(&[edge("z", "m"), edge("a", "m")]).unwrap();
    assert_eq!(order, vec!["a", "z", "m"]);
}
