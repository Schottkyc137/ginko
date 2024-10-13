pub trait MultiPeek<I> {
    fn peek(&mut self, n: usize) -> Option<&I>;

    fn peek_slice(&mut self, n: usize) -> &[I];
}

impl<I> MultiPeek<I> for std::vec::IntoIter<I> {
    fn peek(&mut self, n: usize) -> Option<&I> {
        self.as_slice().get(n)
    }

    fn peek_slice(&mut self, n: usize) -> &[I] {
        let sl = self.as_slice();
        if sl.len() < n {
            sl
        } else {
            &sl[0..n]
        }
    }
}
