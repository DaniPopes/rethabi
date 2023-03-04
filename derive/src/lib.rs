// Copyright 2015-2019 Parity Technologies
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![recursion_limit = "256"]

extern crate proc_macro;

mod constructor;
mod contract;
mod event;
mod function;

use heck::ToSnakeCase;
use proc_macro2::Span;
use quote::{format_ident, quote};
use rethabi::{Contract, Error, Param, ParamType, Result};
use std::{borrow::Cow, env, fs, path::PathBuf};

const ERROR_MSG: &str = "`derive(EthabiContract)` failed";

#[proc_macro_derive(EthabiContract, attributes(ethabi_contract_options))]
pub fn ethabi_derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ast = syn::parse(input).expect(ERROR_MSG);
    let gen = impl_ethabi_derive(&ast).expect(ERROR_MSG);
    gen.into()
}

fn impl_ethabi_derive(ast: &syn::DeriveInput) -> Result<proc_macro2::TokenStream> {
    let options = get_options(&ast.attrs, "ethabi_contract_options")?;
    let path = get_option(&options, "path")?;
    let normalized_path = normalize_path(&path)?;
    let source_file = fs::File::open(&normalized_path).map_err(|e| {
        Error::Other(Cow::Owned(format!(
            "Cannot load contract abi from `{}`: {e}",
            normalized_path.display()
        )))
    })?;
    let contract = Contract::load(source_file)?;
    let c = contract::Contract::from(&contract);
    Ok(c.generate())
}

fn get_options(attrs: &[syn::Attribute], name: &str) -> Result<Vec<syn::NestedMeta>> {
    let options =
        attrs.iter().flat_map(syn::Attribute::parse_meta).find(|meta| meta.path().is_ident(name));

    match options {
        Some(syn::Meta::List(list)) => Ok(list.nested.into_iter().collect()),
        _ => Err(Error::Other(Cow::Borrowed("Unexpected meta item"))),
    }
}

fn get_option(options: &[syn::NestedMeta], name: &str) -> Result<String> {
    let item = options
        .iter()
        .flat_map(|nested| match *nested {
            syn::NestedMeta::Meta(ref meta) => Some(meta),
            _ => None,
        })
        .find(|meta| meta.path().is_ident(name))
        .ok_or_else(|| Error::Other(Cow::Owned(format!("Expected to find option {name}"))))?;

    str_value_of_meta_item(item, name)
}

fn str_value_of_meta_item(item: &syn::Meta, name: &str) -> Result<String> {
    if let syn::Meta::NameValue(ref name_value) = *item {
        if let syn::Lit::Str(ref value) = name_value.lit {
            return Ok(value.value());
        }
    }

    Err(Error::Other(Cow::Owned(format!(
        r#"`{name}` must be in the form `#[{name}="something"]`"#
    ))))
}

fn normalize_path(relative_path: &str) -> Result<PathBuf> {
    // workaround for https://github.com/rust-lang/rust/issues/43860
    let cargo_toml_directory = env::var("CARGO_MANIFEST_DIR")
        .map_err(|_| Error::Other(Cow::Borrowed("Cannot find manifest file")))?;
    let mut path: PathBuf = cargo_toml_directory.into();
    path.push(relative_path);
    Ok(path)
}

