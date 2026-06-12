pub struct Graph<N, E> {
    nodes: Vec<N>,
    adjacency: VecMatrix<E>,
}

impl<N, E> Graph<N, E> {
    pub const fn new() -> Self {
        Self {
            nodes: Vec::new(),
            adjacency: VecMatrix::new(),
        }
    }

    pub fn with_capacity(n: usize) -> Self {
        Self {
            nodes: Vec::with_capacity(n),
            adjacency: VecMatrix::with_capacity(n),
        }
    }

    pub fn with_nodes(nodes: Vec<N>) -> Self {
        Self {
            adjacency: VecMatrix::with_capacity(nodes.len()),
            nodes,
        }
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn node_indices(&self) -> impl Iterator<Item = usize> + use<N, E> {
        0..self.node_count()
    }

    pub fn add_node(&mut self, node: N) -> usize {
        let i = self.nodes.len();
        self.nodes.push(node);

        if self.adjacency.n < self.node_count() {
            self.adjacency.expand(i, self.nodes.capacity());
        }

        i
    }

    pub fn add_edge(&mut self, from: usize, to: usize, edge: E) {
        let e = self.adjacency.edge_mut(from, to);
        assert!(e.is_none(), "edge already exists");
        *e = Some(edge);
    }

    pub fn nodes(&self) -> &[N] {
        &self.nodes
    }

    pub fn nodes_mut(&mut self) -> &mut [N] {
        &mut self.nodes
    }

    pub fn node(&self, index: usize) -> &N {
        &self.nodes[index]
    }

    pub fn node_mut(&mut self, index: usize) -> &mut N {
        &mut self.nodes[index]
    }

    fn outgoing_row(&self, index: usize) -> &[Option<E>] {
        self.adjacency.outgoing_row(self.node_count(), index)
    }

    fn outgoing_row_mut(&mut self, index: usize) -> &mut [Option<E>] {
        self.adjacency.outgoing_row_mut(self.node_count(), index)
    }

    pub fn neighbours(&self, index: usize) -> impl Iterator<Item = usize> {
        self.outgoing_row(index)
            .iter()
            .enumerate()
            .filter_map(|(i, e)| e.as_ref().map(|_| i))
    }

    pub fn disconnect_incoming(&mut self, index: usize) {
        self.adjacency
            .incoming_column_indicies(self.node_count(), index)
            .for_each(|i| {
                self.adjacency.data[i] = None;
            });
    }

    pub fn take_nodes(self) -> (Vec<N>, Graph<(), E>) {
        let Self { nodes, adjacency } = self;
        let len = nodes.len();
        (
            nodes,
            Graph {
                nodes: vec![(); len],
                adjacency,
            },
        )
    }
}

struct VecMatrix<T> {
    n: usize,
    data: Vec<Option<T>>,
}

impl<T> VecMatrix<T> {
    const fn new() -> Self {
        Self {
            n: 0,
            data: Vec::new(),
        }
    }

    fn with_capacity(n: usize) -> Self {
        let capacity = n
            .checked_mul(n)
            .expect("capacity overflow, n * n overflowed usize");
        Self {
            n,
            data: {
                let mut d = Vec::new();
                d.resize_with(capacity, || None);
                d
            },
        }
    }

    fn expand(&mut self, len: usize, new_n: usize) {
        debug_assert!(new_n >= self.n);
        let capacity = new_n
            .checked_mul(new_n)
            .expect("capacity overflow, n * n overflowed usize");

        self.data.resize_with(capacity, || None);

        for row in (0..len).rev() {
            let old_start = row * self.n;
            let new_start = row * new_n;

            for column in (0..len).rev() {
                let old_i = old_start + column;
                let new_i = new_start + column;
                self.data[new_i] = self.data[old_i].take();
            }
        }

        self.n = new_n;
    }

    fn outgoing_row(&self, len: usize, index: usize) -> &[Option<T>] {
        let start = self.n * index;
        &self.data[start..(start + len)]
    }

    fn outgoing_row_mut(&mut self, len: usize, index: usize) -> &mut [Option<T>] {
        let start = self.n * index;
        &mut self.data[start..(start + len)]
    }

    fn incoming_column_indicies(
        &self,
        len: usize,
        index: usize,
    ) -> impl Iterator<Item = usize> + use<T> {
        let capacity = self.n;
        (0..len).map(move |row| row * capacity + index)
    }

    fn edge(&self, from: usize, to: usize) -> &Option<T> {
        &self.data[self.n * from + to]
    }

    fn edge_mut(&mut self, from: usize, to: usize) -> &mut Option<T> {
        &mut self.data[self.n * from + to]
    }
}
