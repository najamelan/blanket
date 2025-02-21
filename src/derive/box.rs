use syn::parse_quote;
use syn::spanned::Spanned;

use crate::utils::deref_expr;
use crate::utils::generics_declaration_to_generics;
use crate::utils::signature_to_method_call;
use crate::utils::trait_to_generic_ident;

pub fn derive(trait_: &syn::ItemTrait) -> syn::Result<syn::ItemImpl> {
    // build the methods
    let mut methods: Vec<syn::ImplItemMethod> = Vec::new();
    for item in trait_.items.iter() {
        if let syn::TraitItem::Method(ref m) = item {
            let signature = &m.sig;
            let mut call = signature_to_method_call(signature)?;

            match signature.receiver() {
                // fn()
                None => unimplemented!(),
                // `fn(self: Type)`
                Some(syn::FnArg::Typed(pat)) => {
                    let msg = "cannot derive `Box` for a trait declaring methods with arbitrary receiver types";
                    return Err(syn::Error::new(pat.span(), msg));
                }
                // `fn(&self)` and `fn(&mut self)`
                Some(syn::FnArg::Receiver(r)) if r.reference.is_some() => {
                    call.receiver = Box::new(deref_expr(deref_expr(*call.receiver)));
                }
                // `fn(self)`
                Some(syn::FnArg::Receiver(_)) => {
                    call.receiver = Box::new(deref_expr(*call.receiver));
                }
            }

            let item = parse_quote!(#[inline] #signature { #call });
            methods.push(item)
        }
    }

    // build an identifier for the generic type used for the implementation
    let trait_ident = &trait_.ident;
    let generic_type = trait_to_generic_ident(&trait_);

    // build the generics for the impl block:
    // we use the same generics as the trait itself, plus
    // a generic type that implements the trait for which we provide the
    // blanket implementation
    let trait_generics = &trait_.generics;
    let where_clause = &trait_.generics.where_clause;
    let mut impl_generics = trait_generics.clone();

    // we must however remove the generic type bounds, to avoid repeating them
    let mut trait_generic_names = trait_generics.clone();
    trait_generic_names.params = generics_declaration_to_generics(&trait_generics.params)?;

    impl_generics.params.push(syn::GenericParam::Type(
        parse_quote!(#generic_type: #trait_ident #trait_generic_names),
    ));

    // generate the impl block
    Ok(parse_quote!(
        #[automatically_derived]
        impl #impl_generics #trait_ident #trait_generic_names for Box<#generic_type> #where_clause {
            #(#methods)*
        }
    ))
}

#[cfg(test)]
mod tests {
    mod derive {

        use syn::parse_quote;

        #[test]
        fn empty() {
            let trait_ = parse_quote!(
                trait MyTrait {}
            );
            let derived = super::super::derive(&trait_).unwrap();
            assert_eq!(
                derived,
                parse_quote!(
                    #[automatically_derived]
                    impl<MT: MyTrait> MyTrait for Box<MT> {}
                )
            );
        }

        #[test]
        fn receiver_ref() {
            let trait_ = parse_quote!(
                trait MyTrait {
                    fn my_method(&self);
                }
            );
            assert_eq!(
                super::super::derive(&trait_).unwrap(),
                parse_quote!(
                    #[automatically_derived]
                    impl<MT: MyTrait> MyTrait for Box<MT> {
                        #[inline]
                        fn my_method(&self) {
                            (*(*self)).my_method()
                        }
                    }
                )
            );
        }

        #[test]
        fn receiver_mut() {
            let trait_ = parse_quote!(
                trait MyTrait {
                    fn my_method(&mut self);
                }
            );
            assert_eq!(
                super::super::derive(&trait_).unwrap(),
                parse_quote!(
                    #[automatically_derived]
                    impl<MT: MyTrait> MyTrait for Box<MT> {
                        #[inline]
                        fn my_method(&mut self) {
                            (*(*self)).my_method()
                        }
                    }
                )
            );
        }

        #[test]
        fn receiver_self() {
            let trait_ = parse_quote!(
                trait MyTrait {
                    fn my_method(self);
                }
            );
            assert_eq!(
                super::super::derive(&trait_).unwrap(),
                parse_quote!(
                    #[automatically_derived]
                    impl<MT: MyTrait> MyTrait for Box<MT> {
                        #[inline]
                        fn my_method(self) {
                            (*self).my_method()
                        }
                    }
                )
            );
        }

        #[test]
        fn receiver_arbitrary() {
            let trait_ = parse_quote!(
                trait MyTrait {
                    fn my_method(self: Box<Self>);
                }
            );
            assert!(super::super::derive(&trait_).is_err());
        }

        #[test]
        fn generics() {
            let trait_ = parse_quote!(
                trait MyTrait<T> {}
            );
            let derived = super::super::derive(&trait_).unwrap();

            assert_eq!(
                derived,
                parse_quote!(
                    #[automatically_derived]
                    impl<T, MT: MyTrait<T>> MyTrait<T> for Box<MT> {}
                )
            );
        }

        #[test]
        fn generics_bounded() {
            let trait_ = parse_quote!(
                trait MyTrait<T: 'static + Send> {}
            );
            let derived = super::super::derive(&trait_).unwrap();

            assert_eq!(
                derived,
                parse_quote!(
                    #[automatically_derived]
                    impl<T: 'static + Send, MT: MyTrait<T>> MyTrait<T> for Box<MT> {}
                )
            );
        }

        #[test]
        fn generics_lifetime() {
            let trait_ = parse_quote!(
                trait MyTrait<'a, 'b: 'a, T: 'static + Send> {}
            );
            let derived = super::super::derive(&trait_).unwrap();

            assert_eq!(
                derived,
                parse_quote!(
                    #[automatically_derived]
                    impl<'a, 'b: 'a, T: 'static + Send, MT: MyTrait<'a, 'b, T>> MyTrait<'a, 'b, T> for Box<MT> {}
                )
            );
        }
    }
}
