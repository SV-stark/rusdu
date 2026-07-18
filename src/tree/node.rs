use bitflags::bitflags;

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct NodeId(pub usize);

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct EntryFlags: u16 {
        const IS_DIR       = 0b0000_0001;
        const READ_ERROR   = 0b0000_0010;  // '!'
        const SUB_ERROR    = 0b0000_0100;  // '.'
        const EXCLUDED     = 0b0000_1000;  // '<'
        const OTHER_FS     = 0b0001_0000;  // '>'
        const KERNFS       = 0b0010_0000;  // 'F'
        const NOT_REG      = 0b0100_0000;  // '@'
        const HARD_LINK    = 0b1000_0000;  // 'H'
        const EMPTY_DIR    = 0b0000_0001_0000_0000; // 'e'
    }
}

#[derive(Debug, Clone)]
pub struct ExtendedInfo {
    pub mtime: i64, // Unix timestamp
    pub uid: u32,
    pub gid: u32,
    pub mode: u32,
}

#[derive(Debug, Clone)]
pub struct TreeNode {
    /// File/directory name (not full path)
    pub name: Box<str>,
    /// Apparent size in bytes
    pub asize: i64,
    /// Disk usage in bytes
    pub dsize: i64,
    /// Device number (for cross-filesystem detection)
    pub dev: u64,
    /// Inode number (for hard link detection)
    pub ino: u64,
    /// Hard link count
    pub nlink: u32,
    /// Entry flags
    pub flags: EntryFlags,
    /// Extended info (optional, only with -e)
    pub extended: Option<ExtendedInfo>,
    /// Children (dirs only) - indices into arena
    pub children: Vec<NodeId>,
    /// Parent index
    pub parent: Option<NodeId>,
    /// Aggregated stats (recalculated/aggregated bottom-up)
    pub stats: crate::tree::AggregateStats,
}

impl TreeNode {
    #[allow(clippy::too_many_arguments)]
    pub fn new_file(
        name: String,
        asize: i64,
        dsize: i64,
        dev: u64,
        ino: u64,
        nlink: u32,
        flags: EntryFlags,
        extended: Option<ExtendedInfo>,
    ) -> Self {
        Self {
            name: name.into_boxed_str(),
            asize,
            dsize,
            dev,
            ino,
            nlink,
            flags,
            extended,
            children: Vec::new(),
            parent: None,
            stats: crate::tree::AggregateStats::default(),
        }
    }

    pub fn new_dir(
        name: String,
        dev: u64,
        ino: u64,
        flags: EntryFlags,
        extended: Option<ExtendedInfo>,
    ) -> Self {
        Self {
            name: name.into_boxed_str(),
            asize: 0,
            dsize: 0,
            dev,
            ino,
            nlink: 1,
            flags: flags | EntryFlags::IS_DIR,
            extended,
            children: Vec::new(),
            parent: None,
            stats: crate::tree::AggregateStats::default(),
        }
    }

    pub fn is_dir(&self) -> bool {
        self.flags.contains(EntryFlags::IS_DIR)
    }
}
