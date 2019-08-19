use std::any::Any;
use std::mem::swap;
use std::rc::Rc;

use md5::{Md5,Digest};
use nom::IResult;
use nom::bytes::complete::{tag,take,take_till};
use nom::combinator::all_consuming;
use nom::error::{ErrorKind,ParseError};
use nom::number::complete::{be_i8,be_i16,be_i32,be_i64,be_u8,be_u16,be_u24,be_u32,be_u64};

use crate::types::{Parseable,SummaryDisk};

const VO_MAGIC:i32 = 8991;

#[allow(non_camel_case_types)]
type u63 = u64;

#[derive(Debug)]
pub struct E {
    pub stuff: Vec<(usize, String)>
}

impl E {
    pub fn msg<T>(msg: String, i:&[u8]) -> Result<T,Self> {
        Err(E{stuff:vec![(i.len(), msg)]})
    }
    pub fn len(actual: usize, expected: usize, name: &str, i:&[u8]) -> Result<(),Self> {
        E::msg(format!("Struct {}: expected size {}, got size {}", name, expected, actual), i)
    }
}

impl<'a> ParseError<&'a[u8]> for E {
    fn from_error_kind(input: &'a[u8], kind: ErrorKind) -> Self {
        E {
            stuff: vec![(input.len(), format!("{:?}", kind))]
        }
    }
    fn append(input: &'a[u8], kind: ErrorKind, mut other: Self) -> Self {
        other.stuff.push((input.len(), format!("{:?}", kind)));
        other
    }
}

impl E {
    fn new(input: &[u8], msg: &str) -> Self {
        E{ 
            stuff: vec![(input.len(), msg.to_string())]
        }
    }
}

fn fail<'a,T>(input: &'a[u8], msg: &str) -> IResult<&'a[u8],T,E> {
    Err(nom::Err::Failure(E::new(input,msg)))
}

//////////////////////////////////////////////////////

#[derive(Debug,Clone)]
enum Repr {
    RInt(i64),
    RInt63(u63),
    RBlock(u8,usize),
    RString(Vec<u8>),
    RPointer(usize),
    RCode(i64)
}

const CODE_INT8:u8 = 0;
const CODE_INT16:u8 = 1;
const CODE_INT32:u8 = 2;
const CODE_INT64:u8 = 3;
const CODE_SHARED8:u8 = 4;
const CODE_SHARED16:u8 = 5;
const CODE_SHARED32:u8 = 6;
const CODE_DOUBLE_ARRAY32_LITTLE:u8 = 7;
const CODE_BLOCK32:u8 = 8;
const CODE_STRING8:u8 = 9;
const CODE_STRING32:u8 = 10;
const CODE_DOUBLE_BIG:u8 = 11;
const CODE_DOUBLE_LITTLE:u8 = 12;
const CODE_DOUBLE_ARRAY8_BIG:u8 = 13;
const CODE_DOUBLE_ARRAY8_LITTLE:u8 = 14;
const CODE_DOUBLE_ARRAY32_BIG:u8 = 15;
const CODE_CODEPOINTER:u8 = 16;
const CODE_INFIXPOINTER:u8 = 17;
const CODE_CUSTOM:u8 = 18;
const CODE_BLOCK64:u8 = 19;

#[derive(Debug,Clone)]
pub enum Data {
    Int(i64),
    Ptr(usize),
    Atm(u8)
}

pub enum MemoryCell {
    ConstructionPhase1,
    Struct(u8,Vec<Data>),
    Int63(u63),
    String(Vec<u8>),
    ConstructionPhase2,
    Ref(Rc<dyn Any>)
}

pub struct Memory {
    cells: Vec<MemoryCell>
}

pub struct SemanticError {
    msg: String
}

impl SemanticError {
    pub fn new(msg:String) -> Self {
        SemanticError{msg:msg}
    }
    pub fn msg<T>(msg:String) -> Result<T,Self> {
        Err(SemanticError::new(msg))
    }
    fn to_nom(self, i:&[u8]) -> nom::Err<E> {
        nom::Err::Failure(E{stuff:vec![(i.len(), self.msg)]})
    }
}

