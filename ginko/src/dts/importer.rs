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

pub trait HasDependencies<V> {
    fn dependencies(&self) -> &[V];

    fn id(&self) -> &V;
}

/// Checks cyclic dependencies by adding them piece-by piece using the `add` method.
/// This struct operates on easy cloneable objects (such as strings or ints)
/// and can provide the dependency map later-on.
pub struct CyclicDependencyChecker<V>
    where
        V: Hash + Eq + Clone,
{
    // Set of elements that are already found.
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
        element: impl HasDependencies<V>,
    ) -> Result<(), CyclicDependencyError<V>> {
        for dependency in element.dependencies() {
            if self.pool.contains(dependency) && self.trace_back.contains_key(element.id()) {
                return Err(CyclicDependencyError::new(
                    self.trace_back(element.id(), dependency),
                ));
            }
        }
        self.pool
            .insert(element.id().clone());
        for dependency in element.dependencies() {
            self.trace_back
                .insert(dependency.clone(), element.id().clone());
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
    use crate::dts::importer::{CyclicDependencyChecker, CyclicDependencyError, HasDependencies};
    use std::hash::Hash;

    struct SimpleDependency<V> {
        dependencies: Vec<V>,
        value: V,
    }

    impl<V> SimpleDependency<V>
        where
            V: Hash + Eq + Clone,
    {
        pub fn new(value: V, dependencies: Vec<V>) -> SimpleDependency<V> {
            SimpleDependency {
                dependencies,
                value,
            }
        }
    }

    impl<V> HasDependencies<V> for SimpleDependency<V> {
        fn dependencies(&self) -> &[V] {
            &self.dependencies
        }

        fn id(&self) -> &V {
            &self.value
        }
    }

    #[test]
    fn ok_for_files_without_dependencies() {
        let mut checker = CyclicDependencyChecker::new();

        let one = SimpleDependency::new(1, vec![]);
        let two = SimpleDependency::new(2, vec![]);

        assert_eq!(checker.add(one), Ok(()));
        assert_eq!(checker.add(two), Ok(()));
    }

    #[test]
    fn ok_for_unrelated_files() {
        let mut checker = CyclicDependencyChecker::new();

        let one = SimpleDependency::new(1, vec![2]);
        let three = SimpleDependency::new(3, vec![4]);

        assert_eq!(checker.add(one), Ok(()));
        assert_eq!(checker.add(three), Ok(()));
    }

    #[test]
    fn ok_dependencies_for_multiple_includes() {
        let mut checker = CyclicDependencyChecker::new();

        let one = SimpleDependency::new(1, vec![]);
        let two = SimpleDependency::new(2, vec![]);
        let three = SimpleDependency::new(3, vec![1, 2]);

        assert_eq!(checker.add(one), Ok(()));
        assert_eq!(checker.add(two), Ok(()));
        assert_eq!(checker.add(three), Ok(()));
    }

    #[test]
    fn ok_for_dependency_in_multiple_files() {
        let mut checker = CyclicDependencyChecker::new();

        let one = SimpleDependency::new(1, vec![2]);
        let two = SimpleDependency::new(2, vec![]);
        let three = SimpleDependency::new(3, vec![1]);

        assert_eq!(checker.add(one), Ok(()));
        assert_eq!(checker.add(two), Ok(()));
        assert_eq!(checker.add(three), Ok(()));
    }

    #[test]
    fn ok_for_files_with_non_cyclic_dependencies() {
        let mut checker = CyclicDependencyChecker::new();

        // one has dependency on two and two has dependency on one.
        let one = SimpleDependency::new(1, vec![2]);
        let three = SimpleDependency::new(2, vec![3]);

        assert_eq!(checker.add(one), Ok(()));
        assert_eq!(checker.add(three), Ok(()));
    }

    #[test]
    fn simple_cyclic_dependency() {
        let mut checker = CyclicDependencyChecker::new();

        // one has dependency on two and two has dependency on one.
        let one = SimpleDependency::new(1, vec![2]);
        let two = SimpleDependency::new(2, vec![1]);

        assert_eq!(checker.add(one), Ok(()));
        assert_eq!(
            checker.add(two),
            Err(CyclicDependencyError { elements: vec![2, 1] })
        );
    }

    #[test]
    fn cylic_dependency_spanning_multiple_files() {
        let mut checker = CyclicDependencyChecker::new();

        // one has dependency on two and two has dependency on one.
        let one = SimpleDependency::new(1, vec![2]);
        let two = SimpleDependency::new(2, vec![3]);
        let three = SimpleDependency::new(3, vec![1]);

        assert_eq!(checker.add(one), Ok(()));
        assert_eq!(checker.add(two), Ok(()));
        assert_eq!(
            checker.add(three),
            Err(CyclicDependencyError {
                elements: vec![3, 2, 1]
            })
        );
    }
}
