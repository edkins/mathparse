use std::rc::Rc;

use nom::IResult;

use crate::parse::{Memory,SemanticError,E,string,fail,as_string,my,block2,block3,tuple2,nullable,vec,wrapped};
use crate::parse::{VoParseRef,unshare};
use vo_parse_derive::VoParse;

#[derive(Clone,VoParse)]
struct Foo {
    foo: String
}

impl VoParseRef for String {
    fn parse_ref<'b>(memory: &mut Memory, input: &'b[u8]) -> IResult<&'b[u8],Rc<Self>,E> {
        string(|data| {
            String::from_utf8(data).map_err(|e|SemanticError::new(format!("{:?}",e)))
        })(memory,input)
    }
}

impl<T:VoParseRef> VoParseRef for Rc<T> {
    fn parse_ref<'b>(memory: &mut Memory, input: &'b[u8]) -> IResult<&'b[u8],Rc<Self>,E> {
        let (i,rc) = T::parse_ref(memory, input)?;
        Ok((i, Rc::new(rc)))
    }
    fn parse_val<'b>(memory: &mut Memory, input: &'b[u8]) -> IResult<&'b[u8],Self,E> {
        T::parse_ref(memory, input)
    }
}

impl<T:VoParseRef+'static> VoParseRef for Vec<T> {
    fn parse_ref<'b>(memory: &mut Memory, input: &'b[u8]) -> IResult<&'b[u8],Rc<Self>,E> {
        vec(T::parse_val)(memory,input)
    }
}

impl<T:VoParseRef+'static,U:VoParseRef+'static> VoParseRef for (T,U) {
    fn parse_ref<'b>(memory: &mut Memory, input: &'b[u8]) -> IResult<&'b[u8],Rc<Self>,E> {
        tuple2(T::parse_val, U::parse_val)(memory,input)
    }
}


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

impl VoParseRef for DigestBytes {
    fn parse_ref<'b>(memory: &mut Memory, input: &'b[u8]) -> IResult<&'b[u8],Rc<Self>,E> {
        wrapped(my_digest)(memory,input)
    }
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

impl VoParseRef for DirPath {
    fn parse_ref<'b>(memory: &mut Memory, i: &'b[u8]) -> IResult<&'b[u8],Rc<Self>,E> {
        let (i,result) = nullable(block2(my_utf8,DirPath::parse_val,|s,d|Ok(d.concat(s))))(memory,i)?;
        if result.is_some() {
            Ok((i,result.unwrap()))
        } else {
            Ok((i,Rc::new(DirPath::empty())))
        }
    }
}

#[derive(Debug,Clone,VoParse)]
pub struct SummaryDisk {
    name: DirPath,
    imports: Vec<DirPath>,
    deps: Vec<(DirPath, DigestBytes)>
}

/*
pub fn my_summary_disk<'a,'b>(memory: &'a mut Memory, i: &'b[u8]) -> IResult<&'b[u8], SummaryDisk, E> {
    my(block3(
            my(dir_path),
            my(vec(my(dir_path))),
            my(vec(my(tuple2(my(dir_path),my(wrapped(my_digest)))))),
            |a,b,c|{ Ok(SummaryDisk{name:a, imports:b, deps:c}) }
    ))(memory,i)
}
*/

/*
#[derive(Debug,Clone)]
pub struct CompiledLibrary {
    name: DirPath,
    module: ModuleBody,
    deps: Vec<LibraryInfo>,
    engagement: Engagement,
    natsymbs: NativeValueSymbols
}

pub fn my_compiled_library<'a,'b>(memory: &'a mut Memory, i: &'b[u8]) -> IResult<&'b[u8], CompiledLibrary, E> {
    my(block5(
            my(dir_path),
            my_module_body,
            my(vec(my_library_info)),
            my_engagement,
            my_native_value_symbols,
            |a,b,c,d,e|{Ok(CompiledLibrary{name:a,module:b,deps:c,engagement:d,natsymbs:e})}
    ))(memory,i)
}


type lib_objects = Vec<(String,Obj)>;

#[derive(Debug,Clone)]
pub struct LibraryDisk {
    compiled: CompiledLibrary,
    objects: (Vec<(String,Obj)>, Vec<(String,Obj)>)
}

pub fn my_library_disk<'a,'b>(memory: &'a mut Memory, i: &'b[u8]) -> IResult<&'b[u8], LibraryDisk, E> {
    my(block2(
            my_compiled_library,
            my(tuple2(my_lib_objects, my_lib_objects)),
            |a,b|{Ok(LibraryDisk{compiled:a,objects:b})}
    ))(memory,i)
}

*/
