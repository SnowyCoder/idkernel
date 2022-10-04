use std::iter::repeat;

use proc_macro::TokenStream;
use proc_macro2::Span;
use syn::{parse::{Parse, ParseStream}, parse_macro_input, Ident, LitInt, token::Comma};
use quote::quote;

const REGISTRIES: &'static [&'static str] = &[
    /*"rax",*/ "rdi", "rsi", "rdx", "r10", "r8", "r9"
];
const INPUT_NAMES: &'static [&'static str] = &[
    /*"sel",*/ "a", "b", "c", "d", "e", "f"
];
const OUTPUT_NAMES: &'static [&'static str] = &[
    /*"err",*/ "ra", "rb", "rc", "rd", "re", "rf"
];

#[derive(Debug)]
struct SyscallDescriptor {
    name: Ident,
    selector: Ident,
    arg_count: u8,
    arg_span: Span,
    ret_count: u8,
    ret_span: Span,
}

impl Parse for SyscallDescriptor {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let name = input.parse()?;
        let _: Comma = input.parse()?;

        let selector = input.parse()?;
        let _: Comma = input.parse()?;

        let argc = input.parse::<LitInt>()?;
        let arg_span = argc.span();
        let arg_count = argc.base10_parse()?;
        let _: Comma = input.parse()?;

        let retc = input.parse::<LitInt>()?;
        let ret_count = retc.base10_parse()?;
        let ret_span = retc.span();

        Ok(SyscallDescriptor {
            name, selector, arg_count, arg_span, ret_count, ret_span,
        })
    }
}


#[proc_macro]
pub fn create_syscall(stream: TokenStream) -> TokenStream {
    let input = parse_macro_input!(stream as SyscallDescriptor);

    if input.arg_count as usize > INPUT_NAMES.len() - 1 {
        let s = format!("Cannot create syscall with {} arguments, the maximum is {}", input.arg_count, INPUT_NAMES.len() - 1);
        return quote! { compile_error!(#s); }.into()
    }

    if input.ret_count as usize > OUTPUT_NAMES.len() - 1 {
        let s = format!("Cannot create syscall with {} arguments, the maximum is {}", input.ret_count, OUTPUT_NAMES.len() - 1);
        return quote! { compile_error!(#s); }.into()
    }

    let args: Vec<_> = INPUT_NAMES[0..(input.arg_count as usize)]
        .iter()
        .map(|x| Ident::new(x, input.arg_span))
        .collect();
    let vars: Vec<_> = OUTPUT_NAMES[0..(input.ret_count as usize)]
        .iter()
        .map(|x| Ident::new(x, input.ret_span))
        .collect();


    let asm_in = (0..input.arg_count).map(|i| {
        let arg = &args[i as usize];
        let reg = REGISTRIES[i as usize];

        quote!{ in(#reg) #arg }
    });

    let asm_out = (0..input.ret_count).map(|i| {
        let arg = &vars[i as usize];
        let reg = REGISTRIES[i as usize];

        quote!{ lateout(#reg) #arg }
    });
    let rets = repeat(quote!(u64))
        .take(input.ret_count as usize);

    let selector = input.selector;
    let name = input.name;
    let expanded = quote! {
        pub unsafe fn #name(#(#args: u64),*) -> SyscallResult<(#(#rets),*)> {
            let sel = SyscallCode::#selector as u64;
            let err: u64;
            #(let #vars: u64;)*
            core::arch::asm! {
                "syscall",
                in ("rax") sel,
                #(#asm_in,)*
                lateout ("rax") err,
                #(#asm_out),*
            };
            if err != 0 {
                return Err(
                    SyscallError::try_from(err)
                    .unwrap_or(SyscallError::UnknownError)
                )
            }
            Ok((#(#vars),*))
        }
    };
    //println!("{expanded}");
    TokenStream::from(expanded)
}
