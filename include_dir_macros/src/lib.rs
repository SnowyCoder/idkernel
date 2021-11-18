#![feature(proc_macro_tracked_env)]

extern crate proc_macro;
use std::{borrow::Cow, ffi::OsStr, panic, path::Path};

use proc_macro::TokenStream;

use proc_macro2::TokenTree;
use quote::quote;
use regex::{Captures, Regex};

#[proc_macro]
pub fn include_dir(t: TokenStream) -> TokenStream {
    let t = proc_macro2::TokenStream::from(t);
    let tokens: Vec<_> = t.into_iter().collect();
    let lit = match tokens.as_slice() {
        [TokenTree::Literal(x)] => x.to_string(),
        _ => return quote! { compile_error!("include_dir expects a single literal!") }.into(),
    };
    let mut chars = lit.chars();
    match chars.next().unwrap() {
        '"' | '\'' => {},
        _ => return quote! { compile_error!("include_dir expects a string literal!") }.into(),
    };
    chars.next_back();

    let resolved_str = expand_str(chars.as_str());
    let path = Path::new(OsStr::new(resolved_str.as_ref()));
    if !path.exists() {
        let msg = format!("{} does not exist", resolved_str);
        return quote! { compile_error!(#msg) }.into();
    }
    if !path.is_dir() {
        let msg = format!("{} isn't a directory", resolved_str);
        return quote! { compile_error!(#msg) }.into();
    }

    let expanded_path = expand_dir(path);

    (quote! {{
        use ::include_dir::{InitDir, InitDirEntry, include_bytes_align_as};

        #expanded_path
    }}).into()
}

fn expand_str(s: &str) -> Cow<str> {
    let re = Regex::new(r"\$\{([^}]*)}").unwrap();
    re.replace_all(s, |caps: &Captures| {
        proc_macro::tracked_env::var(&caps[1])
            .unwrap_or_else(|_| panic!("Cannot find env variable {}", &caps[1]))
    })
}

fn expand_dir(path: &Path) -> proc_macro2::TokenStream {
    let iter = path.read_dir()
            .expect("Failed to read directory")
            .map(|entry| {
        let entry = entry.expect("Failed to read dir entry");

        let tt = if entry.file_type().unwrap().is_dir() {
            let folder = expand_dir(&path.join(entry.file_name()));
            quote! {
                InitDirEntry::Folder(#folder)
            }
        } else {
            let rpath = path.join(entry.file_name());
            let path = rpath.to_str().expect("Invalid OS dir path");
            quote! {
                InitDirEntry::File(include_bytes_align_as!(u64, #path))
            }
        };
        let fname = entry.file_name();
        let name = fname.to_str().unwrap();
        quote!{ (#name, #tt) }
    });
    quote! {
        InitDir(&[
            #(#iter),*
        ])
    }
}
