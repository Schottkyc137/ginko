use std::collections::{HashMap, HashSet};
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
    // Set of elements that are already added.
    pool: HashSet<V>,
    trace_back: HashMap<V, V>,
}

impl<V> CyclicDependencyChecker<V>
    where
        V: Hash + Eq + Clone,
{
    pub fn new() -> CyclicDependencyChecker<V> {
        CyclicDependencyChecker {
            pool: Default::default(),
            trace_back: Default::default(),
        }
    }

    pub fn add(
        &mut self,
        element: V,
        dependencies: &[V],
    ) -> Result<(), CyclicDependencyError<V>> {
        for dependency in dependencies {
            if self.pool.contains(dependency) && self.trace_back.contains_key(&element) {
                return Err(CyclicDependencyError::new(
                    self.trace_back(&element, dependency),
                ));
            }
        }
        self.pool
            .insert(element.clone());
        for dependency in dependencies.into_iter() {
            self.trace_back
                .insert(dependency.clone(), element.clone());
        }
        Ok(())
    }

    fn trace_back(&self, source: &V, target: &V) -> Vec<V> {
        let mut vec = vec![source.clone()];
        let mut cur = source;
        loop {
            let dependent = self
                .trace_back
                .get(cur)
                .expect("Trace back unexpectedly failed since item to trace back is not present");
            vec.push(dependent.clone());
            if dependent == target {
                break;
            } else {
                cur = dependent;
            }
        }
        vec
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
    fn ok_dependencies_for_multiple_includes() {
        let mut checker = CyclicDependencyChecker::new();

        assert_eq!(checker.add(1, &[]), Ok(()));
        assert_eq!(checker.add(2, &[]), Ok(()));
        assert_eq!(checker.add(3, &[1, 2]), Ok(()));
    }

    #[test]
    fn ok_for_dependency_in_multiple_files() {
        let mut checker = CyclicDependencyChecker::new();

        assert_eq!(checker.add(1, &[2]), Ok(()));
        assert_eq!(checker.add(2, &[]), Ok(()));
        assert_eq!(checker.add(3, &[1]), Ok(()));
    }

    #[test]
    fn ok_for_files_with_non_cyclic_dependencies() {
        let mut checker = CyclicDependencyChecker::new();

        assert_eq!(checker.add(1, &[2]), Ok(()));
        assert_eq!(checker.add(2, &[3]), Ok(()));
    }

    #[test]
    fn simple_cyclic_dependency() {
        let mut checker = CyclicDependencyChecker::new();

        assert_eq!(checker.add(1, &[2]), Ok(()));
        assert_eq!(
            checker.add(2, &[1]),
            Err(CyclicDependencyError { elements: vec![2, 1] })
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
                elements: vec![3, 2, 1]
            })
        );
    }
}
