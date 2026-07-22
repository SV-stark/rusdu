use crate::tree::{NodeId, TreeNode};

#[derive(Debug, Clone)]
pub struct TreeArena {
    pub nodes: Vec<TreeNode>,
    pub root: NodeId,
}

impl TreeArena {
    pub fn new(root_node: TreeNode) -> Self {
        let nodes = vec![root_node];
        Self {
            nodes,
            root: NodeId(0),
        }
    }

    pub fn get(&self, id: NodeId) -> &TreeNode {
        &self.nodes[id.0]
    }

    pub fn get_mut(&mut self, id: NodeId) -> &mut TreeNode {
        &mut self.nodes[id.0]
    }

    pub fn add_child(&mut self, parent_id: NodeId, mut child_node: TreeNode) -> NodeId {
        let child_id = NodeId(self.nodes.len());
        child_node.parent = Some(parent_id);
        self.nodes.push(child_node);
        self.nodes[parent_id.0].children.push(child_id);
        child_id
    }

    pub fn delete_node(&mut self, node_id: NodeId) {
        // Safe deletion from tree. To avoid shifting all indices in Vec (which would invalidate all NodeId references),
        // we can simply remove the node from its parent's children list.
        // We leave the node itself in `self.nodes` (or mark it as deleted/empty) to preserve indices.
        if let Some(parent_id) = self.nodes[node_id.0].parent {
            if let Some(pos) = self.nodes[parent_id.0]
                .children
                .iter()
                .position(|&id| id == node_id)
            {
                self.nodes[parent_id.0].children.remove(pos);
            }
        }

        // Recursively clean up descendants to prevent memory leaks
        let mut stack = vec![node_id];
        while let Some(curr_id) = stack.pop() {
            let children = std::mem::take(&mut self.nodes[curr_id.0].children);
            for child_id in children {
                stack.push(child_id);
            }
            self.nodes[curr_id.0].name = Box::from("");
            self.nodes[curr_id.0].extended = None;
            self.nodes[curr_id.0].asize = 0;
            self.nodes[curr_id.0].dsize = 0;
            self.nodes[curr_id.0].stats = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tree::EntryFlags;

    #[test]
    fn test_tree_arena_add_and_delete() {
        let root = TreeNode::new_dir("root".to_string(), 1, 10, EntryFlags::empty(), None);
        let mut arena = TreeArena::new(root);

        let child1 = TreeNode::new_file(
            "child1.txt".to_string(),
            100,
            512,
            1,
            20,
            1,
            EntryFlags::empty(),
            None,
        );
        let child1_id = arena.add_child(arena.root, child1);

        let sub_dir = TreeNode::new_dir("subdir".to_string(), 1, 30, EntryFlags::empty(), None);
        let sub_dir_id = arena.add_child(arena.root, sub_dir);

        let grand_child = TreeNode::new_file(
            "gc.txt".to_string(),
            200,
            512,
            1,
            40,
            1,
            EntryFlags::empty(),
            None,
        );
        let grand_child_id = arena.add_child(sub_dir_id, grand_child);

        assert_eq!(arena.get(arena.root).children.len(), 2);
        assert_eq!(arena.get(sub_dir_id).children.len(), 1);

        // Delete sub_dir and verify cascading cleanup
        arena.delete_node(sub_dir_id);

        assert_eq!(arena.get(arena.root).children.len(), 1);
        assert_eq!(arena.get(arena.root).children[0], child1_id);
        assert_eq!(arena.get(grand_child_id).asize, 0);
    }
}