impl Data {
    pub fn resolve_nullable<T:Parseable>(&self, memory: &mut Memory) -> Result<Option<Rc<T>>,SemanticError> {
        match self {
            Data::Ptr(addr) => Ok(Some(memory.resolve_ptr(*addr)?)),
            Data::Int(0) => Ok(None),
            _ => Err(SemanticError{msg:format!("resolve_nullable: expected ptr or int(0)")})
        }
    }
    pub fn resolve_ref<T:Parseable>(&self, memory: &mut Memory) -> Result<Rc<T>,SemanticError> {
        match self {
            Data::Ptr(addr) => memory.resolve_ptr(*addr),
            _ => Err(SemanticError{msg:format!("resolve_ref: expected ptr")})
        }
    }
    pub fn resolve_int<T:Parseable>(&self) -> Result<i64,SemanticError> {
        match self {
            Data::Int(n) => Ok(*n),
            _ => Err(SemanticError{msg:format!("resolve_int: expected int")})
        }
    }
}

impl std::fmt::Debug for MemoryCell {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> Result<(),std::fmt::Error> {
        match self {
            MemoryCell::ConstructionPhase1 => write!(f, "ConstructionPhase1")?,
            MemoryCell::Struct(tag,data) => write!(f, "Struct({},{:?})", tag, data)?,
            MemoryCell::Int63(n) => write!(f, "Int63({})", n)?,
            MemoryCell::String(data) => write!(f, "String({})", as_string(&data))?,
            MemoryCell::ConstructionPhase2 => write!(f, "ConstructionPhase2")?,
            MemoryCell::Ref(_) => write!(f, "Ref()")?,
        }
        Ok(())
    }
}

impl MemoryCell {
    fn is_under_construction1(&self) -> bool {
        match self {
            MemoryCell::ConstructionPhase1 => true,
            MemoryCell::ConstructionPhase2 => panic!(),
            MemoryCell::Ref(_) => panic!(),
            _ => false
        }
    }
}

impl Memory {
    fn with_capacity(size: usize) -> Self {
        Memory{cells: Vec::with_capacity(size)}
    }
    fn len(&self) -> usize {
        self.cells.len()
    }
    fn point_back(&mut self, offset: usize) -> Result<Data,SemanticError> {
        let index = self.cells.len() - offset;
        if index >= self.cells.len() {
            return SemanticError::msg(format!("Pointer is to next object, is this allowed?"));
        }
        if self.cells[index].is_under_construction1() {
            return SemanticError::msg(format!("Pointer is to object that we haven't finished building, is this allowed?"));
        }
        Ok(Data::Ptr(index))
    }
    fn add_string(&mut self, data: Vec<u8>) -> Data {
        self.cells.push(MemoryCell::String(data));
        Data::Ptr(self.cells.len() - 1)
    }
    fn add_int63(&mut self, n: u63) -> Data {
        self.cells.push(MemoryCell::Int63(n));
        Data::Ptr(self.cells.len() - 1)
    }
    fn reserve_for_struct(&mut self) -> usize {
        self.cells.push(MemoryCell::ConstructionPhase1);
        self.cells.len() - 1
    }
    fn backfill_struct(&mut self, addr: usize, tag: u8, data: Vec<Data>) -> Data {
        match self.cells[addr] {
            MemoryCell::ConstructionPhase1 => {
                self.cells[addr] = MemoryCell::Struct(tag, data);
                Data::Ptr(addr)
            }
            _ => panic!("backfill_struct: expecting cell to be in state ConstructionPhase1")
        }
    }
    fn resolve_ptr<T:Parseable>(&mut self, addr: usize) -> Result<Rc<T>,SemanticError> {
        match &self.cells[addr] {
            MemoryCell::Ref(rc) => {
                return rc.clone().downcast().map_err(|rc|SemanticError::new(format!("downcasting error on pointer")));
            }
            _ => {}
        }

        let mut cell = MemoryCell::ConstructionPhase2;
        swap(&mut cell, &mut self.cells[addr]);

        let t = T::from_cell(self, cell)?;
        let rc = Rc::new(t);
        self.cells[addr] = MemoryCell::Ref(rc.clone());
        Ok(rc)
    }
}

//////////////////////////////////////////////////////

fn vo_magic(i: &[u8]) -> IResult<&[u8],(),E> {
    let (i, magic) = be_i32(i)?;
    if magic == VO_MAGIC {
        Ok((i,()))
    } else {
        fail(i,&format!("vo_magic {}", VO_MAGIC))
    }
}

fn header(i: &[u8]) -> IResult<&[u8],(i32,i32,i32,i32),E> {
    let (i,_) = tag(&[132,149,166,190])(i)?;  // magic
    let (i,length) = be_i32(i)?;
    let (i,objects) = be_i32(i)?;
    let (i,size32) = be_i32(i)?;
    let (i,size64) = be_i32(i)?;
    Ok((i,(length,size32,size64,objects)))
}

