use std::any::Any;
use std::mem::swap;
use std::rc::Rc;

use md5::{Md5,Digest};
use nom::IResult;
use nom::bytes::complete::{tag,take,take_till};
use nom::combinator::all_consuming;
use nom::error::{ErrorKind,ParseError};
use nom::number::complete::{be_i8,be_i16,be_i32,be_i64,be_u8,be_u16,be_u24,be_u32,be_u64};

use crate::types::DigestBytes;
use crate::types::SummaryDisk;

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
    fn new(input: &[u8], msg: String) -> Self {
        E{ 
            stuff: vec![(input.len(), msg)]
        }
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

pub fn fail<'a,T>(input: &'a[u8], msg: String) -> IResult<&'a[u8],T,E> {
    Err(nom::Err::Failure(E::new(input,msg)))
}

//////////////////////////////////////////////////////

pub trait VoParseRef where Self:Sized+Clone {
    fn parse_ref<'b>(memory: &mut Memory, input: &'b[u8]) -> IResult<&'b[u8],Rc<Self>,E>;
    fn parse_val<'b>(memory: &mut Memory, input: &'b[u8]) -> IResult<&'b[u8],Self,E> {
        let (i,rc) = Self::parse_ref(memory, input)?;
        Ok((i,unshare(rc)))
    }
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

pub struct Memory {
    cells: Vec<Option<Rc<dyn Any>>>
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

impl Memory {
    fn with_capacity(size: usize) -> Self {
        Memory{cells: Vec::with_capacity(size)}
    }
    fn len(&self) -> usize {
        self.cells.len()
    }
    fn push<T:'static>(&mut self, rc: Rc<T>) {
        self.cells.push(Some(rc))
    }
    fn point_back2<T:'static>(&mut self, offset: usize) -> Result<Rc<T>,SemanticError> {
        let index = self.cells.len() - offset;
        if index >= self.cells.len() {
            return SemanticError::msg(format!("Pointer is to next object, is this allowed?"));
        }
        match &self.cells[index] {
            Some(rc) => rc.clone().downcast().map_err(|rc|SemanticError::new(format!("downcasting error on pointer"))),
            _ => SemanticError::msg(format!("Pointer is to object that we haven't finished building, is this allowed?"))
        }
    }
    fn reserve_for_struct(&mut self) -> usize {
        self.cells.push(None);
        self.cells.len() - 1
    }
    fn backfill_struct2<T:'static>(&mut self, addr: usize, data: T) -> Rc<T> {
        match self.cells[addr] {
            None => {
                let rc = Rc::new(data);
                self.cells[addr] = Some(rc.clone());
                rc
            }
            _ => panic!("backfill_struct: expecting cell to be under construction")
        }
    }
}

//////////////////////////////////////////////////////

fn vo_magic(i: &[u8]) -> IResult<&[u8],(),E> {
    let (i, magic) = be_i32(i)?;
    if magic == VO_MAGIC {
        Ok((i,()))
    } else {
        fail(i,format!("vo_magic {}", VO_MAGIC))
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
        fail(i, format!("uint63 out of range: {}", n))
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
                _ => fail(i, format!("Unhandled custom code: {:?}", std::str::from_utf8(string)))
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
            fail(i, format!("Unhandled code: {:02x}", data))
        }
    }
}

