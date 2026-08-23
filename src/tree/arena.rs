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

    pub fn replace_subtree(&mut self, target_node_id: NodeId, source_arena: &TreeArena) {
        // First, recursively clean up existing descendants of target_node_id
        let old_children = std::mem::take(&mut self.nodes[target_node_id.0].children);
        let mut stack = old_children;
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

        // Copy root metadata from source_arena root to target_node
        let source_root = source_arena.get(source_arena.root);
        self.nodes[target_node_id.0].dev = source_root.dev;
        self.nodes[target_node_id.0].ino = source_root.ino;
        self.nodes[target_node_id.0].flags = source_root.flags;
        self.nodes[target_node_id.0].extended = source_root.extended.clone();
        self.nodes[target_node_id.0].asize = source_root.asize;
        self.nodes[target_node_id.0].dsize = source_root.dsize;

        // Recursively clone and insert nodes from source_arena into self
        let mut id_map = std::collections::HashMap::new();
        id_map.insert(source_arena.root, target_node_id);

        let mut queue = std::collections::VecDeque::new();
        queue.push_back(source_arena.root);

        while let Some(src_id) = queue.pop_front() {
            let parent_in_self = *id_map.get(&src_id).unwrap();
            let src_node = source_arena.get(src_id);

            for &src_child_id in &src_node.children {
                let src_child = source_arena.get(src_child_id);
                let new_child = TreeNode {
                    name: src_child.name.clone(),
                    asize: src_child.asize,
                    dsize: src_child.dsize,
                    dev: src_child.dev,
                    ino: src_child.ino,
                    nlink: src_child.nlink,
                    flags: src_child.flags,
                    extended: src_child.extended.clone(),
                    parent: Some(parent_in_self),
                    children: Vec::new(),
                    stats: src_child.stats.clone(),
                };
                let new_child_id = NodeId(self.nodes.len());
                self.nodes.push(new_child);
                self.nodes[parent_in_self.0].children.push(new_child_id);
                id_map.insert(src_child_id, new_child_id);
                queue.push_back(src_child_id);
            }
        }
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