fn header32(i: &[u8]) -> IResult<&[u8],(u8,usize),E> {
    let (i,len) = be_u24(i)?;
    let (i,tag) = be_u8(i)?;
    Ok((i,(tag,(len >> 2) as usize)))
}

fn header64(i: &[u8]) -> IResult<&[u8],(u8,usize),E> {
    let (i,data) = be_u64(i)?;
    let tag = (data & 0xff) as u8;
    let len = (data >> 10) as usize;
    Ok((i,(tag,len)))
}

fn cstring(i: &[u8]) -> IResult<&[u8],&[u8],E> {
    let (i,string) = take_till(|b|b==0)(i)?;
    Ok((&i[1..],string))
}

fn be_u63(i: &[u8]) -> IResult<&[u8], u63, E> {
    let (i,n) = be_i64(i)?;
    if n < 0 {
        fail(i, &format!("uint63 out of range: {}", n))
    } else {
        Ok((i,n as u63))
    }
}

fn parse_object(i: &[u8]) -> IResult<&[u8],Repr,E> {
    let (i,data) = be_u8(i)?;
    match data {
        (0x80..=0xff) => {
            Ok((i,Repr::RBlock(data & 0xf, ((data >> 4) & 0x7) as usize)))
        }
        0x40..=0x7f => {
            Ok((i,Repr::RInt(data as i64 & 0x3f)))
        }
        0x20..=0x3f => {
            let (i, string) = take((data & 0x1f) as usize)(i)?;
            Ok((i,Repr::RString(string.to_vec())))
        }
        CODE_INT8 => {
            let (i,n) = be_i8(i)?;
            Ok((i,Repr::RInt(n as i64)))
        }
        CODE_INT16 => {
            let (i,n) = be_i16(i)?;
            Ok((i,Repr::RInt(n as i64)))
        }
        CODE_INT32 => {
            let (i,n) = be_i32(i)?;
            Ok((i,Repr::RInt(n as i64)))
        }
        CODE_INT64 => {
            let (i,n) = be_i64(i)?;
            Ok((i,Repr::RInt(n)))
        }
        CODE_SHARED8 => {
            let (i,n) = be_u8(i)?;
            Ok((i,Repr::RPointer(n as usize)))
        }
        CODE_SHARED16 => {
            let (i,n) = be_u16(i)?;
            Ok((i,Repr::RPointer(n as usize)))
        }
        CODE_SHARED32 => {
            let (i,n) = be_u32(i)?;
            Ok((i,Repr::RPointer(n as usize)))
        }
        CODE_BLOCK32 => {
            let (i,(tag,len)) = header32(i)?;
            Ok((i,Repr::RBlock(tag,len)))
        }
        CODE_BLOCK64 => {
            let (i,(tag,len)) = header64(i)?;
            Ok((i,Repr::RBlock(tag,len)))
        }
        CODE_STRING8 => {
            let (i,len) = be_u8(i)?;
            let (i,string) = take(len as usize)(i)?;
            Ok((i,Repr::RString(string.to_vec())))
        }
        CODE_STRING32 => {
            let (i,len) = be_u32(i)?;
            let (i,string) = take(len)(i)?;
            Ok((i,Repr::RString(string.to_vec())))
        }
        CODE_CODEPOINTER => {
            let (i,addr) = be_u32(i)?;
            let (i,_) = take(16usize)(i)?;
            Ok((i,Repr::RCode(addr as i64)))
        }
        CODE_CUSTOM => {
            let (i,string) = cstring(i)?;
            match string {
                b"_j" => {
                    let (i,n) = be_u63(i)?;
                    Ok((i,Repr::RInt63(n)))
                }
                _ => fail(i, &format!("Unhandled custom code: {:?}", std::str::from_utf8(string)))
            }
        }
        CODE_DOUBLE_ARRAY32_LITTLE|
            CODE_DOUBLE_BIG|
            CODE_DOUBLE_LITTLE|
            CODE_DOUBLE_ARRAY8_BIG|
            CODE_DOUBLE_ARRAY8_LITTLE|
            CODE_DOUBLE_ARRAY32_BIG|
            CODE_INFIXPOINTER|
            20..=31 =>
        {
            fail(i, &format!("Unhandled code: {:02x}", data))
        }
    }
}

