use std::collections::{BTreeMap, BTreeSet};

pub fn topo_order(edges: &[(String, String)]) -> Option<Vec<String>> {
    let mut successors: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut indegree: BTreeMap<String, usize> = BTreeMap::new();
    for (before, after) in edges {
        successors
            .entry(before.clone())
            .or_default()
            .insert(after.clone());
        successors.entry(after.clone()).or_default();
        indegree.entry(before.clone()).or_default();
        indegree.entry(after.clone()).or_default();
    }
    for values in successors.values() {
        for after in values {
            *indegree.get_mut(after).unwrap() += 1;
        }
    }
    let mut ready: BTreeSet<String> = indegree
        .iter()
        .filter(|(_, degree)| **degree == 0)
        .map(|(node, _)| node.clone())
        .collect();
    let mut result = Vec::new();
    while let Some(node) = ready.pop_first() {
        result.push(node.clone());
        for after in &successors[&node] {
            let degree = indegree.get_mut(after).unwrap();
            *degree -= 1;
            if *degree == 0 {
                ready.insert(after.clone());
            }
        }
    }
    (result.len() == indegree.len()).then_some(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn edge(a: &str, b: &str) -> (String, String) {
        (a.into(), b.into())
    }
    #[test]
    fn orders_dependencies_first() {
        let order = topo_order(&[edge("a", "b"), edge("b", "c")]).unwrap();
        assert_eq!(order, vec!["a", "b", "c"]);
    }
}
