#[macro_use]
extern crate quote;
extern crate proc_macro;

use proc_macro::TokenStream;
use syn;

#[proc_macro_derive(VoParse)]
pub fn vo_parse(input: TokenStream) -> TokenStream {
    let ast = syn::parse(input).unwrap();
    impl_vo_parse(&ast)
}

fn impl_vo_parse(ast: &syn::DeriveInput) -> TokenStream {
    let name = &ast.ident;
    let mut length = 0usize;
    let mut field_parsers = vec![];
    let mut field_initializers = vec![];

    match &ast.data {
        syn::Data::Struct(ds) => {
            match &ds.fields {
                syn::Fields::Named(fnamed) => {
                    for field in &fnamed.named {
                        let fname = field.ident.as_ref().unwrap();
                        let ftype = &field.ty;
                        let varname = format_ident!("data_{}", fname);
                        let fsyntax = quote!{
                            let (i,#varname) = <#ftype>::parse_val(memory, i)?;
                        };
                        field_parsers.push(fsyntax);
                        let fsyntax = quote!{ #fname: #varname, };
                        field_initializers.push(fsyntax);
                        length += 1;
                    }
                }
                _ => panic!("Struct fields must be named for VoParse")
            }
        }
        _ => panic!("Cannot only VoParse on struct, not enum")
    }

    let gen = quote! {
        impl crate::parse::VoParseRef for #name {
            fn parse_ref<'b>(memory: &mut Memory, input: &'b[u8]) -> IResult<&'b[u8],Rc<Self>,E> {
                crate::parse::block(move|len,memory,i| {
                    if len == #length {
                        #(#field_parsers)*
                        let data = #name{ #(#field_initializers)* };
                        Ok((i,data))
                    } else {
                        fail(i, format!("{}: expected block length was {}, actual block length was {}", stringify!(#name), #length, len))
                    }
                })(memory,input)
            }
        }
    };
    gen.into()
}

