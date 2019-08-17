use nom::IResult;
use nom::combinator::all_consuming;
use nom::error::{ErrorKind,ParseError};
use nom::number::complete::be_u32;

const VO_MAGIC:u32 = 8991;

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

fn fail<'a>(input: &'a[u8], msg: &str) -> IResult<&'a[u8],(),E> {
    Err(nom::Err::Failure(E::new(input,msg)))
}

fn vo_magic(i: &[u8]) -> IResult<&[u8],(),E> {
    let (i, magic) = be_u32(i)?;
    if magic == VO_MAGIC {
        Ok((i,()))
    } else {
        fail(i,&format!("vo_magic {}", VO_MAGIC))
    }
}

fn file_contents(i: &[u8]) -> IResult<&[u8],(),E> {
    let (i,_) = vo_magic(i)?;
    Ok((i,()))
}

pub fn file(i: &[u8]) -> IResult<&[u8],(),E> {
    all_consuming(file_contents)(i)
}