fn to_syntax_string(param_type: &rethabi::ParamType) -> proc_macro2::TokenStream {
    match *param_type {
        ParamType::Address => quote! { rethabi::ParamType::Address },
        ParamType::Bytes => quote! { rethabi::ParamType::Bytes },
        ParamType::Int(x) => quote! { rethabi::ParamType::Int(#x) },
        ParamType::Uint(x) => quote! { rethabi::ParamType::Uint(#x) },
        ParamType::Bool => quote! { rethabi::ParamType::Bool },
        ParamType::String => quote! { rethabi::ParamType::String },
        ParamType::Array(ref param_type) => {
            let param_type_quote = to_syntax_string(param_type);
            quote! { rethabi::ParamType::Array(Box::new(#param_type_quote)) }
        }
        ParamType::FixedBytes(x) => quote! { rethabi::ParamType::FixedBytes(#x) },
        ParamType::FixedArray(ref param_type, ref x) => {
            let param_type_quote = to_syntax_string(param_type);
            quote! { rethabi::ParamType::FixedArray(Box::new(#param_type_quote), #x) }
        }
        ParamType::Tuple(_) => {
            unimplemented!(
                "Tuples are not supported. https://github.com/openethereum/ethabi/issues/175"
            )
        }
    }
}

fn to_ethabi_param_vec<'a, P: 'a>(params: P) -> proc_macro2::TokenStream
where
    P: IntoIterator<Item = &'a Param>,
{
    let p = params
        .into_iter()
        .map(|x| {
            let name = &x.name;
            let kind = to_syntax_string(&x.kind);
            quote! {
                rethabi::Param {
                    name: #name.to_owned(),
                    kind: #kind,
                    internal_type: None
                }
            }
        })
        .collect::<Vec<_>>();

    quote! { vec![ #(#p),* ] }
}

fn rust_type(input: &ParamType) -> proc_macro2::TokenStream {
    match *input {
        ParamType::Address => quote! { ::rethabi::Address },
        ParamType::Bytes => quote! { ::rethabi::Bytes },
        ParamType::FixedBytes(32) => quote! { ::rethabi::Hash },
        ParamType::FixedBytes(size) => quote! { [u8; #size] },
        ParamType::Int(_) => quote! { ::rethabi::Int },
        ParamType::Uint(_) => quote! { ::rethabi::Uint },
        ParamType::Bool => quote! { bool },
        ParamType::String => quote! { String },
        ParamType::Array(ref kind) => {
            let t = rust_type(kind);
            quote! { Vec<#t> }
        }
        ParamType::FixedArray(ref kind, size) => {
            let t = rust_type(kind);
            quote! { [#t, #size] }
        }
        ParamType::Tuple(_) => {
            unimplemented!(
                "Tuples are not supported. https://github.com/openethereum/ethabi/issues/175"
            )
        }
    }
}

fn template_param_type(input: &ParamType, index: usize) -> proc_macro2::TokenStream {
    let t_ident = format_ident!("T{index}");
    match input {
        ParamType::Array(ty) => {
            let u_ident = format_ident!("U{index}");
            let u = _template_param_type(ty, &u_ident);
            quote! {
                #t_ident: ::core::iter::IntoIterator<Item = #u_ident>, #u
            }
        }
        ParamType::FixedArray(ty, size) => {
            let u_ident = format_ident!("U{index}");
            let u = _template_param_type(ty, &u_ident);
            quote! {
                #t_ident: ::core::convert::Into<[#u_ident; #size]>, #u
            }
        }
        ParamType::Tuple(_) => {
            unimplemented!(
                "Tuples are not supported. https://github.com/openethereum/ethabi/issues/175"
            )
        }
        ty => _template_param_type(ty, &t_ident),
    }
}

fn _template_param_type(input: &ParamType, ident: &syn::Ident) -> proc_macro2::TokenStream {
    match input {
        ParamType::Int(_) | ParamType::Uint(_) => {
            quote! { #ident: ::rethabi::ruint::UintTryTo<::rethabi::Uint> }
        }
        ParamType::Array(_) | ParamType::FixedArray(_, _) => {
            unimplemented!("Recursive arrays are not supported.")
        }
        ParamType::Tuple(_) => {
            unimplemented!(
                "Tuples are not supported. https://github.com/openethereum/ethabi/issues/175"
            )
        }
        ty => {
            let ty = rust_type(ty);
            quote! { #ident: ::core::convert::Into<#ty> }
        }
    }
}

fn from_template_param(input: &ParamType, name: &syn::Ident) -> proc_macro2::TokenStream {
    match input {
        ParamType::Array(ty) => {
            let arg = syn::Ident::new("__v", name.span());
            let convert = from_template_param(ty.as_ref(), &arg);
            quote! { #name.into_iter().map(|#arg| #convert).collect::<Vec<_>>() }
        }
        ParamType::FixedArray(ty, _) => {
            let arg = syn::Ident::new("__v", name.span());
            let convert = from_template_param(ty.as_ref(), &arg);
            quote! { #name.into().into_iter().map(|#arg| #convert).collect::<Vec<_>>() }
        }
        ParamType::Uint(_) | ParamType::Int(_) => {
            quote! { #name.uint_try_to().expect(INTERNAL_ERR) }
        }
        _ => quote! { #name.into() },
    }
}

fn to_token(name: &proc_macro2::TokenStream, kind: &ParamType) -> proc_macro2::TokenStream {
    match *kind {
        ParamType::Address => quote! { rethabi::Token::Address(#name) },
        ParamType::Bytes => quote! { rethabi::Token::Bytes(#name) },
        ParamType::FixedBytes(_) => quote! { rethabi::Token::FixedBytes(#name.to_vec()) },
        ParamType::Int(_) => quote! { rethabi::Token::Int(#name) },
        ParamType::Uint(_) => quote! { rethabi::Token::Uint(#name) },
        ParamType::Bool => quote! { rethabi::Token::Bool(#name) },
        ParamType::String => quote! { rethabi::Token::String(#name) },
        ParamType::Array(ref kind) => {
            let inner_name = quote! { inner };
            let inner_loop = to_token(&inner_name, kind);
            quote! {
                // note the double {{
                {
                    let v = #name.into_iter().map(|#inner_name| #inner_loop).collect();
                    rethabi::Token::Array(v)
                }
            }
        }
        ParamType::FixedArray(ref kind, _) => {
            let inner_name = quote! { inner };
            let inner_loop = to_token(&inner_name, kind);
            quote! {
                // note the double {{
                {
                    let v = #name.into_iter().map(|#inner_name| #inner_loop).collect();
                    rethabi::Token::FixedArray(v)
                }
            }
        }
        ParamType::Tuple(_) => {
            unimplemented!(
                "Tuples are not supported. https://github.com/openethereum/ethabi/issues/175"
            )
        }
    }
}

fn from_token(kind: &ParamType, token: &proc_macro2::TokenStream) -> proc_macro2::TokenStream {
    match *kind {
        ParamType::Address => quote! { #token.into_address().expect(INTERNAL_ERR) },
        ParamType::Bytes => quote! { #token.into_bytes().expect(INTERNAL_ERR) },
        ParamType::FixedBytes(32) => quote! {
            {
                let mut result = [0u8; 32];
                let v = #token.into_fixed_bytes().expect(INTERNAL_ERR);
                result.copy_from_slice(&v);
                rethabi::Hash::from(result)
            }
        },
        ParamType::FixedBytes(size) => {
            quote! {
                {
                    let mut result = [0u8; #size];
                    let v = #token.into_fixed_bytes().expect(INTERNAL_ERR);
                    result.copy_from_slice(&v);
                    result
                }
            }
        }
        ParamType::Int(_) => quote! { #token.into_int().expect(INTERNAL_ERR) },
        ParamType::Uint(_) => quote! { #token.into_uint().expect(INTERNAL_ERR) },
        ParamType::Bool => quote! { #token.into_bool().expect(INTERNAL_ERR) },
        ParamType::String => quote! { #token.into_string().expect(INTERNAL_ERR) },
        ParamType::Array(ref kind) => {
            let inner = quote! { inner };
            let inner_loop = from_token(kind, &inner);
            quote! {
                #token.into_array().expect(INTERNAL_ERR).into_iter()
                    .map(|#inner| #inner_loop)
                    .collect()
            }
        }
        ParamType::FixedArray(ref kind, size) => {
            let inner = quote! { inner };
            let inner_loop = from_token(kind, &inner);
            let to_array = vec![quote! { iter.next() }; size];
            quote! {
                {
                    let iter = #token.to_array().expect(INTERNAL_ERR).into_iter()
                        .map(|#inner| #inner_loop);
                    [#(#to_array),*]
                }
            }
        }
        ParamType::Tuple(_) => {
            unimplemented!(
                "Tuples are not supported. https://github.com/openethereum/ethabi/issues/175"
            )
        }
    }
}

fn input_names(inputs: &[Param]) -> Vec<syn::Ident> {
    inputs
        .iter()
        .enumerate()
        .map(|(index, param)| {
            if param.name.is_empty() {
                syn::Ident::new(&format!("param{index}"), Span::call_site())
            } else {
                syn::Ident::new(&rust_variable(&param.name), Span::call_site())
            }
        })
        .collect()
}

fn get_template_names(kinds: &[proc_macro2::TokenStream]) -> Vec<syn::Ident> {
    kinds
        .iter()
        .enumerate()
        .map(|(index, _)| syn::Ident::new(&format!("T{index}"), Span::call_site()))
        .collect()
}

fn get_output_kinds(outputs: &[Param]) -> proc_macro2::TokenStream {
    match outputs.len() {
        0 => quote! {()},
        1 => {
            let t = rust_type(&outputs[0].kind);
            quote! { #t }
        }
        _ => {
            let outs: Vec<_> = outputs.iter().map(|param| rust_type(&param.kind)).collect();
            quote! { (#(#outs),*) }
        }
    }
}

/// Convert input into a rust variable name.
///
/// Avoid using keywords by escaping them.
fn rust_variable(name: &str) -> String {
    // avoid keyword parameters
    match name {
        "self" => "_self".to_string(),
        other => other.to_snake_case(),
    }
}
