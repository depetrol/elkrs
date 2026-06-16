
/// One tree node in a [`Forest`].
pub struct TreeNode<T> {
    pub value: T,
    pub children: Vec<usize>,
}

/// An arena holding a tree of `T`; `root` indexes into `nodes`.
pub struct Forest<T> {
    pub nodes: Vec<TreeNode<T>>,
    pub root: usize,
}

impl<T> Forest<T> {
    pub fn new(root_value: T) -> Self {
        Forest {
            nodes: vec![TreeNode { value: root_value, children: Vec::new() }],
            root: 0,
        }
    }

    /// Adds a child node under `parent` and returns its index.
    pub fn add_child(&mut self, parent: usize, value: T) -> usize {
        let idx = self.nodes.len();
        self.nodes.push(TreeNode { value, children: Vec::new() });
        self.nodes[parent].children.push(idx);
        idx
    }
}
