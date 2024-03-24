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
    trace_back: HashMap<V, Vec<V>>,
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

    pub fn add(&mut self, element: V, dependencies: &[V]) -> Result<(), CyclicDependencyError<V>> {
        for dependency in dependencies {
            for dependency in dependencies {
                self.trace_back
                    .entry(dependency.clone())
                    .or_default()
                    .push(element.clone());
            }
            if self.pool.contains(dependency) && self.trace_back.contains_key(&element) {
                return Err(CyclicDependencyError::new(
                    self.trace_back(&element, dependency),
                ));
            }
        }
        self.pool.insert(element.clone());
        Ok(())
    }

    fn trace_back(&self, source: &V, target: &V) -> Vec<V> {
        let mut work: HashSet<V> = HashSet::from_iter(self.trace_back.keys().cloned());
        let mut distance = HashMap::<V, usize>::new();
        let mut parents = HashMap::<V, V>::new();

        for par in self.trace_back.get(source).unwrap() {
            distance.insert(par.clone(), 1);
        }

        for v in self.trace_back.values().flatten() {
            parents.insert(v.clone(), source.clone());
        }
        distance.insert(source.clone(), 0);
        work.remove(source);

        while !work.is_empty() {
            let next = work
                .iter()
                .min_by(|a, b| {
                    distance
                        .get(a)
                        .copied()
                        .unwrap_or(usize::MAX)
                        .cmp(&distance.get(b).copied().unwrap_or(usize::MAX))
                })
                .expect("No element but queue should not be empty")
                .clone();
            work.remove(&next);
            for w in &work {
                if distance.get(w).copied().unwrap_or(usize::MAX - 1)
                    > distance.get(&next).copied().unwrap_or(usize::MAX - 1) + 1
                {
                    distance.insert(
                        w.clone(),
                        distance.get(&next).copied().unwrap_or(usize::MAX - 1) + 1,
                    );
                    parents.insert(w.clone(), next.clone());
                }
            }
        }

        for element in &self.trace_back {
            let (mut q, _) = element;
            if q != target {
                continue;
            }

            let mut path: Vec<V> = Vec::new();
            path.push(q.clone());
            while q != source {
                q = parents.get(q).unwrap();
                path.push(q.clone());
            }
            return path;
        }
        unreachable!("Should not happen")
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
            Err(CyclicDependencyError {
                elements: vec![1, 2]
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
                elements: vec![1, 2, 3]
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
            Err(CyclicDependencyError::new(vec![3, 2]))
        );

        let mut checker = CyclicDependencyChecker::new();

        assert_eq!(checker.add(1, &[2]), Ok(()));
        assert_eq!(checker.add(3, &[2]), Ok(()));
        assert_eq!(
            checker.add(2, &[3]),
            Err(CyclicDependencyError::new(vec![3, 2]))
        );
    }
}
