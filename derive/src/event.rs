// Copyright 2015-2019 Parity Technologies
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use heck::{ToSnakeCase, ToUpperCamelCase};
use proc_macro2::{Span, TokenStream};
use quote::quote;

use super::{from_token, get_template_names, rust_type, to_syntax_string, to_token};

/// Structure used to generate contract's event interface.
pub struct Event {
    name: String,
    log_fields: Vec<TokenStream>,
    recreate_inputs_quote: TokenStream,
    log_init: Vec<TokenStream>,
    wildcard_filter_params: Vec<TokenStream>,
    filter_declarations: Vec<TokenStream>,
    filter_definitions: Vec<TokenStream>,
    filter_init: Vec<TokenStream>,
    anonymous: bool,
}

impl<'a> From<&'a rethabi::Event> for Event {
    fn from(e: &'a rethabi::Event) -> Self {
        let names: Vec<_> = e
            .inputs
            .iter()
            .enumerate()
            .map(|(index, param)| {
                if param.name.is_empty() {
                    if param.indexed {
                        syn::Ident::new(&format!("topic{index}"), Span::call_site())
                    } else {
                        syn::Ident::new(&format!("param{index}"), Span::call_site())
                    }
                } else {
                    syn::Ident::new(&param.name.to_snake_case(), Span::call_site())
                }
            })
            .collect();
        let kinds: Vec<_> = e.inputs.iter().map(|param| rust_type(&param.kind)).collect();
        let log_fields = names
            .iter()
            .zip(kinds.iter())
            .map(|(param_name, kind)| quote! { pub #param_name: #kind })
            .collect();

        let log_iter = quote! { log.next().expect(INTERNAL_ERR).value };

        let to_log: Vec<_> =
            e.inputs.iter().map(|param| from_token(&param.kind, &log_iter)).collect();

        let log_init = names
            .iter()
            .zip(to_log.iter())
            .map(|(param_name, convert)| quote! { #param_name: #convert })
            .collect();

        let topic_kinds: Vec<_> = e
            .inputs
            .iter()
            .filter(|param| param.indexed)
            .map(|param| rust_type(&param.kind))
            .collect();
        let topic_names: Vec<_> = e
            .inputs
            .iter()
            .enumerate()
            .filter(|&(_, param)| param.indexed)
            .map(|(index, param)| {
                if param.name.is_empty() {
                    syn::Ident::new(&format!("topic{index}"), Span::call_site())
                } else {
                    syn::Ident::new(&param.name.to_snake_case(), Span::call_site())
                }
            })
            .collect();

        // [T0, T1, T2]
        let template_names: Vec<_> = get_template_names(&topic_kinds);

        let filter_declarations: Vec<_> = topic_kinds
            .iter()
            .zip(template_names.iter())
            .map(|(kind, template_name)| quote! { #template_name: Into<rethabi::Topic<#kind>> })
            .collect();

        let filter_definitions: Vec<_> = topic_names
            .iter()
            .zip(template_names.iter())
            .map(|(param_name, template_name)| quote! { #param_name: #template_name })
            .collect();

        // The number of parameters that creates a filter which matches anything.
        let wildcard_filter_params: Vec<_> =
            filter_definitions.iter().map(|_| quote! { rethabi::Topic::Any }).collect();

        let filter_init: Vec<_> = topic_names
            .iter()
            .zip(e.inputs.iter().filter(|p| p.indexed))
            .enumerate()
            .take(3)
            .map(|(index, (param_name, param))| {
                let topic = syn::Ident::new(&format!("topic{index}"), Span::call_site());
                let i = quote! { i };
                let to_token = to_token(&i, &param.kind);
                quote! { #topic: #param_name.into().map(|#i| #to_token), }
            })
            .collect();

        let event_inputs = &e
            .inputs
            .iter()
            .map(|x| {
                let name = &x.name;
                let kind = to_syntax_string(&x.kind);
                let indexed = x.indexed;

                quote! {
                    rethabi::EventParam {
                        name: #name.to_owned(),
                        kind: #kind,
                        indexed: #indexed
                    }
                }
            })
            .collect::<Vec<_>>();
        let recreate_inputs_quote = quote! { vec![ #(#event_inputs),* ] };

        Event {
            name: e.name.clone(),
            log_fields,
            recreate_inputs_quote,
            log_init,
            anonymous: e.anonymous,
            wildcard_filter_params,
            filter_declarations,
            filter_definitions,
            filter_init,
        }
    }
}

impl Event {
    /// Generates event log struct.
    pub fn generate_log(&self) -> TokenStream {
        let name = syn::Ident::new(&self.name.to_upper_camel_case(), Span::call_site());
        let log_fields = &self.log_fields;

        quote! {
            #[derive(Debug, Clone, PartialEq, Eq)]
            pub struct #name {
                #(#log_fields),*
            }
        }
    }

    /// Generates rust interface for contract's event.
    pub fn generate_event(&self) -> TokenStream {
        let name_as_string = &self.name.to_upper_camel_case();
        let name = syn::Ident::new(&self.name.to_snake_case(), Span::call_site());
        let camel_name = syn::Ident::new(&self.name.to_upper_camel_case(), Span::call_site());
        let recreate_inputs_quote = &self.recreate_inputs_quote;
        let anonymous = &self.anonymous;
        let log_init = &self.log_init;
        let filter_init = &self.filter_init;
        let filter_declarations = &self.filter_declarations;
        let filter_definitions = &self.filter_definitions;
        let wildcard_filter_params = &self.wildcard_filter_params;

        quote! {
            pub mod #name {
                use rethabi;
                use super::INTERNAL_ERR;

                pub fn event() -> rethabi::Event {
                    rethabi::Event {
                        name: #name_as_string.into(),
                        inputs: #recreate_inputs_quote,
                        anonymous: #anonymous,
                    }
                }

                pub fn filter<#(#filter_declarations),*>(#(#filter_definitions),*) -> rethabi::TopicFilter {
                    let raw = rethabi::RawTopicFilter {
                        #(#filter_init)*
                        ..Default::default()
                    };

                    let e = event();
                    e.filter(raw).expect(INTERNAL_ERR)
                }

                pub fn wildcard_filter() -> rethabi::TopicFilter {
                    filter(#(#wildcard_filter_params),*)
                }

                pub fn parse_log(log: rethabi::RawLog) -> rethabi::Result<super::super::logs::#camel_name> {
                    let e = event();
                    let mut log = e.parse_log(log)?.params.into_iter();
                    let result = super::super::logs::#camel_name {
                        #(#log_init),*
                    };
                    Ok(result)
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Event;
    use quote::quote;

    #[test]
    fn test_empty_log() {
        let ethabi_event =
            rethabi::Event { name: "hello".into(), inputs: vec![], anonymous: false };

        let e = Event::from(&ethabi_event);

        let expected = quote! {
            #[derive(Debug, Clone, PartialEq, Eq)]
            pub struct Hello {}
        };

        assert_eq!(expected.to_string(), e.generate_log().to_string());
    }

    #[test]
    fn test_empty_event() {
        let ethabi_event =
            rethabi::Event { name: "hello".into(), inputs: vec![], anonymous: false };

        let e = Event::from(&ethabi_event);

        let expected = quote! {
            pub mod hello {
                use rethabi;
                use super::INTERNAL_ERR;

                pub fn event() -> rethabi::Event {
                    rethabi::Event {
                        name: "Hello".into(),
                        inputs: vec![],
                        anonymous: false,
                    }
                }

                pub fn filter<>() -> rethabi::TopicFilter {
                    let raw = rethabi::RawTopicFilter {
                        ..Default::default()
                    };

                    let e = event();
                    e.filter(raw).expect(INTERNAL_ERR)
                }

                pub fn wildcard_filter() -> rethabi::TopicFilter {
                    filter()
                }

                pub fn parse_log(log: rethabi::RawLog) -> rethabi::Result<super::super::logs::Hello> {
                    let e = event();
                    let mut log = e.parse_log(log)?.params.into_iter();
                    let result = super::super::logs::Hello {};
                    Ok(result)
                }
            }
        };

        assert_eq!(expected.to_string(), e.generate_event().to_string());
    }

    #[test]
    #[ignore = "TODO"]
    fn test_event_with_one_input() {
        let ethabi_event = rethabi::Event {
            name: "one".into(),
            inputs: vec![rethabi::EventParam {
                name: "foo".into(),
                kind: rethabi::ParamType::Address,
                indexed: true,
            }],
            anonymous: false,
        };

        let e = Event::from(&ethabi_event);

        let expected = quote! {
            pub mod one {
                use rethabi;
                use super::INTERNAL_ERR;

                pub fn event() -> rethabi::Event {
                    rethabi::Event {
                        name: "One".into(),
                        inputs: vec![rethabi::EventParam {
                            name: "foo".to_owned(),
                            kind: rethabi::ParamType::Address,
                            indexed: true
                        }],
                        anonymous: false,
                    }
                }

                pub fn filter<T0: Into<rethabi::Topic<rethabi::Address>>>(foo: T0) -> rethabi::TopicFilter {
                    let raw = rethabi::RawTopicFilter {
                        topic0: foo.into().map(|i| rethabi::Token::Address(i)),
                        ..Default::default()
                    };

                    let e = event();
                    e.filter(raw).expect(INTERNAL_ERR)
                }

                pub fn wildcard_filter() -> rethabi::TopicFilter {
                    filter(rethabi::Topic::Any)
                }

                pub fn parse_log(log: rethabi::RawLog) -> rethabi::Result<super::super::logs::One> {
                    let e = event();
                    let mut log = e.parse_log(log)?.params.into_iter();
                    let result = super::super::logs::One {
                        foo: log.next().expect(INTERNAL_ERR).value.into_address().expect(INTERNAL_ERR)
                    };
                    Ok(result)
                }
            }
        };

        assert_eq!(expected.to_string(), e.generate_event().to_string());
    }

    #[test]
    #[ignore = "TODO"]
    fn test_log_with_one_field() {
        let ethabi_event = rethabi::Event {
            name: "one".into(),
            inputs: vec![rethabi::EventParam {
                name: "foo".into(),
                kind: rethabi::ParamType::Address,
                indexed: false,
            }],
            anonymous: false,
        };

        let e = Event::from(&ethabi_event);

        let expected = quote! {
            #[derive(Debug, Clone, PartialEq, Eq)]
            pub struct One {
                pub foo: rethabi::Address
            }
        };

        assert_eq!(expected.to_string(), e.generate_log().to_string());
    }

    #[test]
    #[ignore = "TODO"]
    fn test_log_with_multiple_field() {
        let ethabi_event = rethabi::Event {
            name: "many".into(),
            inputs: vec![
                rethabi::EventParam {
                    name: "foo".into(),
                    kind: rethabi::ParamType::Address,
                    indexed: false,
                },
                rethabi::EventParam {
                    name: "bar".into(),
                    kind: rethabi::ParamType::Array(Box::new(rethabi::ParamType::String)),
                    indexed: false,
                },
                rethabi::EventParam {
                    name: "xyz".into(),
                    kind: rethabi::ParamType::Uint(256),
                    indexed: false,
                },
            ],
            anonymous: false,
        };

        let e = Event::from(&ethabi_event);

        let expected = quote! {
            #[derive(Debug, Clone, PartialEq, Eq)]
            pub struct Many {
                pub foo: rethabi::Address,
                pub bar: Vec<String>,
                pub xyz: rethabi::Uint
            }
        };

        assert_eq!(expected.to_string(), e.generate_log().to_string());
    }
}
