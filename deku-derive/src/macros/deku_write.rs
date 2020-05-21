use crate::DekuReceiver;
use proc_macro2::TokenStream;
use quote::quote;

pub(crate) fn emit_deku_write(input: &DekuReceiver) -> Result<TokenStream, darling::Error> {
    let mut tokens = TokenStream::new();
    let ident = &input.ident;

    let fields = input
        .data
        .as_ref()
        .take_struct()
        .expect("expected `struct` type")
        .fields;

    let mut field_writes = vec![];
    let mut field_overwrites = vec![];

    for (i, f) in fields.into_iter().enumerate() {
        let field_endian = f.endian.unwrap_or(input.endian);
        let field_bits = f.bits;
        let field_bytes = f.bytes;
        let field_writer = f.writer.as_ref().map(|fn_str| {
            let fn_ident: TokenStream = fn_str.parse().unwrap();

            // TODO: Assert the shape of fn_ident? Only allow a structured function call instead of anything?
            quote! { #fn_ident; }
        });

        let field_len = &f
            .len
            .as_ref()
            .map(|v| syn::Ident::new(&v, syn::export::Span::call_site()));

        // Support named or indexed fields
        let field_ident = f
            .ident
            .as_ref()
            .map(|v| quote!(#v))
            .or_else(|| {
                Some({
                    let i = syn::Index::from(i);
                    quote!(#i)
                })
            })
            .map(|v| Some(quote! { input.#v }))
            .unwrap();

        // If `len` attr is provided, overwrite the field with the .len() of the container
        if let Some(field_len) = field_len {
            field_overwrites.push(quote! {
                // TODO: make write return a Result
                input.#field_len = #field_ident.len().try_into().unwrap();
            });
        }

        let endian_flip = field_endian != input.endian;

        if field_bits.is_some() && field_bytes.is_some() {
            return Err(darling::Error::duplicate_field(
                "both \"bits\" and \"bytes\" specified",
            ));
        }
        let field_bits = match field_bits.or_else(|| field_bytes.map(|v| v * 8usize)) {
            Some(b) => quote! {Some(#b)},
            None => quote! {None},
        };

        let field_writer_func = if field_writer.is_some() {
            quote! { #field_writer }
        } else {
            quote! { field_val.write(field_bits) }
        };

        let field_write = quote! {
            let field_val = if (#endian_flip) {
                #field_ident.swap_endian()
            } else {
                #field_ident
            };

            let field_bits = #field_bits;
            let bits = #field_writer_func;
            acc.extend(bits);
        };

        field_writes.push(field_write);
    }

    tokens.extend(quote! {
        impl<P> From<#ident> for BitVec<P, u8> where P: BitOrder {
            fn from(mut input: #ident) -> Self {
                let mut acc: BitVec<P, u8> = BitVec::new();

                #(#field_overwrites)*

                #(#field_writes)*

                acc
            }
        }

        impl From<#ident> for Vec<u8> {
            fn from(mut input: #ident) -> Self {
                let mut acc: BitVec<Msb0, u8> = input.into();
                acc.into_vec()
            }
        }

        impl BitsWriter for #ident {
            fn write(self, bit_size: Option<usize>) -> BitVec<Msb0, u8> {
                self.into()
            }

            fn swap_endian(self) -> Self {
                // do nothing
                self
            }
        }
    });

    // println!("{}", tokens.to_string());
    Ok(tokens)
}
