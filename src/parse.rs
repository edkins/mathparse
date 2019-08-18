use nom::IResult;
use nom::bytes::complete::{tag,take,take_till};
use nom::combinator::all_consuming;
use nom::error::{ErrorKind,ParseError};
use nom::number::complete::{be_i8,be_i16,be_i32,be_i64,be_u8,be_u24,be_u32,be_u64};

const VO_MAGIC:i32 = 8991;

#[allow(non_camel_case_types)]
type u63 = u64;

#[derive(Debug)]
pub struct E {
    pub stuff: Vec<(usize, String)>
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
enum Data {
    Int(i64),
    Ptr(usize),
    Atm(u8),
    Fun(i64)
}

#[derive(Debug,Clone)]
enum Obj {
    Struct(u8,Vec<Data>),
    Int63(u63),
    String(Vec<u8>)
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
            let (i,n) = be_i8(i)?;
            Ok((i,Repr::RPointer(n as usize)))
        }
        CODE_SHARED16 => {
            let (i,n) = be_i16(i)?;
            Ok((i,Repr::RPointer(n as usize)))
        }
        CODE_SHARED32 => {
            let (i,n) = be_i32(i)?;
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

fn fill_obj<'a>(memory: &mut Vec<Obj>, i: &'a[u8]) -> IResult<&'a[u8], Data, E> {
    print!("{:02x?}", &i[..i.len().min(20)]);
    let (i,r) = parse_object(i)?;
    match &r {
        Repr::RString(s) => println!("-> {:?}", std::str::from_utf8(&s)),
        _ => println!("-> {:?}", r)
    }
    match r {
        Repr::RPointer(n) => {
            let data = Data::Ptr(memory.len() - n);
            Ok((i,data))
        }
        Repr::RInt(n) => {
            let data = Data::Int(n);
            Ok((i,data))
        }
        Repr::RString(s) => {
            let data = Data::Ptr(memory.len());
            memory.push(Obj::String(s));
            Ok((i,data))
        }
        Repr::RBlock(tag,len) => {
            if len == 0 {
                let data = Data::Atm(tag);
                Ok((i,data))
            } else {
                let index = memory.len();
                let data = Data::Ptr(index);
                memory.push(Obj::Struct(tag, vec![]));
                let mut nblock = Vec::with_capacity(len);
                let mut i = i;
                for _ in 0..len {
                    let (newi, d) = fill_obj(memory, i)?;
                    i = newi;
                    nblock.push(d);
                }
                memory[index] = Obj::Struct(tag, nblock);
                Ok((i,data))
            }
        }
        Repr::RCode(addr) => {
            let data = Data::Fun(addr);
            Ok((i,data))
        }
        Repr::RInt63(n) => {
            let data = Data::Ptr(memory.len());
            memory.push(Obj::Int63(n));
            Ok((i,data))
        }
    }
}

fn print_data(memory: &[Obj], data: &Data, indent: usize) {
    for _ in 0..indent {
        print!(" ");
    }
    match data {
        Data::Ptr(i) => {
            print!("Ptr({}) -> ", i);
            match &memory[*i] {
                Obj::String(s) => println!("{:?}", std::str::from_utf8(&s)),
                Obj::Int63(n) => println!("{}", n),
                Obj::Struct(t,block) => {
                    println!("Struct({})", t);
                    for item in block {
                        print_data(memory, item, indent + 4);
                    }
                }
            }
        }
        Data::Int(n) => println!("Int({})", n),
        Data::Atm(t) => println!("Atm({})", t),
        Data::Fun(a) => println!("Fun({})", a)
    }
}

fn segment(i: &[u8]) -> IResult<&[u8],(),E> {
    let (i,_stop) = be_i32(i)?;
    let (i,(len,_,_,size)) = header(i)?;
    let orig_pos = i.len();
    let mut memory:Vec<Obj> = Vec::with_capacity(size as usize);
    let (i,root) = fill_obj(&mut memory, i)?;
    print_data(&memory, &root, 0);
    if memory.len() != size as usize {
        fail::<()>(i, &format!("Memory should be length {}, was actually {}", size, memory.len()))?;
    }
    if orig_pos - i.len() != len as usize {
        fail::<()>(i, &format!("Expected to consume {} bytes, actually consumed {}", len, orig_pos - i.len()))?;
    }
    Ok((i,()))
}

fn file_contents(i: &[u8]) -> IResult<&[u8],(),E> {
    let (i,_) = vo_magic(i)?;
    let (i,_) = segment(i)?;
    Ok((i,()))
}

pub fn file(i: &[u8]) -> IResult<&[u8],(),E> {
    all_consuming(file_contents)(i)
}
