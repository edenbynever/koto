use crate::{Id, AstNode};
use std::fmt;

#[derive(Clone, Debug)]
pub struct Index {
    pub id: Option<Id>,
    pub expression: Box<AstNode>,
}

#[derive(Clone, Debug)]
pub struct Lookup(pub Vec<LookupNode>);

#[derive(Clone, Debug)]
pub enum LookupNode {
    Id(Id),
    Index(Index),
}

impl Lookup {
    pub fn as_slice(&self) -> LookupSlice {
        LookupSlice(self.0.as_slice())
    }

    pub fn parent_slice(&self) -> LookupSlice {
        LookupSlice(&self.0[..self.0.len() - 1])
    }

    pub fn value_slice(&self) -> LookupSlice {
        LookupSlice(&self.0[self.0.len() - 1..])
    }

    pub fn value_node(&self) -> &LookupNode {
        &self.0[self.0.len() - 1]
    }
}

impl fmt::Display for Lookup {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        LookupSlice(&self.0).fmt(f)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct LookupSlice<'a>(pub &'a [LookupNode]);

impl<'a> LookupSlice<'a> {
    pub fn parent_slice(&self) -> LookupSlice {
        LookupSlice(&self.0[..self.0.len() - 1])
    }

    pub fn value_slice(&self) -> LookupSlice {
        LookupSlice(&self.0[self.0.len() - 1..])
    }

    pub fn value_node(&self) -> &LookupNode {
        &self.0[self.0.len() - 1]
    }

    pub fn slice(&self, start: usize, end: usize) -> LookupSlice {
        LookupSlice(&self.0[start..end])
    }
}

impl<'a> fmt::Display for LookupSlice<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut first = true;
        for node in self.0.iter() {
            match &node {
                LookupNode::Id(id) => {
                    if !first {
                        write!(f, ".")?;
                    }
                    write!(f, "{}", id)?
                }
                LookupNode::Index(index) => {
                    if !first && index.id.is_some() {
                        write!(f, ".")?;
                    }

                    write!(
                        f,
                        "{}[]",
                        index.id.as_ref().map_or("".to_string(), |s| s.to_string())
                    )?
                }
            }
            first = false;
        }
        Ok(())
    }
}