pub fn fill_obj<'a>(memory: &mut Memory, i: &'a[u8]) -> IResult<&'a[u8], Data, E> {
    let (i,r) = parse_object(i)?;
    match r {
        Repr::RPointer(n) => {
            let data = memory.point_back(n).map_err(|e|e.to_nom(i))?;
            Ok((i,data))
        }
        Repr::RInt(n) => {
            let data = Data::Int(n);
            Ok((i,data))
        }
        Repr::RString(s) => {
            let data = memory.add_string(s);
            Ok((i,data))
        }
        Repr::RBlock(tag,len) => {
            if len == 0 {
                let data = Data::Atm(tag);
                Ok((i,data))
            } else {
                let index = memory.reserve_for_struct();
                let mut nblock = Vec::with_capacity(len);
                let mut i = i;
                for _ in 0..len {
                    let (newi, d) = fill_obj(memory, i)?;
                    i = newi;
                    nblock.push(d);
                }
                let data = memory.backfill_struct(index, tag, nblock);
                Ok((i,data))
            }
        }
        Repr::RCode(_addr) => {
            fail(i, "We shouldn't serialize closures?")
        }
        Repr::RInt63(n) => {
            let data = memory.add_int63(n);
            Ok((i,data))
        }
    }
}

fn as_string(string: &[u8]) -> String {
    let result = std::str::from_utf8(string);
    if result.is_ok() {
        result.unwrap().to_string()
    } else {
        format!("{:?}", string)
    }
}
/*

fn print_data(data: &Data, indent: usize) {
    for _ in 0..indent {
        print!(" ");
    }
    match data {
        Data::Ptr(rc) => {
            print!("Ptr -> ");
            match &**rc {
                Obj::String(s) => println!("{}", as_string(s)),
                Obj::Int63(n) => println!("{}", n),
                Obj::Struct(t,block) => {
                    println!("Struct({})", t);
                    for item in block {
                        print_data(item, indent + 4);
                    }
                }
            }
        }
        Data::Int(n) => println!("Int({})", n),
        Data::Atm(t) => println!("Atm({})", t)
    }
}
*/

fn segment<T:Parseable>(file_len: usize, i: &[u8]) -> IResult<&[u8],(Rc<T>,usize,&[u8]),E> {
    let (i,stop) = be_i32(i)?;
    let (i,(len,_,_,size)) = header(i)?;
    let orig_pos = i.len();
    let mut memory= Memory::with_capacity(size as usize);
    let (i,root) = fill_obj(&mut memory, i)?;
    if memory.len() != size as usize {
        fail::<()>(i, &format!("Memory should be length {}, was actually {}", size, memory.len()))?;
    }
    if orig_pos - i.len() != len as usize {
        fail::<()>(i, &format!("Expected to consume {} bytes, actually consumed {}", len, orig_pos - i.len()))?;
    }
    if file_len - i.len() != stop as usize {
        fail::<()>(i, &format!("Expected to stop at {}, actually stopped at {}", stop, file_len - i.len()))?;
    }
    let (i,digest) = take(16usize)(i)?;

    let obj = root.resolve_ref::<T>(&mut memory).map_err(|e|e.to_nom(i))?;

    Ok((i,(obj,stop as usize,digest)))
}

fn md5(i: &[u8]) -> Vec<u8> {
    let mut hasher = Md5::new();
    hasher.input(i);
    hasher.result().to_vec()
}

fn file_contents(i: &[u8]) -> IResult<&[u8],(),E> {
    let entire_file = i;
    let file_len = i.len();
    let (i,_) = vo_magic(i)?;
    let (i,(summary_disk,_,_)) = segment::<SummaryDisk>(file_len,i)?;
    debug!("{:?}", summary_disk);
/*    let (i,(_library_disk,_,digest)) = segment(file_len,i)?;
    let (i,(_opaque_csts,_,udg)) = segment(file_len,i)?;
    let (i,(_tasks,_,_)) = segment(file_len,i)?;
    let (i,(_table,pos,checksum)) = segment(file_len,i)?;

    let actual_checksum = md5(&entire_file[..pos]);
    if actual_checksum != checksum {
        fail::<()>(i, &format!("Checksum mismatch. Should be {:?}, was {:?}", checksum, actual_checksum))?;
    }
    debug!("pos = {}, checksum = {:?}", pos, checksum);*/
    Ok((i,()))
}

pub fn file(i: &[u8]) -> IResult<&[u8],(),E> {
    all_consuming(file_contents)(i)
}
