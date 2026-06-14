use crate::tree::{NodeId, TreeNode};

#[derive(Debug, Clone)]
pub struct TreeArena {
    pub nodes: Vec<TreeNode>,
    pub root: NodeId,
}

impl TreeArena {
    pub fn new(root_node: TreeNode) -> Self {
        let mut nodes = Vec::new();
        nodes.push(root_node);
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
            if let Some(pos) = self.nodes[parent_id.0].children.iter().position(|&id| id == node_id) {
                self.nodes[parent_id.0].children.remove(pos);
            }
        }
    }
}
