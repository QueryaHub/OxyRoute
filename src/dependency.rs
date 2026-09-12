use std::collections::{HashMap, HashSet};

use crate::state::DependencyEntry;

/// Directed acyclic graph of dependencies.
#[derive(Debug, Clone, Default)]
pub struct DependencyGraph {
    /// Adjacency list: prerequisite -> set of dependents (edge: prereq -> dependent)
    pub adj: HashMap<String, HashSet<String>>,
    /// All registered node names
    pub nodes: HashSet<String>,
}

impl DependencyGraph {
    pub fn new() -> Self {
        Self {
            adj: HashMap::new(),
            nodes: HashSet::new(),
        }
    }

    pub fn ensure_node(&mut self, name: &str) {
        self.nodes.insert(name.to_string());
        self.adj.entry(name.to_string()).or_default();
    }

    /// Add a directed edge: `prereq` must be evaluated before `dependent`.
    pub fn add_edge(&mut self, prereq: &str, dependent: &str) {
        self.ensure_node(prereq);
        self.ensure_node(dependent);
        self.adj
            .entry(prereq.to_string())
            .or_default()
            .insert(dependent.to_string());
    }

    /// Perform topological sort using Kahn's algorithm.
    /// Returns an ordering where prerequisites appear before dependents.
    /// Returns Err if a cycle is detected.
    pub fn topological_sort(&self) -> Result<Vec<String>, String> {
        let mut in_degree: HashMap<&str, usize> = HashMap::new();
        for node in &self.nodes {
            in_degree.insert(node.as_str(), 0);
        }
        for dependents in self.adj.values() {
            for dep in dependents {
                *in_degree.entry(dep.as_str()).or_default() += 1;
            }
        }

        let mut queue: Vec<&str> = in_degree
            .iter()
            .filter_map(|(&node, &deg)| if deg == 0 { Some(node) } else { None })
            .collect();
        // Deterministic queue order
        queue.sort_unstable();

        let mut order = Vec::with_capacity(self.nodes.len());
        while let Some(node) = queue.pop() {
            order.push(node.to_string());
            if let Some(dependents) = self.adj.get(node) {
                let mut next_nodes = Vec::new();
                for dep in dependents {
                    if let Some(deg) = in_degree.get_mut(dep.as_str()) {
                        *deg -= 1;
                        if *deg == 0 {
                            next_nodes.push(dep.as_str());
                        }
                    }
                }
                next_nodes.sort_unstable();
                for n in next_nodes.into_iter().rev() {
                    queue.push(n);
                }
            }
        }

        if order.len() == self.nodes.len() {
            Ok(order)
        } else {
            Err("dependency cycle detected".to_string())
        }
    }
}

/// Build a dependency graph from route dependency entries.
pub fn build_graph_from_entries(entries: &[DependencyEntry]) -> DependencyGraph {
    let mut graph = DependencyGraph::new();
    let name_set: HashSet<&str> = entries.iter().map(|e| e.name.as_str()).collect();

    for entry in entries {
        graph.ensure_node(&entry.name);
    }

    for entry in entries {
        for param in &entry.factory_params {
            if name_set.contains(param.as_str()) {
                // `param` is a prerequisite of `entry.name`
                graph.add_edge(param, &entry.name);
            }
        }
    }

    graph
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_independent_nodes() {
        let mut graph = DependencyGraph::new();
        graph.ensure_node("a");
        graph.ensure_node("b");
        let order = graph.topological_sort().expect("no cycle");
        assert_eq!(order.len(), 2);
    }

    #[test]
    fn test_linear_chain() {
        let mut graph = DependencyGraph::new();
        // b depends on a (a is prereq of b)
        graph.add_edge("a", "b");
        // c depends on b (b is prereq of c)
        graph.add_edge("b", "c");

        let order = graph.topological_sort().expect("no cycle");
        let pos_a = order.iter().position(|x| x == "a").unwrap();
        let pos_b = order.iter().position(|x| x == "b").unwrap();
        let pos_c = order.iter().position(|x| x == "c").unwrap();
        assert!(pos_a < pos_b);
        assert!(pos_b < pos_c);
    }

    #[test]
    fn test_diamond_dependency() {
        let mut graph = DependencyGraph::new();
        // b and c depend on a; d depends on b and c
        graph.add_edge("a", "b");
        graph.add_edge("a", "c");
        graph.add_edge("b", "d");
        graph.add_edge("c", "d");

        let order = graph.topological_sort().expect("no cycle");
        let pos_a = order.iter().position(|x| x == "a").unwrap();
        let pos_b = order.iter().position(|x| x == "b").unwrap();
        let pos_c = order.iter().position(|x| x == "c").unwrap();
        let pos_d = order.iter().position(|x| x == "d").unwrap();
        assert!(pos_a < pos_b);
        assert!(pos_a < pos_c);
        assert!(pos_b < pos_d);
        assert!(pos_c < pos_d);
    }

    #[test]
    fn test_simple_cycle() {
        let mut graph = DependencyGraph::new();
        graph.add_edge("a", "b");
        graph.add_edge("b", "a");
        assert!(graph.topological_sort().is_err());
    }

    #[test]
    fn test_self_cycle() {
        let mut graph = DependencyGraph::new();
        graph.add_edge("a", "a");
        assert!(graph.topological_sort().is_err());
    }
}