pub fn string<'b,F,T:'static>(f:F) -> impl Fn(&mut Memory, &'b[u8]) -> IResult<&'b[u8],Rc<T>,E>
    where F:Fn(Vec<u8>) -> Result<T,SemanticError>
{
    move|memory,i| {
        let (i,r) = parse_object(i)?;
        match r {
            Repr::RPointer(n) => {
                let rc = memory.point_back2(n).map_err(|e|e.to_nom(i))?;
                Ok((i,rc))
            }
            Repr::RString(s) => {
                let data = f(s).map_err(|e|e.to_nom(i))?;
                let rc = Rc::new(data);
                memory.push(rc.clone());
                Ok((i,rc))
            }
            _ => fail(i, format!("Expected string or pointer to string, got {:?}", r))
        }
    }
}

pub fn int<'b,'a>(memory: &'a mut Memory, i:&'b[u8]) -> IResult<&'b[u8],i64,E>
{
    let (i,r) = parse_object(i)?;
    match r {
        Repr::RInt(n) => {
            Ok((i,n))
        }
        _ => fail(i, format!("Expected int, got {:?}", r))
    }
}

pub fn block<'b,F,T:'static>(f:F) -> impl Fn(&mut Memory, &'b[u8]) -> IResult<&'b[u8],Rc<T>,E>
    where F:Fn(usize, &mut Memory, &'b[u8]) -> IResult<&'b[u8],T,E>
{
    move|memory,i| {
        let (i,r) = parse_object(i)?;
        match r {
            Repr::RPointer(n) => {
                let rc = memory.point_back2(n).map_err(|e|e.to_nom(i))?;
                Ok((i,rc))
            }
            Repr::RBlock(0,len) if len>0 => {
                let index = memory.reserve_for_struct();
                let (i,data) = f(len, memory, i)?;
                let rc = memory.backfill_struct2(index, data);
                Ok((i,rc))
            }
            _ => fail(i, format!("Expected block or pointer to array, got {:?}", r))
        }
    }
}

pub fn vec<'b,F,T:'static>(f:F) -> impl Fn(&mut Memory, &'b[u8]) -> IResult<&'b[u8],Rc<Vec<T>>,E>
    where F:Fn(&mut Memory, &'b[u8]) -> IResult<&'b[u8],T,E>
{
    block(move|len,memory,i| {
        let mut nblock = Vec::with_capacity(len);
        let mut i = i;
        for _ in 0..len {
            let (newi, d) = f(memory, i)?;
            i = newi;
            nblock.push(d);
        }
        Ok((i,nblock))
    })
}

pub fn block1<'b,F,M,T:'static,R:'static>(f:F,m:M) -> impl Fn(&mut Memory, &'b[u8]) -> IResult<&'b[u8],Rc<R>,E>
    where F:Fn(&mut Memory, &'b[u8]) -> IResult<&'b[u8],T,E>,
          M:Fn(T) -> Result<R,SemanticError>
{
    block(move|len,memory,i| {
        if len == 1 {
            let (i,a) = f(memory, i)?;
            let data = m(a).map_err(|e|e.to_nom(i))?;
            Ok((i,data))
        } else {
            fail(i, format!("tuple1: actual block length was {}", len))
        }
    })
}

pub fn block2<'b,F,G,M,T:'static,U:'static,R:'static>(f:F,g:G,m:M) -> impl Fn(&mut Memory, &'b[u8]) -> IResult<&'b[u8],Rc<R>,E>
    where F:Fn(&mut Memory, &'b[u8]) -> IResult<&'b[u8],T,E>,
          G:Fn(&mut Memory, &'b[u8]) -> IResult<&'b[u8],U,E>,
          M:Fn(T,U) -> Result<R,SemanticError>
{
    block(move|len,memory,i| {
        if len == 2 {
            let (i,a) = f(memory, i)?;
            let (i,b) = g(memory, i)?;
            let data = m(a,b).map_err(|e|e.to_nom(i))?;
            Ok((i,data))
        } else {
            fail(i, format!("tuple2: actual block length was {}", len))
        }
    })
}

pub fn block3<'b,F,G,H,M,T:'static,U:'static,V:'static,R:'static>(f:F,g:G,h:H,m:M) -> impl Fn(&mut Memory, &'b[u8]) -> IResult<&'b[u8],Rc<R>,E>
    where F:Fn(&mut Memory, &'b[u8]) -> IResult<&'b[u8],T,E>,
          G:Fn(&mut Memory, &'b[u8]) -> IResult<&'b[u8],U,E>,
          H:Fn(&mut Memory, &'b[u8]) -> IResult<&'b[u8],V,E>,
          M:Fn(T,U,V) -> Result<R,SemanticError>
{
    block(move|len,memory,i| {
        if len == 3 {
            let (i,a) = f(memory, i)?;
            let (i,b) = g(memory, i)?;
            let (i,c) = h(memory, i)?;
            let data = m(a,b,c).map_err(|e|e.to_nom(i))?;
            Ok((i,data))
        } else {
            fail(i, format!("tuple3: actual block length was {}", len))
        }
    })
}

pub fn block5<'b,F,G,H,I,J,M,T:'static,U:'static,V:'static,W:'static,X:'static,R:'static>(f:F,g:G,h:H,i:I,j:J,m:M)
    -> impl Fn(&mut Memory, &'b[u8]) -> IResult<&'b[u8],Rc<R>,E>
    where F:Fn(&mut Memory, &'b[u8]) -> IResult<&'b[u8],T,E>,
          G:Fn(&mut Memory, &'b[u8]) -> IResult<&'b[u8],U,E>,
          H:Fn(&mut Memory, &'b[u8]) -> IResult<&'b[u8],V,E>,
          I:Fn(&mut Memory, &'b[u8]) -> IResult<&'b[u8],W,E>,
          J:Fn(&mut Memory, &'b[u8]) -> IResult<&'b[u8],X,E>,
          M:Fn(T,U,V,W,X) -> Result<R,SemanticError>
{
    block(move|len,memory,input| {
        if len == 5 {
            let (input,a) = f(memory, input)?;
            let (input,b) = g(memory, input)?;
            let (input,c) = h(memory, input)?;
            let (input,d) = i(memory, input)?;
            let (input,e) = j(memory, input)?;
            let data = m(a,b,c,d,e).map_err(|err|err.to_nom(input))?;
            Ok((input,data))
        } else {
            fail(input, format!("tuple3: actual block length was {}", len))
        }
    })
}

pub fn wrapped<'b,F,T:'static>(f:F) -> impl Fn(&mut Memory, &'b[u8]) -> IResult<&'b[u8],Rc<T>,E>
    where F:Fn(&mut Memory, &'b[u8]) -> IResult<&'b[u8],T,E>
{
    block1(f,|a|Ok(a))
}

pub fn tuple2<'b,F,G,T:'static,U:'static>(f:F,g:G) -> impl Fn(&mut Memory, &'b[u8]) -> IResult<&'b[u8],Rc<(T,U)>,E>
    where F:Fn(&mut Memory, &'b[u8]) -> IResult<&'b[u8],T,E>,
          G:Fn(&mut Memory, &'b[u8]) -> IResult<&'b[u8],U,E>
{
    block2(f,g,|a,b|Ok((a,b)))
}

pub fn unshare<T:Clone>(rc: Rc<T>) -> T {
    match Rc::try_unwrap(rc) {
        Ok(item) => item,
        Err(rc) => (*rc).clone()
    }
}

pub fn my<'b,F,T:Clone+'static>(f:F) -> impl Fn(&mut Memory, &'b[u8]) -> IResult<&'b[u8],T,E>
    where F:Fn(&mut Memory, &'b[u8]) -> IResult<&'b[u8],Rc<T>,E>,
{
    move|memory,i| {
        let (i,rc) = f(memory,i)?;
        Ok((i, unshare(rc)))
    }
}

// Treats int(0) as a special null value
pub fn nullable<'b,F,T:Clone+'static>(f:F) -> impl Fn(&mut Memory, &'b[u8]) -> IResult<&'b[u8],Option<T>,E>
    where F:Fn(&mut Memory, &'b[u8]) -> IResult<&'b[u8],T,E>,
{
    move|memory,i| {
        let (newi,r) = parse_object(i)?;
        match r {
            Repr::RInt(0) => {
                Ok((newi,None))
            }
            _ => {
                // backtrack
                let (i, data) = f(memory,i)?;
                Ok((i, Some(data)))
            }
        }
    }
}


pub fn as_string(string: &[u8]) -> String {
    let result = std::str::from_utf8(string);
    if result.is_ok() {
        result.unwrap().to_string()
    } else {
        format!("{:?}", string)
    }
}

fn segment<'b,'a:'b,F,T:Clone+Sized+'static>(f:F, file_len: usize, i:&'b[u8]) -> IResult<&'b[u8],(T,usize,DigestBytes),E>
    where F:Fn(&mut Memory, &'b[u8]) -> IResult<&'b[u8],T,E>
{
    let (i,stop) = be_i32(i)?;
    let (i,(len,_,_,size)) = header(i)?;
    let orig_pos = i.len();
    let mut memory= Memory::with_capacity(size as usize);
    let (i,obj) = f(&mut memory,i)?;
    if memory.len() != size as usize {
        return fail(i, format!("Memory should be length {}, was actually {}", size, memory.len()));
    }
    if orig_pos - i.len() != len as usize {
        return fail(i, format!("Expected to consume {} bytes, actually consumed {}", len, orig_pos - i.len()));
    }
    if file_len - i.len() != stop as usize {
        return fail(i, format!("Expected to stop at {}, actually stopped at {}", stop, file_len - i.len()));
    }
    let (i,digest) = take(16usize)(i)?;

    Ok((i,(obj,stop as usize,DigestBytes::new(digest))))
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
    let (i,(summary_disk,_,_)) = segment(SummaryDisk::parse_val,file_len,i)?;
    debug!("{:#?}", summary_disk);
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
