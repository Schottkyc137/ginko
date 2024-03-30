use itertools::Itertools;
use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt::Debug;
use std::hash::Hash;

#[derive(Debug, Eq, PartialEq)]
pub struct CyclicDependencyError<V> {
    elements: Vec<V>,
}

impl<V> CyclicDependencyError<V> {
    pub fn new(elements: Vec<V>) -> CyclicDependencyError<V> {
        CyclicDependencyError { elements }
    }
}

/// Checks cyclic dependencies by adding them piece-by piece using the `add` method.
/// This struct operates on easy cloneable objects (such as strings or ints)
/// and can provide the dependency map later-on.
pub struct CyclicDependencyChecker<V>
where
    V: Hash + Eq + Clone,
{
    nodes: HashMap<V, Vec<V>>,
}

impl<V> CyclicDependencyChecker<V>
where
    V: Hash + Eq + Clone,
{
    pub fn new() -> CyclicDependencyChecker<V> {
        CyclicDependencyChecker {
            nodes: Default::default(),
        }
    }

    /// Adds an element to the checker. This will return `Ok(())`,
    /// if there are no cyclic dependencies and an error containing the import cycle, if there are
    /// such dependencies.
    ///
    /// ## Edge cases:
    /// * An element should only be added once.
    /// * Therefore, all dependencies of the `element` must be added at once.
    ///   You should not overwrite elements or add dependencies after adding the element.
    ///
    /// # Arguments
    /// * `element` The node that contains dependencies
    /// * `dependencies` The dependencies of the element.
    pub fn add(&mut self, element: V, dependencies: &[V]) -> Result<(), CyclicDependencyError<V>> {
        debug_assert!(!self.nodes.contains_key(&element));
        self.nodes
            .insert(element.clone(), dependencies.iter().cloned().collect_vec());
        self.check_for_cyclic_dependencies(element)
    }

    /// Checks for cycles in the dependency graph and returns `Ok(())`, if no cycles were found and
    /// `CyclicDependencyError(...)` with the cycle, if cycles were found.
    fn check_for_cyclic_dependencies(&self, start: V) -> Result<(), CyclicDependencyError<V>> {
        let mut visited = HashSet::new();
        let mut stack = VecDeque::new();
        let mut parent: HashMap<V, V> = HashMap::new();

        stack.push_front(start.clone());

        while let Some(node) = stack.pop_front() {
            if visited.contains(&node) {
                let mut cycle = vec![node.clone()];
                let mut prev = parent.get(&node).cloned();
                while let Some(p) = prev {
                    cycle.push(p.clone());
                    if p == node {
                        break;
                    }
                    prev = parent.get(&p).cloned();
                }
                cycle.reverse();
                return Err(CyclicDependencyError::new(cycle));
            }

            visited.insert(node.clone());

            if let Some(neighbors) = self.nodes.get(&node) {
                for neighbor in neighbors {
                    if !parent.contains_key(neighbor) {
                        parent.insert(neighbor.clone(), node.clone());
                        stack.push_front(neighbor.clone());
                    }
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::dts::importer::{CyclicDependencyChecker, CyclicDependencyError};

    #[test]
    fn ok_for_files_without_dependencies() {
        let mut checker = CyclicDependencyChecker::new();

        assert_eq!(checker.add(1, &[]), Ok(()));
        assert_eq!(checker.add(2, &[]), Ok(()));
    }

    #[test]
    fn ok_for_unrelated_files() {
        let mut checker = CyclicDependencyChecker::new();

        assert_eq!(checker.add(1, &[2]), Ok(()));
        assert_eq!(checker.add(3, &[4]), Ok(()));
    }

    #[test]
    fn ok_for_files_with_non_cyclic_dependencies() {
        let mut checker = CyclicDependencyChecker::new();

        assert_eq!(checker.add(1, &[2]), Ok(()));
        assert_eq!(checker.add(2, &[3]), Ok(()));
        assert_eq!(checker.add(3, &[]), Ok(()));
    }

    #[test]
    fn ok_dependencies_for_multiple_includes() {
        let mut checker = CyclicDependencyChecker::new();

        assert_eq!(checker.add(1, &[]), Ok(()));
        assert_eq!(checker.add(2, &[]), Ok(()));
        assert_eq!(checker.add(3, &[1, 2]), Ok(()));
    }

    #[test]
    fn ok_for_dependency_in_multiple_files() {
        let mut checker = CyclicDependencyChecker::new();

        assert_eq!(checker.add(1, &[]), Ok(()));
        assert_eq!(checker.add(2, &[1]), Ok(()));
        assert_eq!(checker.add(3, &[1]), Ok(()));
    }

    #[test]
    fn simple_cyclic_dependency() {
        let mut checker = CyclicDependencyChecker::new();

        assert_eq!(checker.add(1, &[2]), Ok(()));
        assert_eq!(
            checker.add(2, &[1]),
            Err(CyclicDependencyError {
                elements: vec![2, 1, 2]
            })
        );
    }

    #[test]
    fn cylic_dependency_spanning_multiple_files() {
        let mut checker = CyclicDependencyChecker::new();

        assert_eq!(checker.add(1, &[2]), Ok(()));
        assert_eq!(checker.add(2, &[3]), Ok(()));
        assert_eq!(
            checker.add(3, &[1]),
            Err(CyclicDependencyError {
                elements: vec![3, 1, 2, 3]
            })
        );
    }

    #[test]
    fn cyclic_dependency_is_independent_of_order() {
        let mut checker = CyclicDependencyChecker::new();

        assert_eq!(checker.add(3, &[2]), Ok(()));
        assert_eq!(checker.add(1, &[2]), Ok(()));
        assert_eq!(
            checker.add(2, &[3]),
            Err(CyclicDependencyError::new(vec![2, 3, 2]))
        );

        let mut checker = CyclicDependencyChecker::new();

        assert_eq!(checker.add(1, &[2]), Ok(()));
        assert_eq!(checker.add(3, &[2]), Ok(()));
        assert_eq!(
            checker.add(2, &[3]),
            Err(CyclicDependencyError::new(vec![2, 3, 2]))
        );
    }

    #[test]
    fn complex_cyclic_dependency_graph() {
        let mut checker = CyclicDependencyChecker::new();

        assert_eq!(checker.add(1, &[2, 3]), Ok(()));
        assert_eq!(checker.add(2, &[4]), Ok(()));
        assert_eq!(checker.add(4, &[]), Ok(()));
        assert_eq!(checker.add(3, &[4]), Ok(()));
    }

    #[test]
    fn self_import() {
        let mut checker = CyclicDependencyChecker::new();

        assert_eq!(
            checker.add(1, &[1]),
            Err(CyclicDependencyError::new(vec![1, 1]))
        );
    }

    #[test]
    fn parallel_edges() {
        let mut checker = CyclicDependencyChecker::new();

        assert_eq!(checker.add(1, &[2]), Ok(()));
        assert_eq!(checker.add(1, &[2]), Ok(()))
    }
}
