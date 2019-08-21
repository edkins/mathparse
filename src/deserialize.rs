use serde::Deserialize;
use serde::de::Deserializer;

use crate::parse::{Memory,E};

pub struct VoDeserializer<'de> {
    input: &'de [u8],
    memory: Memory
}

impl<'de> VoDeserializer<'de> {
    pub fn from_bytes_with_capacity(input: &'de [u8], capacity: usize) -> Self {
        VoDeserializer{
            input: input,
            memory: Memory::with_capacity(usize)
        }
    }
}

type Err = nom::Err<E>;

impl<'de,'a> Deserializer<'de> for &'a mut VoDeserializer<'de> {
    type Error = nom::Err<E>;
    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value,Err> where V:Visitor<'de> {
        let (i,r) = parse_object(self.input)?;
        self.input = i;
        match r {

        }
    }
}
