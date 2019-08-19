use std::rc::Rc;

use crate::parse::{Memory,MemoryCell,SemanticError};

#[derive(Debug,Clone)]
pub struct DirPath {
    segments: Vec<String>
}

#[derive(Debug,Clone)]
pub struct Digest {
    bytes: [u8;16]
}

pub trait Parseable where Self:Sized+'static {
    fn from_cell(memory: &mut Memory, cell: MemoryCell) -> Result<Self,SemanticError>;
}

fn my<T:Clone>(rc: Rc<T>) -> T {
    match Rc::try_unwrap(rc) {
        Ok(item) => item,
        Err(rc) => (*rc).clone()
    }
}

impl Parseable for String {
    fn from_cell(memory: &mut Memory, cell: MemoryCell) -> Result<Self,SemanticError> {
        match cell {
            MemoryCell::String(data) => {
                match String::from_utf8(data) {
                    Ok(string) => Ok(string),
                    Err(e) => SemanticError::msg(format!("{:?}", e))?
                }
            }
            _ => SemanticError::msg(format!("String: expecting String, got {:?}", cell))
        }
    }
}

impl Parseable for Digest {
    fn from_cell(memory: &mut Memory, cell: MemoryCell) -> Result<Self,SemanticError> {
        match cell {
            MemoryCell::String(data) => {
                if data.len() == 16 {
                    let mut buffer = [0;16];
                    buffer.copy_from_slice(&data);
                    Ok(Digest{bytes:buffer})
                } else {
                    SemanticError::msg(format!("Digest: expecting 16 bytes, got {}", data.len()))
                }
            }
            _ => SemanticError::msg(format!("Digest: expecting String, got {:?}", cell))
        }
    }
}

impl Parseable for DirPath {
    fn from_cell(memory: &mut Memory, cell: MemoryCell) -> Result<Self,SemanticError> {
        match cell {
            MemoryCell::Struct(0, block) => {
                if block.len() != 2 {
                    return SemanticError::msg(format!("DirPath: expecting block length 2, got {}", block.len()));
                }
                let head = block[0].resolve_ref::<String>(memory)?;
                let tail_opt = block[1].resolve_nullable::<DirPath>(memory)?;

                let mut vec;
                if tail_opt.is_none() {
                    vec = vec![my(head)];
                } else {
                    let tail = tail_opt.as_ref().unwrap();
                    vec = Vec::with_capacity(tail.segments.len());
                    vec.push(my(head));
                    vec.extend_from_slice(&tail.segments)
                }
                Ok(DirPath{segments:vec})
            }
            _ => SemanticError::msg(format!("DirPath: expecting Struct, got {:?}", cell))
        }
    }
}

impl<T:Parseable+Clone> Parseable for Vec<T> {
    fn from_cell(memory: &mut Memory, cell: MemoryCell) -> Result<Self,SemanticError> {
        match cell {
            MemoryCell::Struct(0, mut block) => {
                let mut vec = Vec::with_capacity(block.len());
                for item in block.drain(..) {
                    let mut processed = item.resolve_ref::<T>(memory)?;
                    vec.push(my(processed));
                }
                Ok(vec)
            }
            _ => SemanticError::msg(format!("Vec: expecting Struct, got {:?}", cell))
        }
    }
}

impl<T:Parseable+Clone,U:Parseable+Clone> Parseable for (T,U) {
    fn from_cell(memory: &mut Memory, cell: MemoryCell) -> Result<Self,SemanticError> {
        match cell {
            MemoryCell::Struct(0, block) => {
                if block.len() != 2 {
                    return SemanticError::msg(format!("tuple: expecting block length 2, got {}", block.len()));
                }
                let a = block[0].resolve_ref::<T>(memory)?;
                let b = block[1].resolve_ref::<U>(memory)?;
                Ok((my(a),my(b)))
            }
            _ => SemanticError::msg(format!("tuple: expecting Struct, got {:?}", cell))
        }
    }
}

#[derive(Debug,Clone)]
pub struct Wrapped<T> {
    item: T
}

impl <T:Parseable+Clone> Parseable for Wrapped<T> {
    fn from_cell(memory: &mut Memory, cell: MemoryCell) -> Result<Self,SemanticError> {
        match cell {
            MemoryCell::Struct(0, block) => {
                if block.len() != 1 {
                    return SemanticError::msg(format!("Wrapped: expecting block length 1, got {}", block.len()));
                }
                let item = block[0].resolve_ref::<T>(memory)?;
                Ok(Wrapped{item:my(item)})
            }
            _ => SemanticError::msg(format!("Wrapped: expecting Struct, got {:?}", cell))
        }
    }
}

#[derive(Debug)]
pub struct SummaryDisk {
    name: Rc<DirPath>,
    imports: Rc<Vec<DirPath>>,
    deps: Rc<Vec<(DirPath, Wrapped<Digest>)>>
}

impl Parseable for SummaryDisk {
    fn from_cell(memory: &mut Memory, cell: MemoryCell) -> Result<Self,SemanticError> {
        match cell {
            MemoryCell::Struct(0, block) => {
                if block.len() != 3 {
                    return SemanticError::msg(format!("SummaryDisk: expecting block length 3, got {}", block.len()));
                }
                let name = block[0].resolve_ref::<DirPath>(memory)?;
                let imports = block[1].resolve_ref::<Vec<DirPath>>(memory)?;
                let deps = block[2].resolve_ref::<Vec<(DirPath,Wrapped<Digest>)>>(memory)?;
                Ok(SummaryDisk{name:name, imports:imports, deps:deps})
            }
            _ => SemanticError::msg(format!("DirPath: expecting Struct, got {:?}", cell))
        }
    }

}

