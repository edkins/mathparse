use std::rc::Rc;

use nom::IResult;

use crate::parse::{Memory,SemanticError,E,string,fail,as_string,my,block2,block3,tuple2,nullable,vec,wrapped};

#[derive(Clone)]
pub struct DigestBytes {
    bytes: [u8;16]
}

impl DigestBytes {
    pub fn new(slice: &[u8]) -> Self {
        let mut bytes = [0;16];
        bytes.copy_from_slice(&slice[..16]);
        DigestBytes{bytes:bytes}
    }
}

impl std::fmt::Debug for DigestBytes {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> Result<(),std::fmt::Error> {
        write!(f, "DigestBytes {:x?}", self.bytes)
    }
}

fn my_utf8<'a,'b>(memory: &'a mut Memory, i: &'b[u8]) -> IResult<&'b[u8], String, E> {
    my(string(|data| {
        String::from_utf8(data).map_err(|e|SemanticError::new(format!("{:?}",e)))
    }))(memory,i)
}

fn my_digest<'a,'b>(memory: &'a mut Memory, i: &'b[u8]) -> IResult<&'b[u8], DigestBytes, E> {
    my(string(|data| {
        if data.len() == 16 {
            Ok(DigestBytes::new(&data))
        } else {
            SemanticError::msg(format!("digest: expected string of length 16, got {}", as_string(&data)))
        }
    }))(memory,i)
}

#[derive(Clone)]
pub struct DirPath {
    segments: Vec<String>
}

impl std::fmt::Debug for DirPath {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> Result<(),std::fmt::Error> {
        write!(f, "DirPath {:?}", self.segments)
    }
}

impl DirPath {
    fn empty() -> Self {
        DirPath{segments:vec![]}
    }
    fn concat(&self, head: String) -> Self {
        let mut vec = Vec::with_capacity(self.segments.len() + 1);
        vec.extend_from_slice(&self.segments);
        vec.push(head);
        DirPath{segments:vec}
    }
}

fn dir_path<'a,'b>(memory: &'a mut Memory, i: &'b[u8]) -> IResult<&'b[u8], Rc<DirPath>, E> {
    let (i,result) = nullable(block2(my_utf8,dir_path,|s,d|Ok(d.concat(s))))(memory,i)?;
    if result.is_some() {
        Ok((i,result.unwrap()))
    } else {
        Ok((i,Rc::new(DirPath::empty())))
    }
}

#[derive(Debug,Clone)]
pub struct SummaryDisk {
    name: DirPath,
    imports: Vec<DirPath>,
    deps: Vec<(DirPath, DigestBytes)>
}

pub fn my_summary_disk<'a,'b>(memory: &'a mut Memory, i: &'b[u8]) -> IResult<&'b[u8], SummaryDisk, E> {
    my(block3(
            my(dir_path),
            my(vec(my(dir_path))),
            my(vec(my(tuple2(my(dir_path),my(wrapped(my_digest)))))),
            |a,b,c|{ Ok(SummaryDisk{name:a, imports:b, deps:c}) }
    ))(memory,i)
}

/*
#[derive(Debug)]
pub struct LibraryDisk {
    compiled: CompiledLibrary,
    objects: (Vec<(String,Obj)>, Vec<(String,Obj)>)
}

impl Parseable for LibraryDisk {
}
*/

